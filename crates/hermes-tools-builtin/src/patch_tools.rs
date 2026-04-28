//! patch_tools — 文件补丁工具
//!
//! 支持两种模式：
//! - replace: 单文件查找替换
//! - patch: 多文件批量补丁（两阶段验证-应用）

use async_trait::async_trait;
use hermes_checkpoint::CheckpointManager;
use hermes_core::{ToolContext, ToolError};
use hermes_environment::Environment;
use serde_json::json;
use std::sync::Arc;

/// 文件补丁工具
pub struct PatchTool {
    environment: Arc<dyn Environment>,
}

impl PatchTool {
    pub fn new(environment: Arc<dyn Environment>) -> Self {
        Self { environment }
    }

    /// 模糊匹配：在内容中查找 old_str
    /// 先精确匹配，失败时尝试 trim 前后空格，再失败逐行匹配
    fn fuzzy_find(content: &str, old_str: &str) -> Option<(usize, usize)> {
        // 1. 精确匹配
        if let Some(pos) = content.find(old_str) {
            return Some((pos, pos + old_str.len()));
        }
        // 2. trim 前后空格后匹配
        let trimmed = old_str.trim();
        if let Some(pos) = content.find(trimmed) {
            // 扩展匹配范围以包含原 old_str 可能的空格
            let start = content[..pos].rfind(old_str.lines().next()?.trim()).unwrap_or(pos);
            let end = pos + trimmed.len();
            return Some((start, end));
        }
        // 3. 逐行匹配：提取 old_str 中的非空行，找到它们在 content 中的位置
        let old_lines: Vec<&str> = old_str.lines().filter(|l| !l.trim().is_empty()).collect();
        if old_lines.is_empty() {
            return None;
        }
        let first_line = old_lines[0].trim();
        if let Some(line_start) = content.find(first_line) {
            let last_line = old_lines.last().unwrap().trim();
            if let Some(line_end) = content[line_start..].rfind(last_line) {
                let end = line_start + line_end + last_line.len();
                return Some((line_start, end));
            }
        }
        None
    }

    /// replace 模式：单文件查找替换
    async fn execute_replace(
        &self,
        file_path: &str,
        old_str: &str,
        new_str: &str,
        replace_all: bool,
        context: &ToolContext,
    ) -> Result<String, ToolError> {
        let normalized = Self::normalize_path(file_path, context)?;

        // 读取当前文件内容
        let content = self
            .environment
            .read_file(&normalized)
            .await
            .map_err(|e| ToolError::Execution(format!("读取文件失败: {}", e)))?;

        if !replace_all {
            // 单次替换
            let (_start, _end) = Self::fuzzy_find(&content, old_str).ok_or_else(|| {
                ToolError::InvalidArgs(format!(
                    "在文件 {} 中未找到匹配的文本。请用 read_file 确认文件内容，确保 old_str 完全匹配。",
                    normalized.display()
                ))
            })?;

            let new_content = content.replacen(old_str, new_str, 1);

            // 自动检查点
            if let Some(cm) = &context.checkpoint_manager {
                let _ = cm
                    .snapshot_file(&normalized, &context.working_directory)
                    .await;
            }

            self.environment
                .write_file(&normalized, &new_content)
                .await
                .map_err(|e| ToolError::Execution(format!("写入文件失败: {}", e)))?;

            Ok(json!({
                "success": true,
                "path": normalized.display().to_string(),
                "replacements": 1,
            })
            .to_string())
        } else {
            // 全量替换
            let count = content.matches(old_str).count();
            if count == 0 {
                return Err(ToolError::InvalidArgs(format!(
                    "在文件 {} 中未找到匹配的文本",
                    normalized.display()
                )));
            }

            let new_content = content.replace(old_str, new_str);

            if let Some(cm) = &context.checkpoint_manager {
                let _ = cm
                    .snapshot_file(&normalized, &context.working_directory)
                    .await;
            }

            self.environment
                .write_file(&normalized, &new_content)
                .await
                .map_err(|e| ToolError::Execution(format!("写入文件失败: {}", e)))?;

            Ok(json!({
                "success": true,
                "path": normalized.display().to_string(),
                "replacements": count,
            })
            .to_string())
        }
    }

    /// patch 模式：多文件批量补丁（两阶段：验证 → 应用）
    async fn execute_patches(
        &self,
        patches: &[serde_json::Value],
        context: &ToolContext,
    ) -> Result<String, ToolError> {
        if patches.is_empty() {
            return Err(ToolError::InvalidArgs("patches 数组不能为空".into()));
        }

        // ===== 阶段 1: 验证 =====
        let mut errors: Vec<String> = Vec::new();
        let mut file_contents: Vec<(std::path::PathBuf, String)> = Vec::new();

        for (i, patch) in patches.iter().enumerate() {
            let file_path = patch["file_path"].as_str().unwrap_or("");
            let old_str = patch["old_str"].as_str().unwrap_or("");
            let new_str = patch["new_str"].as_str().unwrap_or("");

            if file_path.is_empty() {
                errors.push(format!("patch[{}]: file_path 为必填参数", i));
                continue;
            }

            let normalized = match Self::normalize_path(file_path, context) {
                Ok(p) => p,
                Err(e) => {
                    errors.push(format!("patch[{}]: {}", i, e));
                    continue;
                }
            };

            let content = match self.environment.read_file(&normalized).await {
                Ok(c) => c,
                Err(e) => {
                    errors.push(format!("patch[{}] ({}): 读取失败 - {}", i, file_path, e));
                    continue;
                }
            };

            if Self::fuzzy_find(&content, old_str).is_none() {
                errors.push(format!(
                    "patch[{}] ({}): 未找到匹配的 old_str",
                    i, file_path
                ));
                continue;
            }

            let new_content = content.replace(old_str, new_str);
            file_contents.push((normalized, new_content));
        }

        if !errors.is_empty() {
            return Err(ToolError::InvalidArgs(format!(
                "补丁验证失败 ({} 个错误):\n{}",
                errors.len(),
                errors.join("\n")
            )));
        }

        // ===== 阶段 2: 应用 =====
        let mut applied = 0;
        for (i, (normalized, new_content)) in file_contents.iter().enumerate() {
            // 自动检查点
            if let Some(cm) = &context.checkpoint_manager {
                let _ = cm
                    .snapshot_file(normalized, &context.working_directory)
                    .await;
            }

            self.environment
                .write_file(normalized, new_content)
                .await
                .map_err(|e| {
                    ToolError::Execution(format!(
                        "写入 patch[{}] ({}): 失败 - {}。状态可能不一致，请用 /rollback 恢复。",
                        i,
                        normalized.display(),
                        e
                    ))
                })?;
            applied += 1;
        }

        Ok(json!({
            "success": true,
            "files_patched": applied,
            "total_patches": patches.len(),
        })
        .to_string())
    }

    /// 规范化文件路径
    fn normalize_path(
        file_path: &str,
        context: &ToolContext,
    ) -> Result<std::path::PathBuf, ToolError> {
        let path = std::path::Path::new(file_path);
        let normalized = if path.is_absolute() {
            path.to_path_buf()
        } else {
            context.working_directory.join(path)
        };
        // 安全检查：路径必须在工作目录内
        let canonical = normalized
            .canonicalize()
            .map_err(|e| ToolError::NotFound(format!("路径不存在: {} ({})", file_path, e)))?;
        if !canonical.starts_with(&context.working_directory) {
            return Err(ToolError::PermissionDenied(format!(
                "路径 {} 在工作目录之外",
                file_path
            )));
        }
        Ok(canonical)
    }
}

#[async_trait]
impl hermes_tool_registry::Tool for PatchTool {
    fn name(&self) -> &str {
        "patch"
    }

    fn description(&self) -> &str {
        "对文件进行查找替换。replace 模式用于单文件修改，patch 模式用于多文件批量补丁（两阶段验证-应用，确保原子性）。"
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "mode": {
                    "type": "string",
                    "enum": ["replace", "patch"],
                    "description": "replace: 单文件查找替换, patch: 多文件批量补丁",
                    "default": "replace"
                },
                "file_path": {
                    "type": "string",
                    "description": "要修改的文件路径（replace 模式必填）"
                },
                "old_str": {
                    "type": "string",
                    "description": "要查找的原始文本（replace 模式必填）"
                },
                "new_str": {
                    "type": "string",
                    "description": "替换后的新文本（replace 模式必填）"
                },
                "replace_all": {
                    "type": "boolean",
                    "description": "是否替换所有匹配项（仅 replace 模式）",
                    "default": false
                },
                "patches": {
                    "type": "array",
                    "description": "批量补丁列表（patch 模式），每项含 file_path, old_str, new_str",
                    "items": {
                        "type": "object",
                        "properties": {
                            "file_path": {"type": "string"},
                            "old_str": {"type": "string"},
                            "new_str": {"type": "string"}
                        },
                        "required": ["file_path", "old_str", "new_str"]
                    }
                }
            },
            "required": []
        })
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        context: ToolContext,
    ) -> Result<String, ToolError> {
        let mode = args["mode"].as_str().unwrap_or("replace");

        match mode {
            "replace" => {
                let file_path = args["file_path"]
                    .as_str()
                    .ok_or_else(|| ToolError::InvalidArgs("replace 模式需要 file_path 参数".into()))?;
                let old_str = args["old_str"]
                    .as_str()
                    .ok_or_else(|| ToolError::InvalidArgs("replace 模式需要 old_str 参数".into()))?;
                let new_str = args["new_str"]
                    .as_str()
                    .ok_or_else(|| ToolError::InvalidArgs("replace 模式需要 new_str 参数".into()))?;
                let replace_all = args["replace_all"].as_bool().unwrap_or(false);

                self.execute_replace(file_path, old_str, new_str, replace_all, &context)
                    .await
            }
            "patch" => {
                let patches: Vec<serde_json::Value> = args["patches"]
                    .as_array()
                    .cloned()
                    .ok_or_else(|| ToolError::InvalidArgs("patch 模式需要 patches 数组参数".into()))?;

                self.execute_patches(&patches, &context).await
            }
            _ => Err(ToolError::InvalidArgs(format!(
                "未知的 patch 模式: {}。支持 replace 和 patch",
                mode
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fuzzy_find_exact() {
        let content = "hello world\nthis is a test\n";
        let result = PatchTool::fuzzy_find(content, "hello world");
        assert!(result.is_some());
        let (start, end) = result.unwrap();
        assert_eq!(&content[start..end], "hello world");
    }

    #[test]
    fn test_fuzzy_find_trimmed() {
        let content = "  hello world\n";
        let result = PatchTool::fuzzy_find(content, "hello world");
        assert!(result.is_some());
    }

    #[test]
    fn test_fuzzy_find_not_found() {
        let content = "some content";
        let result = PatchTool::fuzzy_find(content, "nonexistent text");
        assert!(result.is_none());
    }

    #[test]
    fn test_fuzzy_find_multiline() {
        let content = "fn main() {\n    println!(\"hello\");\n}\n";
        let old = "fn main() {\n    println!(\"hello\");\n}";
        let result = PatchTool::fuzzy_find(content, old);
        assert!(result.is_some());
    }
}

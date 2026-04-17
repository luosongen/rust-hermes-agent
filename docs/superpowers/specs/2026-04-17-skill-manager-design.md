# Skill Manager Tool 设计文档

> **Status:** Approved for implementation

## 1. 概述

**目标：** 扩展现有 SkillsTool，添加创建/编辑/删除/patch 本地 skills 的能力，并集成安全扫描。

**技术方案：** 在 `SkillsTool` 上新增 actions，新增独立 `security_scanner.rs` 模块。

---

## 2. 数据结构

### 新增 SkillMetadata 字段

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SkillMetadata {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub triggers: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub source: String,        // "remote" | "local"
    #[serde(default)]
    pub origin_hash: String,    // SHA256 of remote source
    #[serde(default)]
    pub created_at: f64,        // 新增：创建时间
    #[serde(default)]
    pub updated_at: f64,        // 新增：更新时间
}
```

### 新的 Actions

```rust
enum SkillAction {
    // 现有
    List,
    View { name: String },
    Search { query: String, #[serde(default)] limit: Option<usize> },
    Sync,
    Install { name: String, #[serde(default)] source: Option<String> },
    Remove { name: String },

    // 新增
    Create { name: String, content: String, #[serde(default)] triggers: Option<Vec<String>>, #[serde(default)] tags: Option<Vec<String>> },
    Edit { name: String, field: String, value: String },
    Delete { name: String },
    Patch { name: String, patch_content: String },
    Scan { #[serde(default)] name: Option<String> },
}
```

---

## 3. 安全扫描模块

### 文件

`crates/hermes-tools-extended/src/skills/security_scanner.rs`

### 扫描规则

检测以下恶意模式：

| 模式 | 正则 | 风险 |
|------|------|------|
| 代码执行 | `eval\(\|exec\(\|compile\(.*\)` | 高 |
| 命令执行 | `subprocess\|os\.system\|os\.popen` | 高 |
| 动态导入 | `__import__\|importlib` | 高 |
| 文件操作劫持 | `open\s*=\s*\|_builtin_\.open` | 中 |
| 网络请求（非常规） | `requests\.(get\|post)\(` | 中 |
| Shell 脚本 | `\|\s*sh\|/bin/sh` | 高 |
| 凭证访问 | `os\.environ\["\|getenv\(` | 高 |

### 扫描结果

```rust
pub struct ScanResult {
    pub safe: bool,
    pub threats: Vec<Threat>,
}

pub struct Threat {
    pub pattern: String,
    pub line_number: usize,
    pub severity: Severity,  // High, Medium, Low
}
```

### 扫描流程

```
content → scan_content() →
  ✅ safe: return ScanResult { safe: true }
  ❌ unsafe: return ScanResult { safe: false, threats }
```

---

## 4. Actions 规格

### create

**参数：** `name`, `content`, `triggers?`, `tags?`

**流程：**
1. 检查 `name` 不存在
2. 调用 `security_scan(&content)`
3. ✅ 通过：创建 `{skills_dir}/{name}/SKILL.md`
4. ✅ 通过：生成 frontmatter（created_at, updated_at）
5. ✅ 通过：更新 manifest
6. ❌ 拒绝：返回 `ToolError::Execution("security scan failed: {threats}")`

**返回：**
```json
{
  "status": "ok",
  "name": "...",
  "path": "~/.config/hermes-agent/skills/{name}/"
}
```

### edit

**参数：** `name`, `field`, `value`

**field 可选值：** `description`, `triggers`, `tags`

**流程：**
1. 检查 skill 存在
2. 读取现有 SKILL.md
3. 更新对应 field
4. security_scan 仅在 content 字段时调用
5. 更新 `updated_at`
6. 写回文件

### delete

**参数：** `name`

**流程：**
1. 检查 skill 存在
2. 删除 `{skills_dir}/{name}/` 目录
3. 从 manifest 移除
4. 返回确认

### patch

**参数：** `name`, `patch_content`

**流程：**
1. 检查 skill 存在
2. 读取现有 SKILL.md
3. 将 patch_content 追加到文件末尾
4. security_scan 扫描新增内容
5. ✅ 通过：更新文件
6. ❌ 拒绝：返回错误

### scan

**参数：** `name?`（可选，不提供则扫描所有）

**返回：**
```json
{
  "scanned": 5,
  "safe": 4,
  "threats_found": 1,
  "results": [
    {
      "name": "malicious-skill",
      "safe": false,
      "threats": [
        { "pattern": "eval\\(", "line_number": 15, "severity": "High" }
      ]
    }
  ]
}
```

---

## 5. 目录结构

```
~/.config/hermes-agent/skills/
├── .bundled_manifest
├── skill-name-1/
│   └── SKILL.md
└── skill-name-2/
    └── SKILL.md
```

---

## 6. 错误处理

| 场景 | 处理 |
|------|------|
| name 已存在 | `create` 返回错误 |
| skill 不存在 | `edit/delete/patch` 返回错误 |
| security scan 失败 | 返回错误，包含威胁详情 |
| 无写权限 | `ToolError::Execution` |
| manifest 损坏 | 重建空 manifest |

---

## 7. 文件变更

- **修改：** `crates/hermes-tools-extended/src/skills.rs` — 新增 Create/Edit/Delete/Patch/Scan actions
- **创建：** `crates/hermes-tools-extended/src/skills/security_scanner.rs` — 安全扫描模块
- **创建：** `crates/hermes-tools-extended/tests/test_skills_manager.rs` — 测试
- **修改：** `crates/hermes-tools-extended/src/lib.rs` — 导出新模块

---

## 8. 验收标准

- [ ] `create` 能创建新 skill 并通过安全扫描
- [ ] `create` 在安全扫描失败时正确拒绝
- [ ] `edit` 能更新 description/triggers/tags
- [ ] `delete` 能删除 skill 并更新 manifest
- [ ] `patch` 能追加内容到现有 skill
- [ ] `scan` 能扫描单个或所有 skills
- [ ] 安全扫描能检测 eval/exec/subprocess 等模式

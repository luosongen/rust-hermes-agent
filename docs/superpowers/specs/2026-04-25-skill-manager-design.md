# Skill Manager 设计文档

## 概述

在 `hermes-skills` crate 中实现 Skill Manager 功能，允许 Agent 自主创建、编辑、删除技能，实现"从经验中学习"的闭环能力。

## 目标

- Agent 成功完成复杂任务后，可自主将解决方案保存为可复用 Skill
- 支持技能的完整生命周期管理（创建、编辑、补丁、删除）
- Skill 修改后立即生效（触发重新加载）

## 架构

```
hermes-skills/
├── src/
│   ├── lib.rs
│   ├── manager.rs          # 新增：SkillManager 核心逻辑
│   ├── tools.rs            # 新增：skill_manage tool 定义
│   ├── error.rs
│   ├── loader.rs
│   ├── metadata.rs
│   ├── registry.rs
│   ├── security.rs
│   └── hub/
│       └── ...
```

## Tool 定义

### skill_manage

单一 Tool，通过 `action` 参数区分操作。

**参数：**

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `action` | enum | 是 | `create` \| `edit` \| `patch` \| `delete` \| `write_file` \| `remove_file` |
| `name` | string | 是 | Skill 名称 |
| `content` | string | create/edit | 完整 SKILL.md 内容（YAML frontmatter + markdown body） |
| `category` | string | create (可选) | 分类目录，如 `devops`、`data-science` |
| `file_path` | string | write_file/remove_file/patch | 文件路径（相对于 skill 目录） |
| `file_content` | string | write_file | 文件内容 |
| `old_string` | string | patch | 要替换的文本 |
| `new_string` | string | patch | 替换后的文本 |
| `replace_all` | bool | patch (可选) | 是否全部替换，默认 false |

**返回值：** JSON 格式结果

```json
{
  "success": true,
  "message": "Skill 'my-skill' created.",
  "path": "devops/my-skill"
}
```

## 操作详情

### create

1. 验证名称格式（`^[a-z0-9][a-z0-9._-]*$`，最大64字符）
2. 验证 category（可选）
3. 验证 frontmatter 格式（必须有 `name` 和 `description`）
4. 检查名称冲突
5. 创建目录结构：
   ```
   ~/.hermes/skills/[category/]name/
   ├── SKILL.md
   ├── references/
   ├── templates/
   ├── scripts/
   └── assets/
   ```
6. 写入 SKILL.md
7. 触发重新加载

### edit

1. 查找现有 skill
2. 验证 frontmatter 格式
3. 完整重写 SKILL.md
4. 触发重新加载

### patch

1. 查找现有 skill 和文件（默认 SKILL.md）
2. 使用精确字符串匹配（`old_string` → `new_string`）
3. 验证修改后 frontmatter 仍然有效
4. 原子性写入
5. 触发重新加载

### delete

1. 查找 skill
2. 删除整个目录
3. 清理空 category 目录
4. 触发重新加载

### write_file

1. 验证 file_path 必须在允许的子目录（`references/`、`templates/`、`scripts/`、`assets/`）
2. 防止路径遍历攻击（不允许 `..`）
3. 原子性写入
4. 触发重新加载

### remove_file

1. 验证 file_path
2. 删除文件
3. 清理空子目录
4. 触发重新加载

## 验证规则

| 规则 | 限制 |
|------|------|
| 名称格式 | `^[a-z0-9][a-z0-9._-]*$` |
| 最大名称长度 | 64 字符 |
| 最大描述长度 | 1024 字符 |
| SKILL.md 最大大小 | 100,000 字符 |
| 单个支持文件最大 | 1 MiB |
| 允许的子目录 | `references/`、`templates/`、`scripts/`、`assets/` |

## 目录结构

Skill 存储位置：`~/.hermes/skills/`

```
~/.hermes/skills/
├── skill-a/
│   └── SKILL.md
├── category-b/
│   └── skill-c/
│       ├── SKILL.md
│       └── references/
│           └── api.md
```

## 重新加载机制

修改 skill 后，通过 `SkillRegistry::reload()` 重新扫描并加载所有 skill，确保立即生效。

## 错误处理

所有错误返回结构化 JSON：

```json
{
  "success": false,
  "error": "Skill name is required."
}
```

## 与开源兼容性

- Tool 名称：`skill_manage`（与开源一致）
- 参数结构：与开源保持兼容
- 操作语义：完全一致
- 目录结构：完全一致

## 实现顺序

1. `manager.rs` - 核心逻辑（验证、文件操作、原子写入）
2. `tools.rs` - Tool 定义和 schema
3. 集成到 `lib.rs`
4. CLI 命令（如需要）
5. 测试

# Skills Tool + Manager 设计文档

> **Status:** Approved for implementation

## 1. 概述

**目标：** 实现 Rust 版 SkillsTool + Manager，对齐 Python hermes-agent 的 skills 管理能力。

**核心功能：** 提供 local skills 的 list / view / search / sync / install / remove 操作，对接 agentskills.io 远程技能市场。

**技术方案：** 单 `~/.config/hermes-agent/skills/` 目录 + `serde_yaml` 解析 SKILL.md + `reqwest` 调用远程 API。

---

## 2. 数据结构

### SkillMetadata

从 SKILL.md 的 YAML frontmatter 解析：

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
    pub origin_hash: String,  // SHA256 of remote source
}
```

### bundled_manifest

文件：`~/.config/hermes-agent/skills/.bundled_manifest`

```json
{
  "version": 1,
  "skills": {
    "skill-name": {
      "source": "https://github.com/user/repo/skills/skill-name",
      "origin_hash": "sha256:abc123...",
      "installed_at": 1710000000.0
    }
  }
}
```

---

## 3. SKILL.md 格式

```markdown
---
name: my-skill
description: Do something useful
triggers: ["keyword", "another"]
tags: ["automation", "productivity"]
---

# My Skill

Content here...
```

解析规则：
1. 读取文件前 50 行（或直到第二个 `---`）
2. 第一个 `---` 和第二个 `---` 之间的内容为 YAML frontmatter
3. 用 `serde_yaml` 解析为 `SkillMetadata`
4. 剩余内容（第二个 `---` 之后）为 skill 说明文档（纯文本）

---

## 4. 目录结构

```
~/.config/hermes-agent/skills/
├── .bundled_manifest          # 安装清单（JSON）
├── skill-name-1/
│   └── SKILL.md               # Skill 定义文件
├── skill-name-2/
│   └── SKILL.md
└── ...
```

**自动创建：** 目录不存在时自动创建。

---

## 5. 远程 API（agentskills.io）

### 搜索

```
GET https://skills.sh/index?query=<search>&limit=<N>
```

响应：
```json
{
  "skills": [
    {
      "name": "skill-name",
      "source": "https://raw.githubusercontent.com/...",
      "origin_hash": "sha256:..."
    }
  ]
}
```

### 下载

直接从 `source` URL GET 获取 SKILL.md 内容。

---

## 6. Tools 操作

### list

读取 `.bundled_manifest`，扫描目录下所有 `{name}/SKILL.md` 文件，解析 frontmatter 返回 skill 列表。

**返回：**
```json
{
  "skills": [
    {
      "name": "skill-name",
      "description": "...",
      "triggers": ["..."],
      "tags": ["..."]
    }
  ]
}
```

### view

读取指定 skill 的 `SKILL.md`，解析 frontmatter 并返回完整元信息 + 内容摘要（前 200 字符）。

**参数：** `name` (skill 名称)

**返回：**
```json
{
  "name": "skill-name",
  "description": "...",
  "triggers": ["..."],
  "tags": ["..."],
  "content_preview": "..."
}
```

### search

调用 `https://skills.sh/index?query=<query>&limit=<limit>`，返回匹配的远程 skill 列表。

**参数：** `query` (必填), `limit` (默认 10)

**返回：**
```json
{
  "results": [
    {
      "name": "...",
      "source": "...",
      "origin_hash": "..."
    }
  ]
}
```

### sync

更新本地 `.bundled_manifest` 与远程索引同步（不下载，只更新 manifest 记录）。

**返回：** `{ "status": "ok", "synced_count": N }`

### install

从远程下载 skill，保存到 `{skill_dir}/{name}/SKILL.md`，写入 manifest。

**参数：** `name` (必填), `source` (可选，默认从 manifest 查找)

**返回：**
```json
{
  "status": "ok",
  "name": "...",
  "installed_path": "..."
}
```

**错误：** skill 已存在时返回错误。

### remove

删除本地 `{skill_dir}/{name}/` 目录，更新 manifest。

**参数：** `name` (skill 名称)

**返回：** `{ "status": "ok", "name": "..." }`

---

## 7. 错误处理

| 场景 | 处理 |
|------|------|
| 目录不存在 | 自动创建 `~/.config/hermes-agent/skills/` |
| manifest 损坏/缺失 | 备份旧文件（如存在），重建空 manifest |
| 远程 API 失败 | 返回 `ToolError::Execution`，包含网络错误信息 |
| skill 已存在 | `install` 返回错误："already installed" |
| SKILL.md 解析失败 | 跳过该文件，WARN 日志到 stderr |
| 离线模式 | `sync` / `search` 返回错误；`list` / `view` / `install` / `remove` 正常（读本地） |

---

## 8. 文件变更

- **新建：** `crates/hermes-tools-extended/src/skills.rs`
- **修改：** `crates/hermes-tools-extended/src/lib.rs` — 添加 `pub mod skills;`
- **修改：** `crates/hermes-tools-extended/src/lib.rs` — `register_extended_tools()` 中添加 `registry.register(SkillsTool::new());`
- **新增依赖：** `serde_yaml`（如尚未引入）

---

## 9. 测试策略

- `cargo test -p hermes-tools-extended` 运行内置测试
- 手动测试场景：
  1. `list` 空目录 → 返回空数组
  2. `install` 一个远程 skill → 验证目录创建 + manifest 更新
  3. `view` 已安装 skill → 验证元信息解析正确
  4. `remove` 已安装 skill → 验证目录删除 + manifest 更新
  5. `search` 离线 → 返回网络错误

---

## 10. 验收标准

- [ ] `list` 能列出本地已安装的所有 skills
- [ ] `view` 能解析 SKILL.md frontmatter 并返回完整元信息
- [ ] `search` 能从 `https://skills.sh` 搜索并返回结果
- [ ] `sync` 能更新本地 manifest
- [ ] `install` 能下载并安装 skill 到本地目录
- [ ] `remove` 能删除本地 skill 并更新 manifest
- [ ] 离线时 `list` / `view` / `install` / `remove` 正常工作
- [ ] 目录不存在时自动创建
- [ ] 所有操作返回结构化的 JSON 结果
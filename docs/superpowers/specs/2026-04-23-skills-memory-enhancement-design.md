# Hermes Agent Skills & Memory Enhancement Design

**Date**: 2026-04-23
**Status**: Draft
**Author**: Claude

---

## 1. Overview

This document describes the design for enhancing the rust-hermes-agent with two major systems:
- **Skills System**: Full-featured skill management with self-improvement capabilities
- **Memory System**: Multi-layered memory with nudge reminders, session search, and pluggable user modeling

These enhancements bring rust-hermes-agent closer to feature parity with the Python hermes-agent.

---

## 2. Skills System

### 2.1 Architecture

```
hermes-skills/              # New crate
├── src/
│   ├── lib.rs             # Public exports
│   ├── skill.rs           # Skill struct, Frontmatter
│   ├── registry.rs        # SkillRegistry (list/view)
│   ├── tools.rs           # skills_list, skills_view, skills_manage
│   ├── fuzzy_patch.rs     # FuzzyPatch engine
│   ├── security.rs        # Security scanner
│   ├── sync.rs           # Bundled skills sync
│   └── hub.rs             # Hub registry download
├── skills/                # Bundled skills (copied to ~/.hermes/skills/)
└── Cargo.toml
```

### 2.2 Skill Format

YAML frontmatter + Markdown body, compatible with agentskills.io standard:

```yaml
---
name: skill-name
description: Generate spectrograms and audio feature visualizations...
version: 1.0.0
author: community
license: MIT
platforms: [macos, linux]
prerequisites:
  env_vars: [TENOR_API_KEY]
  commands: [curl, jq]
metadata:
  hermes:
    tags: [Audio, Visualization]
    related_skills: [some-other-skill]
    config:
      - key: wiki.path
        description: Path to the LLM Wiki knowledge base
        default: "~/wiki"
---

# Skill Title

Full markdown content with instructions, commands, examples...
```

**Required frontmatter fields:**
- `name`: string (max 64 chars)
- `description`: string (max 1024 chars)

**Optional frontmatter fields:**
- `version`: semver string
- `author`: string
- `license`: SPDX string
- `platforms`: array of ["macos", "linux", "windows"]
- `prerequisites.env_vars`: array of required env var names
- `prerequisites.commands`: array of required shell commands
- `metadata.hermes.tags`: array of searchable tags
- `metadata.hermes.related_skills`: array of related skill names
- `metadata.hermes.config`: array of skill-declared config variables

### 2.3 Core Tools

| Tool | Description |
|------|-------------|
| `skills_list(category)` | Returns minimal metadata (name, description, category). Token-efficient enumeration. |
| `skills_view(name, file_path?)` | Loads full SKILL.md content. Optional file_path for linked files. |
| `skills_manage(action, name, ...)` | CRUD for self-improvement: create, edit, patch, delete, write_file, remove_file |

### 2.4 Self-Improvement Mechanism

**Fuzzy Patch Flow:**
```
Agent detects issue → skill_manage(patch, old_string, new_string) → fuzzy_match定位 → 精确替换 → 保存
```

**Fuzzy matching handles:**
- Whitespace normalization
- Indentation flexibility
- Block-anchor matching
- Returns preview on failure to help agent self-correct

**Implementation:** Use `fuzzy-match` crate.

### 2.5 Security Scanner

Port patterns from `skills_guard.py`:
- **Exfiltration**: Secret env var exfil via curl/wget/fetch
- **Prompt injection**: `ignore previous instructions`, role hijack
- **Destructive ops**: `rm -rf /`, filesystem formats
- **Persistence**: Crontab, SSH keys, systemd
- **Network**: Reverse shells, tunneling services
- **Obfuscation**: Base64 decode pipes, eval()
- **Credential exposure**: Hardcoded API keys, private keys

### 2.6 Bundled Skills Sync

- Track bundled skills with MD5 hashes in `~/.hermes/skills/.bundled_manifest`
- User modifications detected by hash mismatch → preserved during updates
- Safe auto-update when user hasn't modified

### 2.7 Hub Integration

- Download community skills from GitHub registries via GitHub contents API
- Track provenance with `~/.hermes/skills/.hub/lock.json`

---

## 3. Memory System

### 3.1 Architecture

```
hermes-memory/             # New crate (or enhance existing)
├── src/
│   ├── lib.rs             # Public exports
│   ├── provider.rs        # MemoryProvider trait
│   ├── builtin/
│   │   ├── mod.rs        # BuiltinMemoryProvider
│   │   ├── memory_store.rs  # MEMORY.md/USER.md storage
│   │   └── injection_scan.rs # Security scanning
│   ├── nudge/
│   │   ├── mod.rs        # Nudge system
│   │   └── background.rs # Background review agent
│   ├── search/
│   │   ├── mod.rs        # Session search
│   │   ├── fts.rs        # FTS5 query sanitization
│   │   └── summarizer.rs # LLM summarization
│   ├── honcho/
│   │   ├── mod.rs        # HonchoProvider (trait impl)
│   │   ├── client.rs     # HonchoClient
│   │   └── session.rs    # HonchoSessionManager
│   └── manager.rs        # MemoryManager orchestration
```

### 3.2 MemoryProvider Trait

```rust
pub trait MemoryProvider: Send + Sync {
    fn name(&self) -> &str;
    fn is_available(&self) -> bool;
    fn initialize(&self, session_id: &str) -> Result<()>;
    fn get_tool_schemas(&self) -> Vec<ToolSchema>;
    fn handle_tool_call(&self, tool_call: &ToolCall) -> Result<ToolResult>;

    // Optional hooks
    fn system_prompt_block(&self) -> Option<String>;
    fn prefetch(&self, query: &str, session_id: &str) -> Result<Option<String>>;
    fn sync_turn(&self, turn: &Turn) -> Result<()>;
    fn on_turn_start(&self, ctx: &TurnContext) -> Result<()>;
    fn on_session_end(&self, session_id: &str) -> Result<()>;
    fn on_pre_compress(&self, messages: &[Message]) -> Result<Option<String>>;
}
```

### 3.3 Built-in Memory Store

**File format:** `MEMORY.md` and `USER.md` with `§` delimiter.

```
§
[记忆条目1 - Agent的环境事实、项目约定、工具特性]
§
[记忆条目2]
§
[用户相关信息 - 用户偏好、沟通风格、习惯]
```

**Constraints:**
- MEMORY.md: 2200 chars limit
- USER.md: 1375 chars limit

**Features:**
- Entry delimiter: `§` (section sign, `\n§\n`)
- Atomic write via temp file + `os.replace()`
- File locking via `.lock` sidecar
- Injection/exfiltration scanning (blocks invisible unicode, prompt injection patterns)
- Deduplication on load (preserves first occurrence)
- Frozen snapshot pattern: loaded once at session start for prefix cache stability

### 3.4 Memory Nudge System

**Trigger:** Every N user turns (default: 10, configurable via `memory.nudge_interval`)

**Flow:**
```
每N轮 → 后台Agent fork (same model/tools/context) → 审查对话历史 → 调用memory工具 → 更新MEMORY.md/USER.md → 打印通知
```

**Background review prompt focuses on:**
- User persona and preferences
- Personal details and behavioral expectations
- Non-trivial approaches from experiential learning

### 3.5 Session Search

**Flow:**
1. FTS5 search finds matching messages (up to 50 results)
2. Groups by resolved parent session (follows parent_session_id chains)
3. Excludes current session lineage
4. Takes top N unique sessions (default 3, max 5)
5. Loads full conversation, truncates to 100k chars centered on query matches
6. Sends to LLM with summarization prompt
7. Returns per-session summaries with metadata

**Query sanitization:**
- Strip FTS5-special characters
- Wrap hyphenated/dotted terms in quotes
- Preserve balanced quoted phrases

### 3.6 Honcho Provider (Pluggable)

**Interface:** Implements `MemoryProvider` trait

**Tools (via MemoryProvider interface):**
| Tool | LLM? | Description |
|------|------|-------------|
| `honcho_profile` | No | User's peer card — key facts snapshot |
| `honcho_search` | No | Semantic search over stored context |
| `honcho_context` | Yes | LLM-synthesized answer via dialectic reasoning |
| `honcho_conclude` | No | Write persistent fact about the user |

**Architecture:**
- Uses Honcho SDK for AI-native cross-session user modeling
- Dual peer model: user peer + AI peer
- Async prefetch: fires `dialectic_query` and `prefetch_context` at turn end in background threads
- Consumed next turn (zero latency on response path)

### 3.7 Context Compressor

**Trigger:** When `prompt_tokens >= threshold_tokens` (50% of context window)

**Algorithm:**
1. Prune old tool results (>200-char outputs replaced with placeholder)
2. Protect head messages (system + first N exchanges)
3. Find tail boundary by token budget (~20% of context length)
4. Summarize middle turns with LLM prompt
5. On re-compression, iteratively update previous summary

**Summary template:**
```
## Goal
## Constraints & Preferences
## Progress (Done / In Progress / Blocked)
## Key Decisions
## Resolved Questions
## Pending User Asks
## Relevant Files
## Remaining Work
## Critical Context
## Tools & Patterns
```

---

## 4. Implementation Phases

### Phase 1: Skills Core (P0)
**hermes-skills crate**
- [ ] Skill struct with YAML frontmatter parsing
- [ ] SkillRegistry (list/view)
- [ ] skills_list, skills_view, skills_manage tools
- [ ] FuzzyPatch engine (fuzzy-match crate)
- [ ] Security scanner (port from skills_guard.py)
- [ ] Bundled skills sync

### Phase 2: Memory Core (P0)
**hermes-memory crate**
- [ ] MemoryProvider trait
- [ ] BuiltinMemoryProvider (MEMORY.md/USER.md)
- [ ] MemoryStore (atomic write, injection scan)
- [ ] MemoryManager orchestration
- [ ] Integration with existing SessionStore

### Phase 3: Nudge + Search (P1)
**Memory enhancements**
- [ ] Nudge background review system
- [ ] Session search (FTS5 + LLM summarization)
- [ ] Context compressor
- [ ] CLI commands (hermes memory *)

### Phase 4: Honcho Provider (P2)
**Optional enhancement**
- [ ] HonchoProvider trait implementation
- [ ] HonchoClient (SDK integration)
- [ ] SessionManager
- [ ] Dialectic reasoning

---

## 5. Dependencies

### hermes-skills
- `serde_yaml`: YAML parsing
- `fuzzy-match`: Fuzzy patch engine
- `regex`: Security pattern matching
- `walkdir`: Directory traversal
- `tokio`: Async runtime
- `reqwest`: Hub download

### hermes-memory
- `tokio`: Async runtime
- `serde`: Serialization
- Existing `hermes-memory` SQLite dependencies

---

## 6. CLI Commands

```bash
hermes skills list|search|install|uninstall
hermes skills sync          # Sync bundled skills
hermes memory show|add|remove
hermes memory search <query>
```

---

## 7. Configuration

```toml
[skills]
# skills config

[memory]
nudge_interval = 10         # Turns between nudge reminders
# honcho provider config (optional)
```

---

## 8. Acceptance Criteria

### Skills System
- [ ] Skills can be listed with metadata (name, description, category)
- [ ] Full skill content can be viewed with linked files
- [ ] Agent can create, edit, patch, delete skills
- [ ] Fuzzy patch handles whitespace/indentation differences
- [ ] Security scanner blocks dangerous patterns
- [ ] Bundled skills sync works correctly

### Memory System
- [ ] Built-in memory (MEMORY.md/USER.md) persists correctly
- [ ] Memory nudges fire every N turns
- [ ] Session search returns relevant history with LLM summaries
- [ ] Context compression works when context is large
- [ ] Honcho provider is pluggable and configurable

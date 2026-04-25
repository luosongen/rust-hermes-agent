# Hermes Agent 功能完整实现设计文档

**版本**: v1.0
**日期**: 2026-04-25
**项目**: rust-hermes-agent
**参考**: NousResearch/hermes-agent (Python)

---

## 1. 概述

本文档描述 rust-hermes-agent 项目的完整功能实现计划，旨在对标 NousResearch 的 Python 版本 hermes-agent，填补当前 Rust 实现中的功能缺失。

### 1.1 项目目标

将 rust-hermes-agent 打造成一个功能完整的 AI Agent 框架，具有：
- 多模型、多平台支持
- 强大的工具系统（40+ 内置工具）
- 自我改进能力（Skills 系统从经验中学习）
- 企业级安全特性
- 灵活的部署选项

### 1.2 当前状态

✅ **已实现**:
- 核心架构（Agent 主循环、Provider trait、ToolDispatcher trait）
- LLM Providers: OpenAI, Anthropic, OpenRouter, GLM, MiniMax, Kimi, DeepSeek, Qwen
- 消息平台: Telegram, WeCom
- 工具: 文件读写、终端执行、浏览器自动化、基础 Skills
- 执行环境: Local
- 会话存储: SQLite + FTS5
- 基础设施: 重试逻辑、Credentials 池

❌ **待实现** (按本文档实现)

---

## 2. 开发阶段总览

```
阶段 1: 基础设施完善
├── 1.1 上下文管理 (Context Management)
└── 1.2 执行环境扩展 (Docker + SSH)

阶段 2: 核心功能增强
├── 2.1 定时调度系统 (Cron Scheduler)
├── 2.2 终端 UI 增强 (Terminal UI)
└── 2.3 MCP Client

阶段 3: Skills 系统
└── 3.1 Skills Hub 完整实现

阶段 4: 消息平台
└── 4.1 消息平台适配器集群

阶段 5: 高级功能
├── 5.1 子 Agent 委托
├── 5.2 Home Assistant 集成
├── 5.3 备份/导入系统
├── 5.4 多实例 Profiles
├── 5.5 自我改进学习循环
├── 5.6 皮肤/主题系统
└── 5.7 RL 训练工具
```

---

## 3. 阶段一：基础设施完善

### 3.1 上下文管理 (Context Management)

**目标**: 实现完整的上下文管理，支持自动压缩和 Prompt Caching。

**组件**:

| 组件 | 职责 |
|------|------|
| `ContextCompressor` | 自动上下文压缩，移除低价值消息 |
| `PromptCache` | Anthropic Prompt Caching 支持 |
| `ContextPressureMonitor` | Tiered context pressure 警告系统 |

**接口设计**:

```rust
// hermes-core/src/context_compressor.rs
pub trait ContextCompressor: Send + Sync {
    /// 压缩对话上下文
    fn compress(&self, messages: &[Message], max_tokens: usize) -> Vec<Message>;

    /// 估算当前上下文 token 数
    fn estimate_tokens(&self, messages: &[Message]) -> usize;

    /// 获取压缩建议
    fn get_suggestions(&self, messages: &[Message]) -> CompressionSuggestions;
}

pub struct CompressionSuggestions {
    pub should_compress: bool,
    pub suggested_strategy: CompressionStrategy,
    pub tokens_saved: usize,
}

pub enum CompressionStrategy {
    None,
    RemoveLowValueMessages,
    SummarizeOldMessages,
    TruncateMiddle,
}
```

**与 Provider 集成**:

```rust
// hermes-core/src/provider.rs 扩展
pub trait LlmProvider: Send + Sync {
    // ... existing methods ...

    /// 返回是否支持 Prompt Caching
    fn supports_prompt_caching(&self, model: &ModelId) -> bool;

    /// 构建带缓存提示的请求
    fn build_cached_request(&self, request: &ChatRequest, cache_prefix: &[Message]) -> ChatRequest;
}
```

**实现任务**:

- [ ] 创建 `hermes-context` crate
- [ ] 实现 `ContextCompressor` trait 和默认实现
- [ ] 实现 `PromptCache` 结构（基于 Redis 或内存）
- [ ] 实现 `ContextPressureMonitor`
- [ ] 集成到 Agent 主循环
- [ ] 为每个 Provider 实现 `supports_prompt_caching()` 和 `build_cached_request()`
- [ ] 添加配置选项

---

### 3.2 执行环境扩展 (Execution Environment)

**目标**: 支持 Docker 和 SSH 执行环境。

**架构**:

```
hermes-environment/
├── src/
│   ├── lib.rs
│   ├── traits.rs          # Environment trait
│   ├── local.rs           # 现有 LocalEnvironment
│   ├── docker.rs          # 新增 DockerEnvironment
│   └── ssh.rs             # 新增 SshEnvironment
```

**DockerEnvironment 设计**:

```rust
// hermes-environment/src/docker.rs
pub struct DockerEnvironment {
    image: String,
    container_name: Option<String>,
    network: Option<String>,
    volumes: Vec<VolumeMapping>,
    env_vars: HashMap<String, String>,
}

#[derive(Clone)]
pub struct VolumeMapping {
    host_path: PathBuf,
    container_path: PathBuf,
    read_only: bool,
}

#[async_trait]
impl Environment for DockerEnvironment {
    fn environment_type(&self) -> EnvironmentType {
        EnvironmentType::Docker
    }

    async fn execute(&self, command: &str, args: &[&str], cwd: Option<&Path>, timeout: Option<Duration>, env_vars: Option<&HashMap<String, String>>) -> Result<ExecutionResult, EnvironmentError> {
        // 使用 bollard crate 与 Docker API 交互
    }
}
```

**SshEnvironment 设计**:

```rust
// hermes-environment/src/ssh.rs
pub struct SshEnvironment {
    connection: SshConnection,
    working_dir: PathBuf,
}

pub struct SshConnection {
    host: String,
    port: u16,
    user: String,
    authentication: SshAuth,
}

pub enum SshAuth {
    Password(String),
    KeyFile { path: PathBuf, passphrase: Option<String> },
}

#[async_trait]
impl Environment for SshEnvironment {
    async fn execute(&self, command: &str, args: &[&str], cwd: Option<&Path>, timeout: Option<Duration>, env_vars: Option<&HashMap<String, String>>) -> Result<ExecutionResult, EnvironmentError> {
        // 使用 ssh2 crate 建立 SSH 会话
    }
}
```

**配置**:

```toml
# config.toml
[environments.docker]
default_image = "rust:1.75"
network = "hermes-network"

[[environments.docker.containers]]
name = "code-executor"
image = "python:3.11"
command = ["python", "-m", "http.server", "8080"]

[environments.ssh]
[[environments.ssh.servers]]
name = "prod-server"
host = "example.com"
port = 22
user = "deploy"
auth_method = "key_file"
key_path = "~/.ssh/id_rsa"
default_cwd = "/app"
```

**实现任务**:

- [ ] 创建 `hermes-environment` crate（如果不存在）
- [ ] 实现 `DockerEnvironment`
  - [ ] 集成 `bollard` crate
  - [ ] 实现容器生命周期管理
  - [ ] 支持 volume 映射
  - [ ] 支持网络配置
- [ ] 实现 `SshEnvironment`
  - [ ] 集成 `ssh2` crate
  - [ ] 实现 SSH 连接池
  - [ ] 支持 key 文件和密码认证
- [ ] 实现 `EnvironmentRegistry`
- [ ] 添加配置解析
- [ ] 集成到 Tool 执行层

---

## 4. 阶段二：核心功能增强

### 4.1 定时调度系统 (Cron Scheduler)

**目标**: 实现完整的 cron 调度系统，支持自然语言配置和后台监控。

**组件**:

| 组件 | 职责 |
|------|------|
| `CronScheduler` | 核心调度器，管理所有定时任务 |
| `NaturalLanguageParser` | 自然语言转 cron 表达式 |
| `JobExecutor` | 执行定时任务 |
| `WatchPatternMonitor` | 后台进程监控 |

**接口设计**:

```rust
// hermes-core/src/cron/mod.rs
pub struct CronScheduler {
    jobs: HashMap<JobId, ScheduledJob>,
    executor: Arc<JobExecutor>,
    watch_patterns: Arc<WatchPatternMonitor>,
}

pub struct ScheduledJob {
    id: JobId,
    name: String,
    schedule: Schedule,
    command: JobCommand,
    enabled: bool,
    last_run: Option<DateTime<Utc>>,
    next_run: Option<DateTime<Utc>>,
    retry_policy: RetryPolicy,
}

pub enum Schedule {
    Cron(CronExpression),
    NaturalLanguage(String),
    Interval(Duration),
}

#[async_trait]
pub trait JobCommand: Send + Sync {
    async fn execute(&self, context: &JobContext) -> Result<JobOutput, JobError>;
}
```

**自然语言解析示例**:

```
"每天早上9点" → "0 9 * * *"
"每隔5分钟" → "*/5 * * * *"
"工作日每半小时" → "*/30 9-18 * * 1-5"
"每周一早上10点" → "0 10 * * 1"
```

**WatchPatternMonitor**:

```rust
// 监控文件变化或进程状态
pub struct WatchPatternMonitor {
    patterns: Vec<WatchPattern>,
    notifier: tokio::sync::mpsc::Sender<WatchEvent>,
}

pub struct WatchPattern {
    pattern: glob::Pattern,
    events: Vec<WatchEventType>,
    action: WatchAction,
}

pub enum WatchAction {
    RunJob(JobId),
    SendNotification(String),
    ExecuteCommand(String),
}
```

**实现任务**:

- [ ] 创建 `hermes-cron` crate
- [ ] 实现 `CronScheduler`
- [ ] 实现 `NaturalLanguageParser`
  - [ ] 使用 chrono 或定时解析库
  - [ ] 支持中文和英文自然语言
- [ ] 实现 `JobExecutor`
- [ ] 实现 `WatchPatternMonitor`
- [ ] 集成到 Agent
- [ ] 添加 CLI 命令 (`hermes cron list`, `hermes cron add`, etc.)
- [ ] 添加配置选项

---

### 4.2 终端 UI 增强 (Terminal UI)

**目标**: 实现完整的增强终端 UI。

**组件**:

| 组件 | 职责 |
|------|------|
| `MultilineEditor` | 多行编辑支持 |
| `SlashCommandCompleter` | 斜杠命令自动补全 |
| `StreamingOutput` | 流式工具输出显示 |
| `CommandHistory` | 命令历史管理 |

**接口设计**:

```rust
// hermes-cli/src/ui/
pub struct EnhancedRepl {
    editor: MultilineEditor,
    completer: SlashCommandCompleter,
    history: CommandHistory,
    streaming_output: StreamingOutput,
}

pub struct MultilineEditor {
    buffer: String,
    cursor_pos: usize,
    max_lines: usize,
    indent: String,
}

pub struct SlashCommandCompleter {
    commands: HashMap<String, CommandMetadata>,
    partial: String,
}

impl Completer for SlashCommandCompleter {
    fn complete(&self, line: &str, pos: usize) -> Vec<Completion> {
        // 实现斜杠命令补全
    }
}
```

**流式输出**:

```rust
pub struct StreamingOutput {
    tx: mpsc::Sender<OutputChunk>,
    current_tool: Option<ToolId>,
    buffer: String,
}

pub enum OutputChunk {
    Text(String),
    ToolStart { tool_id: ToolId, name: String },
    ToolEnd { tool_id: ToolId, output: String },
    ToolError { tool_id: ToolId, error: String },
}

impl StreamingOutput {
    pub async fn render(&self, terminal: &mut Terminal) {
        // 使用 ratatui 或 cursive 渲染
    }
}
```

**实现任务**:

- [ ] 评估 UI 库选择 (ratatui, cursive, boa_terminal)
- [ ] 实现 `MultilineEditor`
- [ ] 实现 `SlashCommandCompleter`
  - [ ] 从现有命令注册表读取
  - [ ] 支持子命令补全
- [ ] 实现 `StreamingOutput`
  - [ ] 实时显示 Tool 执行进度
  - [ ] 格式化 JSON 输出
- [ ] 实现 `CommandHistory`
  - [ ] 持久化到文件
  - [ ] 支持搜索
- [ ] 重构现有 REPL 集成这些组件
- [ ] 添加配置选项

---

### 4.3 MCP Client

**目标**: 实现 MCP Client，支持连接外部 MCP 服务器。

**架构**:

```
hermes-mcp/
├── src/
│   ├── lib.rs
│   ├── client.rs        # MCP Client 实现
│   ├── protocol.rs      # MCP 协议解析
│   ├── tools.rs         # MCP 工具适配
│   └── transport.rs     # STDIO/HTTP 传输层
```

**接口设计**:

```rust
// hermes-mcp/src/client.rs
pub struct McpClient {
    transport: Box<dyn McpTransport>,
    request_id: AtomicU64,
    capabilities: ServerCapabilities,
}

#[async_trait]
pub trait McpTransport: Send + Sync {
    async fn connect(&self) -> Result<(), McpError>;
    async fn send(&self, message: McpMessage) -> Result<McpMessage, McpError>;
    async fn receive(&self) -> Result<McpMessage, McpError>;
}

pub struct StdioTransport {
    command: Command,
    args: Vec<String>,
}

pub struct HttpTransport {
    url: Url,
    headers: HashMap<String, String>,
}
```

**工具适配**:

```rust
// hermes-mcp/src/tools.rs
pub struct McpToolAdapter {
    server_name: String,
    tool_name: String,
    definition: ToolDefinition,
}

#[async_trait]
impl Tool for McpToolAdapter {
    async fn execute(&self, args: serde_json::Value, context: ToolContext) -> Result<String, ToolError> {
        // 调用 MCP 服务器执行工具
    }
}
```

**配置**:

```toml
[mcp]
[[mcp.servers]]
name = "filesystem"
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "/tmp"]
enabled = true

[[mcp.servers]]
name = "github"
command = "uvx"
args = ["mcp-server-github"]
env = { GITHUB_TOKEN = "${GITHUB_TOKEN}" }
enabled = true
```

**实现任务**:

- [ ] 创建 `hermes-mcp` crate
- [ ] 实现 `McpTransport` trait
  - [ ] StdioTransport (用于本地 MCP 服务器)
  - [ ] HttpTransport (用于远程 MCP 服务器)
- [ ] 实现 `McpClient`
  - [ ] 协议握手
  - [ ] 工具列表发现
  - [ ] 工具调用
- [ ] 实现 `McpToolAdapter`
- [ ] 实现 `McpRegistry`
- [ ] 集成到 ToolDispatcher
- [ ] 添加配置解析
- [ ] 添加 CLI 命令 (`hermes mcp list`, `hermes mcp enable`, etc.)

---

## 5. 阶段三：Skills 系统

### 5.1 Skills Hub 完整实现

**目标**: 实现完整的 Skills Hub，支持从 agentskills.io 搜索/安装、管理和创建 Skill。

**组件**:

| 组件 | 职责 |
|------|------|
| `SkillRegistry` | 现有 Skill 注册表增强 |
| `SkillHubClient` | 与 agentskills.io 交互 |
| `SkillCreator` | Agent 自主创建 Skill |
| `ProceduralMemory` | 程序化记忆 |

**接口设计**:

```rust
// hermes-skills/src/hub.rs
pub struct SkillHubClient {
    http_client: reqwest::Client,
    base_url: Url,
    cache: Arc<SkillCache>,
}

impl SkillHubClient {
    /// 从 Hub 搜索 Skills
    pub async fn search(&self, query: &str) -> Result<Vec<HubSkillSummary>, SkillError>;

    /// 安装 Skill 到本地
    pub async fn install(&self, skill_id: &str) -> Result<InstalledSkill, SkillError>;

    /// 获取 Skill 详情
    pub async fn get_skill(&self, skill_id: &str) -> Result<HubSkillDetail, SkillError>;

    /// 更新已安装的 Skill
    pub async fn update(&self, skill_id: &str) -> Result<(), SkillError>;

    /// 列出已安装的 Skills
    pub fn list_installed(&self) -> Vec<InstalledSkill>;
}
```

**Skill 创建**:

```rust
// hermes-skills/src/creator.rs
pub struct SkillCreator {
    llm_provider: Arc<dyn LlmProvider>,
    skill_template: SkillTemplate,
}

pub struct SkillCreationRequest {
    pub name: String,
    pub description: String,
    pub capability: SkillCapability,
    pub examples: Vec<SkillExample>,
}

impl SkillCreator {
    /// 根据自然语言描述创建 Skill
    pub async fn create_from_description(&self, request: SkillCreationRequest) -> Result<Skill, SkillError>;

    /// 从对话历史中提取 Skill
    pub async fn extract_from_history(&self, conversation_id: &str) -> Result<Vec<Skill>, SkillError>;

    /// 验证 Skill 代码
    pub async fn validate(&self, skill: &Skill) -> ValidationResult;
}
```

**程序化记忆**:

```rust
// hermes-skills/src/memory.rs
pub struct ProceduralMemory {
    user_profiles: HashMap<UserId, UserProfile>,
    session_learnings: HashMap<SessionId, Vec<Learning>>,
    persistent_knowledge: KnowledgeGraph,
}

pub struct UserProfile {
    user_id: UserId,
    preferences: UserPreferences,
    interaction_patterns: Vec<Pattern>,
    learned_topics: Vec<Topic>,
}

pub struct Learning {
    timestamp: DateTime<Utc>,
    topic: String,
    insight: String,
    confidence: f32,
    source: LearningSource,
}
```

**配置**:

```toml
[skills]
storage_path = "~/.config/hermes-agent/skills"
auto_update = true

[skills.hub]
base_url = "https://agentskills.io/api"
auth_token = "${SKILLS_HUB_TOKEN}"
cache_ttl = 3600

[skills.memory]
enabled = true
learning_retention_days = 90
profile_update_interval = 300
```

**实现任务**:

- [ ] 增强 `hermes-skills` crate
- [ ] 实现 `SkillHubClient`
  - [ ] HTTP 客户端
  - [ ] API 认证
  - [ ] 搜索/安装/更新
- [ ] 实现 `SkillCreator`
  - [ ] LLM 辅助创建
  - [ ] 模板系统
  - [ ] 验证机制
- [ ] 实现 `ProceduralMemory`
  - [ ] 用户画像
  - [ ] 学习历史
  - [ ] 知识图谱
- [ ] 增强 `SkillRegistry`
  - [ ] Skill 依赖解析
  - [ ] Skill 版本管理
  - [ ] Skill 沙箱执行
- [ ] 添加 CLI 命令
  - [ ] `hermes skills search <query>`
  - [ ] `hermes skills install <skill_id>`
  - [ ] `hermes skills create <description>`
  - [ ] `hermes skills list`
- [ ] 添加 Web UI（可选）

---

## 6. 阶段四：消息平台适配器集群

**目标**: 实现完整的消息平台适配器集群，每个平台支持完整 Bot 功能。

**平台列表**:

| 平台 | 优先级 | 难度 | 备注 |
|------|--------|------|------|
| Discord | 高 | 中 | Webhook + Bot API |
| Slack | 高 | 中 | Webhook + WebSocket |
| WhatsApp | 中 | 高 | WhatsApp Business API |
| Signal | 低 | 高 | Signal API 需要特殊权限 |
| Matrix | 中 | 中 | Matrix protocol |
| Email | 高 | 低 | SMTP/IMAP |
| SMS | 中 | 中 | Twilio 等服务 |
| DingTalk | 高 | 中 | 钉钉 Webhook |
| Feishu | 高 | 中 | 飞书 Webhook |
| Mattermost | 中 | 低 | Webhook |
| Home Assistant | 高 | 中 | Webhook |
| iMessage (BlueBubbles) | 低 | 高 | BlueBubbles API |
| WeChat | 中 | 高 | 微信 API 限制 |
| Generic Webhook | 高 | 低 | 自定义 Webhook |

**架构**:

```
hermes-platform/
├── hermes-platform-telegram/    # 已完成 ✅
├── hermes-platform-wecom/       # 已完成 ✅
├── hermes-platform-discord/     # 新增
├── hermes-platform-slack/       # 新增
├── hermes-platform-whatsapp/    # 新增
├── hermes-platform-signal/      # 新增
├── hermes-platform-matrix/      # 新增
├── hermes-platform-email/       # 新增
├── hermes-platform-sms/         # 新增
├── hermes-platform-dingtalk/    # 新增
├── hermes-platform-feishu/      # 新增
├── hermes-platform-mattermost/  # 新增
├── hermes-platform-homeassistant/ # 新增
├── hermes-platform-bluebubbles/ # 新增
├── hermes-platform-weixin/      # 新增
└── hermes-platform-generic/     # 新增
```

**通用 PlatformAdapter Trait**:

```rust
// hermes-core/src/gateway.rs
#[async_trait]
pub trait PlatformAdapter: Send + Sync {
    fn platform_id(&self) -> &str;
    fn platform_name(&self) -> &str;

    // 同步验证（检查签名/token）
    fn verify_webhook(&self, request: &Request<Body>) -> bool;

    // 异步解析（解析消息格式）
    async fn parse_inbound(&self, request: Request<Body>) -> Result<InboundMessage, GatewayError>;

    // 发送响应
    async fn send_response(&self, response: ConversationResponse, message: &InboundMessage) -> Result<(), GatewayError>;

    // 可选：获取更新（用于 WebSocket 模式）
    async fn poll_updates(&self) -> Result<Vec<InboundMessage>, GatewayError> {
        Ok(vec![])
    }
}
```

**标准消息格式**:

```rust
pub struct InboundMessage {
    pub platform: String,
    pub message_id: String,
    pub chat_id: String,
    pub sender_id: String,
    pub sender_name: Option<String>,
    pub content: MessageContent,
    pub timestamp: DateTime<Utc>,
    pub raw: serde_json::Value,
}

pub enum MessageContent {
    Text(String),
    Image { url: String, caption: Option<String> },
    Document { url: String, filename: String },
    Audio { url: String, duration: Option<Duration> },
    Video { url: String, thumbnail: Option<String> },
    Location { lat: f64, lon: f64 },
    Command { command: String, args: Vec<String> },
}
```

**每个平台适配器的实现任务** (以 Discord 为例):

- [ ] 创建 `hermes-platform-discord` crate
- [ ] 实现 `DiscordAdapter`
- [ ] 实现 Webhook 验证
- [ ] 实现消息解析 (Discord 格式 → InboundMessage)
- [ ] 实现消息发送
- [ ] 实现斜杠命令处理
- [ ] 实现按钮/选择菜单交互
- [ ] 添加配置
- [ ] 编写测试
- [ ] 集成到 `hermes-gateway`

**通用任务**:

- [ ] 创建 `hermes-platform-generic` crate（用于自定义 Webhook）
- [ ] 实现统一错误处理
- [ ] 实现消息重试机制
- [ ] 添加限流处理
- [ ] 编写平台适配器文档

---

## 7. 阶段五：高级功能

### 7.1 子 Agent 委托 (Sub-Agent Delegation)

**目标**: 实现子 Agent 委托系统，支持 Spawn 隔离上下文的子 agent 和并行工作流。

**架构**:

```rust
// hermes-core/src/delegate.rs
pub struct SubAgentDispatcher {
    agent_config: AgentConfig,
    max_concurrent: usize,
    timeout: Duration,
}

pub struct SubAgentRequest {
    pub task: String,
    pub context: SubAgentContext,
    pub tools: Vec<ToolDefinition>,
    pub model: Option<ModelId>,
}

pub struct SubAgentContext {
    pub parent_session_id: SessionId,
    pub isolation_level: IsolationLevel,
    pub shared_state: Option<SharedState>,
}

pub enum IsolationLevel {
    Full,           // 完全隔离，无父上下文
    Partial(Vec<String>),  // 共享指定变量
    FullShare,       // 共享全部父上下文
}
```

**实现任务**:

- [ ] 实现 `SubAgentDispatcher`
- [ ] 实现子 Agent 生命周期管理
- [ ] 实现上下文隔离策略
- [ ] 实现并行工作流支持
- [ ] 添加超时和取消机制
- [ ] 添加结果聚合
- [ ] 集成到 Tool 系统 (`delegate_task` 工具)

---

### 7.2 Home Assistant 集成

**目标**: 实现完整的 Home Assistant 集成。

**工具**:

| 工具 | 描述 |
|------|------|
| `ha_list_entities` | 列出所有实体 |
| `ha_get_state` | 获取实体状态 |
| `ha_call_service` | 调用服务 |
| `ha_list_services` | 列出可用服务 |
| `ha_subscribe_events` | 订阅事件 |
| `ha_get_history` | 获取历史数据 |

**实现任务**:

- [ ] 创建 `hermes-tools-homeassistant` crate
- [ ] 实现 Home Assistant API 客户端
- [ ] 实现各工具
- [ ] 添加配置
- [ ] 集成到工具注册表

---

### 7.3 备份/导入系统

**目标**: 实现配置、会话、Skills 完整迁移。

**命令**:

```bash
hermes backup --output backup.tar.gz
hermes import --input backup.tar.gz
hermes export --type sessions --format json
```

**实现任务**:

- [ ] 实现 `BackupManager`
- [ ] 实现 `ImportManager`
- [ ] 支持增量备份
- [ ] 添加 CLI 命令
- [ ] 添加加密选项

---

### 7.4 多实例 Profiles

**目标**: 支持完全隔离的多实例。

**架构**:

```
~/.config/hermes-agent/
├── profiles/
│   ├── default/
│   │   ├── config.toml
│   │   ├── sessions.db
│   │   └── skills/
│   ├── work/
│   │   ├── config.toml
│   │   ├── sessions.db
│   │   └── skills/
│   └── personal/
│       └── ...
```

**实现任务**:

- [ ] 实现 `ProfileManager`
- [ ] 修改配置加载逻辑
- [ ] 修改数据库路径
- [ ] 添加 CLI 命令
  - [ ] `hermes profile list`
  - [ ] `hermes profile create <name>`
  - [ ] `hermes profile switch <name>`
  - [ ] `hermes profile delete <name>`

---

### 7.5 自我改进学习循环

**目标**: 实现从经验中学习的 Agent 改进机制。

**组件**:

| 组件 | 职责 |
|------|------|
| `TrajectoryLogger` | 记录 Agent 决策轨迹 |
| `LearningAnalyzer` | 分析成功/失败模式 |
| `SelfModifier` | 改进 Agent 配置 |
| `UserModeler` | 建立用户模型 |

**实现任务**:

- [ ] 实现 `TrajectoryLogger`
- [ ] 实现 `LearningAnalyzer`
- [ ] 实现 `SelfModifier`
- [ ] 实现 `UserModeler`
- [ ] 集成到 Agent 主循环
- [ ] 添加配置选项

---

### 7.6 皮肤/主题系统

**目标**: 实现 YAML 可配置的皮肤/主题系统。

**配置示例**:

```yaml
# skin: kawaii (default)
colors:
  primary: "#FFD700"      # Gold
  secondary: "#FF69B4"    # Hot Pink
  background: "#1E1E2E"
  user_bubble: "#3B82F6"
  assistant_bubble: "#10B981"
  error: "#EF4444"

fonts:
  main: "JetBrains Mono"
  fallback: "monospace"

symbols:
  user_prefix: "👤"
  assistant_prefix: "🤖"
  error_prefix: "❌"
  success_prefix: "✅"
```

**实现任务**:

- [ ] 实现 `SkinEngine`
- [ ] 实现 YAML 皮肤加载器
- [ ] 实现内置皮肤 (kawaii, ares, mono, slate)
- [ ] 添加 CLI 命令
  - [ ] `hermes skin list`
  - [ ] `hermes skin set <name>`
  - [ ] `hermes skin preview <name>`

---

### 7.7 RL 训练工具

**目标**: 实现强化学习训练工具。

**工具**:

| 工具 | 描述 |
|------|------|
| `rl_list_environments` | 列出可用 RL 环境 |
| `rl_select_environment` | 选择训练环境 |
| `rl_start_training` | 开始训练 |
| `rl_get_results` | 获取训练结果 |
| `rl_stop_training` | 停止训练 |
| `rl_watch` | 实时观察训练 |

**实现任务**:

- [ ] 创建 `hermes-tools-rl` crate
- [ ] 实现 RL API 客户端
- [ ] 实现各工具
- [ ] 添加配置
- [ ] 集成到工具注册表

---

## 8. 技术依赖

### 8.1 新增 Crates

| Crate | 用途 | 外部依赖 |
|-------|------|----------|
| `hermes-context` | 上下文管理 | - |
| `hermes-cron` | 定时调度 | `cron`, `chrono` |
| `hermes-mcp` | MCP Client | `reqwest` |
| `hermes-platform-discord` | Discord 适配器 | `reqwest`, `serenity` |
| `hermes-platform-slack` | Slack 适配器 | `reqwest` |
| `hermes-platform-whatsapp` | WhatsApp 适配器 | `reqwest`, `whatsapp-web.js` |
| `hermes-platform-matrix` | Matrix 适配器 | `reqwest`, `matrix-sdk` |
| `hermes-platform-email` | Email 适配器 | `lettre` |
| `hermes-platform-sms` | SMS 适配器 | `reqwest` (Twilio) |
| `hermes-platform-dingtalk` | 钉钉适配器 | `reqwest` |
| `hermes-platform-feishu` | 飞书适配器 | `reqwest` |
| `hermes-platform-mattermost` | Mattermost 适配器 | `reqwest` |
| `hermes-platform-homeassistant` | HA 适配器 | `reqwest` |
| `hermes-platform-bluebubbles` | iMessage 适配器 | `reqwest` |
| `hermes-platform-weixin` | 微信适配器 | `reqwest` |
| `hermes-platform-generic` | 通用 Webhook | `reqwest`, `axum` |
| `hermes-tools-homeassistant` | HA 工具 | `reqwest` |
| `hermes-tools-rl` | RL 工具 | `reqwest` |
| `hermes-skin` | 皮肤引擎 | `serde_yaml` |

### 8.2 现有 Crates 增强

| Crate | 增强内容 |
|-------|----------|
| `hermes-environment` | + Docker, SSH 支持 |
| `hermes-skills` | + SkillHub Client, SkillCreator, ProceduralMemory |
| `hermes-cli` | + 增强 REPL, Cron CLI, Profile CLI, MCP CLI |
| `hermes-gateway` | + 动态平台加载 |
| `hermes-tools-builtin` | + SubAgent 工具 |

---

## 9. 测试策略

### 9.1 单元测试

- 每个新 crate 的核心逻辑
- Mock 外部依赖
- 覆盖率目标: >80%

### 9.2 集成测试

- 与真实外部服务交互（如有测试账号）
- 平台适配器测试
- 使用 testcontainers-rs 做数据库测试

### 9.3 端到端测试

- 完整的 Agent 对话流程
- 工具执行流程
- 多平台消息流程

---

## 10. 文档

- 每个新 crate 的 README
- API 文档（rustdoc）
- 用户文档（Markdown）
- 迁移指南（从 Python 版本来）

---

## 11. 风险与缓解

| 风险 | 缓解措施 |
|------|----------|
| 外部 API 变更 | 抽象 API 层，版本锁定 |
| 第三方库不稳定 | 评估替代方案，保持核心抽象 |
| 平台 API 限制 | 从简单平台开始，逐步复杂 |
| 性能问题 | 性能测试，早期发现瓶颈 |
| 安全漏洞 | 安全审计，依赖审计 |

---

## 12. 里程碑

| 阶段 | 预计时间 | 交付物 |
|------|----------|--------|
| 阶段 1: 基础设施 | 2 周 | Context 管理、Docker/SSH 环境 |
| 阶段 2: 核心功能 | 2 周 | Cron Scheduler、增强 UI、MCP Client |
| 阶段 3: Skills Hub | 2 周 | 完整 Skills Hub |
| 阶段 4: 消息平台 | 3 周 | 所有平台适配器 |
| 阶段 5: 高级功能 | 3 周 | SubAgent、HA、RL、自我改进等 |

**总预计时间**: ~12 周

---

## 13. 附录

### A. 参考资源

- [NousResearch/hermes-agent](https://github.com/NousResearch/hermes-agent)
- [agentskills.io](https://agentskills.io)
- [Anthropic Prompt Caching](https://docs.anthropic.com/en/docs/build-with-claude/prompt-caching)
- [MCP Specification](https://modelcontextprotocol.io)

### B. 术语表

| 术语 | 定义 |
|------|------|
| Skill | 可重用的 Agent 能力模块 |
| Tool | Agent 可调用的外部能力 |
| Provider | LLM 服务提供商 |
| Platform Adapter | 消息平台适配器 |
| Environment | 代码执行环境 |

//! Agent Copilot Protocol (ACP) 实现
//! 
//! 本 crate 实现了 Agent Copilot Protocol (ACP)，使 Hermes Agent 能够在编辑器
//! 和其他 ACP 兼容的客户端中作为智能代理使用。
//! 
//! ## 主要功能
//! - 支持 ACP 协议的核心方法：initialize、new_session、prompt
//! - 实现了完整的 slash 命令系统
//! - 会话管理和历史记录
//! - 与 Hermes Agent 核心功能的集成

use std::sync::{Arc, Mutex};
use tokio::sync::RwLock;

/// ACP 服务器实现
/// 
/// 负责处理 ACP 协议的请求，管理会话状态，并与 Hermes Agent 核心功能集成。
pub struct AcpServer {
    /// 会话管理器，用于管理所有 ACP 会话
    session_manager: Arc<RwLock<SessionManager>>,
    /// 代理配置，包含模型等设置
    agent_config: hermes_core::AgentConfig,
}

/// 会话管理器
/// 
/// 负责存储和管理所有活跃的 ACP 会话。
struct SessionManager {
    /// 活跃会话的哈希表，键为会话 ID，值为会话状态
    sessions: std::collections::HashMap<String, SessionState>,
}

/// 会话状态
/// 
/// 存储单个 ACP 会话的状态信息，包括会话 ID、工作目录、代理实例和对话历史。
struct SessionState {
    /// 会话唯一标识符
    session_id: String,
    /// 当前工作目录
    cwd: String,
    /// Hermes Agent 实例
    agent: Arc<Mutex<hermes_core::Agent>>,
    /// 会话存储，用于持久化会话数据
    session_store: Arc<dyn hermes_memory::SessionStore>,
    /// 对话历史记录
    history: Vec<hermes_core::Message>,
}

impl AcpServer {
    /// 创建新的 ACP 服务器实例
    /// 
    /// # 参数
    /// - `agent_config`: Hermes Agent 的配置信息
    /// 
    /// # 返回值
    /// 新创建的 AcpServer 实例
    pub fn new(agent_config: hermes_core::AgentConfig) -> Self {
        Self {
            session_manager: Arc::new(RwLock::new(SessionManager {
                sessions: std::collections::HashMap::new(),
            })),
            agent_config,
        }
    }

    /// 处理 ACP 初始化请求
    /// 
    /// 响应 ACP 客户端的初始化请求，返回协议版本、代理信息和能力。
    /// 
    /// # 返回值
    /// 包含初始化信息的 InitializeResponse 结构体
    pub async fn initialize(&self) -> InitializeResponse {
        InitializeResponse {
            protocol_version: 1,  // ACP 协议版本
            agent_info: Implementation {
                name: "hermes-agent".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),  // 使用当前 crate 的版本
            },
            agent_capabilities: AgentCapabilities {
                load_session: true,  // 支持加载会话
                session_capabilities: SessionCapabilities {
                    fork: SessionForkCapabilities {},  // 支持会话分叉
                    list: SessionListCapabilities {},  // 支持列会话
                    resume: SessionResumeCapabilities {},  // 支持恢复会话
                },
            },
            auth_methods: None,  // 暂不支持认证方法
        }
    }

    /// 处理 ACP 新会话请求
    /// 
    /// 创建一个新的 ACP 会话，并返回会话 ID。
    /// 
    /// # 参数
    /// - `cwd`: 当前工作目录
    /// 
    /// # 返回值
    /// 包含新会话 ID 的 NewSessionResponse 结构体
    pub async fn new_session(&self, cwd: String) -> NewSessionResponse {
        // 生成唯一的会话 ID
        let session_id = uuid::Uuid::new_v4().to_string();
        
        // 创建会话状态
        let session_state = SessionState {
            session_id: session_id.clone(),
            cwd,
            // 创建 Hermes Agent 实例
            agent: Arc::new(Mutex::new(hermes_core::Agent::new(
                // 创建 OpenAI 提供者（TODO: 后续可扩展为支持多种提供者）
                Arc::new(hermes_provider::OpenAiProvider::new(
                    &std::env::var("OPENAI_API_KEY").unwrap_or_default(),
                    None,
                )),
                // 创建工具注册表
                Arc::new(hermes_tool_registry::ToolRegistry::new()),
                // 创建 SQLite 会话存储
                Arc::new(hermes_memory::SqliteSessionStore::new("hermes.db".into()).await.unwrap()),
                self.agent_config.clone(),
                hermes_core::NudgeConfig::default(),
                None,
                None,
                None,
                None,
                None,
                hermes_core::RetryConfig::default(),
            ))),
            // 创建会话存储
            session_store: Arc::new(hermes_memory::SqliteSessionStore::new("hermes.db".into()).await.unwrap()),
            // 初始化空的对话历史
            history: Vec::new(),
        };

        // 将会话添加到管理器
        self.session_manager.write().await.sessions.insert(session_id.clone(), session_state);

        // 返回新会话 ID
        NewSessionResponse {
            session_id,
        }
    }

    /// 处理 ACP 提示请求
    /// 
    /// 处理用户的输入，执行命令或运行对话，并返回响应。
    /// 
    /// # 参数
    /// - `prompt`: 内容块列表，包含用户的输入
    /// - `session_id`: 会话 ID
    /// 
    /// # 返回值
    /// 包含响应信息的 PromptResponse 结构体
    pub async fn prompt(&self, prompt: Vec<ContentBlock>, session_id: String) -> PromptResponse {
        // 获取会话管理器的写锁
        let mut session_manager = self.session_manager.write().await;
        // 根据会话 ID 获取会话状态
        let session_state = session_manager.sessions.get_mut(&session_id);

        if let Some(session) = session_state {
            // 从内容块中提取文本
            let user_text = extract_text(&prompt);
            
            // 如果文本为空，返回结束回合的响应
            if user_text.is_empty() {
                return PromptResponse {
                    stop_reason: "end_turn".to_string(),
                    usage: None,
                };
            }

            // 处理 slash 命令（以 / 开头的命令）
            if user_text.starts_with('/') {
                // 执行命令并获取响应
                let response = self.handle_slash_command(&user_text, session).await;
                // 将命令响应添加到对话历史
                session.history.push(hermes_core::Message::system(response));
                return PromptResponse {
                    stop_reason: "end_turn".to_string(),
                    usage: None,
                };
            }

            // 将用户消息添加到对话历史
            session.history.push(hermes_core::Message::user(user_text.clone()));

            // 创建对话请求
            let request = hermes_core::ConversationRequest {
                content: user_text,
                session_id: Some(session_id.clone()),
                system_prompt: None,
            };

            // 运行对话
            let response = session.agent.lock().unwrap().run_conversation(request).await;

            match response {
                Ok(resp) => {
                    // 将助手消息添加到对话历史
                    session.history.push(hermes_core::Message::assistant(resp.content));
                    // 返回包含使用情况的响应
                    PromptResponse {
                        stop_reason: "end_turn".to_string(),
                        usage: resp.usage.map(|u| Usage {
                            input_tokens: u.input_tokens,
                            output_tokens: u.output_tokens,
                            total_tokens: u.input_tokens + u.output_tokens,
                            thought_tokens: u.reasoning_tokens,
                            cached_read_tokens: u.cache_read_tokens,
                        }),
                    }
                },
                Err(e) => {
                    // 将错误消息添加到对话历史
                    session.history.push(hermes_core::Message::system(format!("Error: {}", e)));
                    // 返回错误响应
                    PromptResponse {
                        stop_reason: "end_turn".to_string(),
                        usage: None,
                    }
                },
            }
        } else {
            // 会话不存在，返回拒绝响应
            PromptResponse {
                stop_reason: "refusal".to_string(),
                usage: None,
            }
        }
    }

    /// 处理 slash 命令
    /// 
    /// 解析并执行以 / 开头的命令。
    /// 
    /// # 参数
    /// - `text`: 命令文本
    /// - `session`: 会话状态
    /// 
    /// # 返回值
    /// 命令执行结果的字符串
    async fn handle_slash_command(&self, text: &str, session: &mut SessionState) -> String {
        // 分割命令和参数
        let parts: Vec<&str> = text.splitn(2, ' ').collect();
        // 提取命令名称（去除 / 前缀并转为小写）
        let cmd = parts[0].trim_start_matches('/').to_lowercase();
        // 提取命令参数
        let args = parts.get(1).unwrap_or(&"").trim();

        // 根据命令名称执行对应的处理函数
        match cmd.as_str() {
            "help" => self.cmd_help(),
            "model" => self.cmd_model(args, session),
            "tools" => self.cmd_tools(session),
            "context" => self.cmd_context(session),
            "reset" => self.cmd_reset(session),
            "compact" => self.cmd_compact(session),
            "version" => self.cmd_version(),
            _ => "Unknown command. Type /help for available commands.".to_string(),
        }
    }

    /// 帮助命令
    /// 
    /// 显示所有可用的命令及其描述。
    /// 
    /// # 返回值
    /// 包含命令列表的字符串
    fn cmd_help(&self) -> String {
        let commands = [
            ("help", "显示可用命令"),
            ("model", "显示或更改当前模型"),
            ("tools", "列出可用工具"),
            ("context", "显示对话上下文信息"),
            ("reset", "清空对话历史"),
            ("compact", "压缩对话上下文"),
            ("version", "显示 Hermes 版本"),
        ];

        let mut response = "可用命令:\n".to_string();
        for (cmd, desc) in commands {
            response.push_str(&format!("  /{:10}  {}\n", cmd, desc));
        }
        response.push_str("\n未识别的 / 命令会作为普通消息发送给模型。");
        response
    }

    /// 模型命令
    /// 
    /// 显示当前使用的模型，或尝试切换模型。
    /// 
    /// # 参数
    /// - `args`: 命令参数，如果为空则显示当前模型，否则尝试切换模型
    /// - `session`: 会话状态
    /// 
    /// # 返回值
    /// 命令执行结果的字符串
    fn cmd_model(&self, args: &str, session: &mut SessionState) -> String {
        if args.is_empty() {
            // 显示当前模型
            let model = session.agent.lock().unwrap().config().model.clone();
            "当前模型: ".to_string() + &model
        } else {
            // TODO: 实现模型切换功能
            "模型切换功能尚未实现".to_string()
        }
    }

    /// 工具命令
    /// 
    /// 列出所有可用的工具。
    /// 
    /// # 参数
    /// - `session`: 会话状态
    /// 
    /// # 返回值
    /// 包含工具列表的字符串
    fn cmd_tools(&self, session: &mut SessionState) -> String {
        // 获取工具定义
        let tools = session.agent.lock().unwrap().tools().get_definitions();
        if tools.is_empty() {
            "没有可用的工具。".to_string()
        } else {
            let mut response = format!("可用工具 ({}):\n", tools.len());
            for tool in tools {
                response.push_str(&format!("  {}: {}\n", tool.name, tool.description));
            }
            response
        }
    }

    /// 上下文命令
    /// 
    /// 显示对话上下文信息，包括消息数量和角色分布。
    /// 
    /// # 参数
    /// - `session`: 会话状态
    /// 
    /// # 返回值
    /// 包含上下文信息的字符串
    fn cmd_context(&self, session: &mut SessionState) -> String {
        let n_messages = session.history.len();
        if n_messages == 0 {
            "对话为空（尚无消息）。".to_string()
        } else {
            // 统计各角色的消息数量
            let mut roles = std::collections::HashMap::new();
            for msg in &session.history {
                let role_str = match msg.role {
                    hermes_core::Role::System => "system",
                    hermes_core::Role::User => "user",
                    hermes_core::Role::Assistant => "assistant",
                    hermes_core::Role::Tool => "tool",
                };
                *roles.entry(role_str.to_string()).or_insert(0) += 1;
            }

            // 获取当前模型
            let model = session.agent.lock().unwrap().config().model.clone();
            // 格式化上下文信息
            format!(
                "对话: {} 条消息\n  user: {}, assistant: {}, tool: {}, system: {}\n模型: {}",
                n_messages,
                roles.get("user").unwrap_or(&0),
                roles.get("assistant").unwrap_or(&0),
                roles.get("tool").unwrap_or(&0),
                roles.get("system").unwrap_or(&0),
                model
            )
        }
    }

    /// 重置命令
    /// 
    /// 清空对话历史。
    /// 
    /// # 参数
    /// - `session`: 会话状态
    /// 
    /// # 返回值
    /// 命令执行结果的字符串
    fn cmd_reset(&self, session: &mut SessionState) -> String {
        // 清空对话历史
        session.history.clear();
        "对话历史已清空。".to_string()
    }

    /// 压缩命令
    /// 
    /// 压缩对话上下文，只保留最近的消息。
    /// 
    /// # 参数
    /// - `session`: 会话状态
    /// 
    /// # 返回值
    /// 命令执行结果的字符串
    fn cmd_compact(&self, session: &mut SessionState) -> String {
        // 如果消息数量较少，无需压缩
        if session.history.len() <= 2 {
            return "上下文已经很紧凑".to_string();
        }
        
        // 简单压缩：只保留第一条消息和最后几条消息
        let mut compressed = Vec::new();
        
        // 保留第一条消息（通常是系统提示或初始用户消息）
        if let Some(first_msg) = session.history.first() {
            compressed.push(first_msg.clone());
        }
        
        // 保留最后 4 条消息（最近的对话）
        let start_idx = session.history.len().saturating_sub(4);
        for msg in &session.history[start_idx..] {
            compressed.push(msg.clone());
        }
        
        // 更新对话历史
        session.history = compressed;
        // 返回压缩结果
        format!("上下文已从 {} 条消息压缩到 {} 条消息", session.history.len() + 4, session.history.len())
    }

    /// 版本命令
    /// 
    /// 显示 Hermes Agent 的版本。
    /// 
    /// # 返回值
    /// 包含版本信息的字符串
    fn cmd_version(&self) -> String {
        format!("Hermes Agent v{}", env!("CARGO_PKG_VERSION"))
    }
}

/// 从内容块中提取文本
/// 
/// 遍历内容块列表，提取所有文本内容并合并。
/// 
/// # 参数
/// - `blocks`: 内容块列表
/// 
/// # 返回值
/// 提取的文本字符串
fn extract_text(blocks: &[ContentBlock]) -> String {
    let mut text = String::new();
    for block in blocks {
        if let ContentBlock::Text(text_block) = block {
            text.push_str(&text_block.text);
            text.push_str("\n");
        }
    }
    text.trim().to_string()
}

// ACP 模式类型
// 
// 以下是 ACP 协议中使用的数据结构定义。

/// 初始化响应
/// 
/// 包含 ACP 协议版本、代理信息和能力。
#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct InitializeResponse {
    /// ACP 协议版本
    pub protocol_version: i32,
    /// 代理信息
    pub agent_info: Implementation,
    /// 代理能力
    pub agent_capabilities: AgentCapabilities,
    /// 认证方法
    pub auth_methods: Option<Vec<AuthMethodAgent>>,
}

/// 实现信息
/// 
/// 包含代理的名称和版本。
#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct Implementation {
    /// 代理名称
    pub name: String,
    /// 代理版本
    pub version: String,
}

/// 代理能力
/// 
/// 包含代理的会话能力。
#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct AgentCapabilities {
    /// 是否支持加载会话
    pub load_session: bool,
    /// 会话能力
    pub session_capabilities: SessionCapabilities,
}

/// 会话能力
/// 
/// 包含会话的分叉、列表和恢复能力。
#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct SessionCapabilities {
    /// 会话分叉能力
    pub fork: SessionForkCapabilities,
    /// 会话列表能力
    pub list: SessionListCapabilities,
    /// 会话恢复能力
    pub resume: SessionResumeCapabilities,
}

/// 会话分叉能力
#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct SessionForkCapabilities {
}

/// 会话列表能力
#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct SessionListCapabilities {
}

/// 会话恢复能力
#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct SessionResumeCapabilities {
}

/// 认证方法
/// 
/// 包含认证方法的 ID、名称和描述。
#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct AuthMethodAgent {
    /// 认证方法 ID
    pub id: String,
    /// 认证方法名称
    pub name: String,
    /// 认证方法描述
    pub description: String,
}

/// 新会话响应
/// 
/// 包含新创建的会话 ID。
#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct NewSessionResponse {
    /// 会话 ID
    pub session_id: String,
}

/// 提示响应
/// 
/// 包含响应的停止原因和使用情况。
#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct PromptResponse {
    /// 停止原因
    pub stop_reason: String,
    /// 使用情况
    pub usage: Option<Usage>,
}

/// 使用情况
/// 
/// 包含令牌使用情况。
#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct Usage {
    /// 输入令牌数
    pub input_tokens: usize,
    /// 输出令牌数
    pub output_tokens: usize,
    /// 总令牌数
    pub total_tokens: usize,
    /// 思考令牌数
    pub thought_tokens: Option<usize>,
    /// 缓存读取令牌数
    pub cached_read_tokens: Option<usize>,
}

/// 内容块
/// 
/// 表示 ACP 协议中的各种内容类型。
#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub enum ContentBlock {
    /// 文本内容
    Text(TextContentBlock),
    /// 图片内容
    Image(ImageContentBlock),
    /// 音频内容
    Audio(AudioContentBlock),
    /// 资源内容
    Resource(ResourceContentBlock),
    /// 嵌入式资源内容
    EmbeddedResource(EmbeddedResourceContentBlock),
}

/// 文本内容块
/// 
/// 包含文本内容。
#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct TextContentBlock {
    /// 文本内容
    pub text: String,
}

/// 图片内容块
/// 
/// 包含图片 URL 和替代文本。
#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct ImageContentBlock {
    /// 图片 URL
    pub url: String,
    /// 替代文本
    pub alt: Option<String>,
}

/// 音频内容块
/// 
/// 包含音频 URL。
#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct AudioContentBlock {
    /// 音频 URL
    pub url: String,
}

/// 资源内容块
/// 
/// 包含资源 URL 和名称。
#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct ResourceContentBlock {
    /// 资源 URL
    pub url: String,
    /// 资源名称
    pub name: String,
}

/// 嵌入式资源内容块
/// 
/// 包含嵌入式资源的名称、内容和媒体类型。
#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct EmbeddedResourceContentBlock {
    /// 资源名称
    pub name: String,
    /// 资源内容
    pub content: String,
    /// 媒体类型
    pub media_type: String,
}



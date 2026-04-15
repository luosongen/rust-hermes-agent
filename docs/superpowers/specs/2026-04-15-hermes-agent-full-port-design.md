# rust-hermes-agent 功能移植设计文档

**日期**: 2026-04-15
**项目**: rust-hermes-agent 功能完整移植
**目标**: 完整复现原版 Python 项目功能

## 背景

rust-hermes-agent 是原版 [hermes-agent](https://github.com/NousResearch/hermes-agent) 的 Rust 重写。当前实现与原版存在较大功能差距，需要分阶段补全。

**优先级**: 模型优先 > 核心功能 > 工具 > MCP > 平台不重要

---

## 阶段划分

### Phase 1: LLM Providers (模型优先)
**目标**: 支持更多 LLM 模型

| Provider | 描述 | 状态 |
|----------|------|------|
| OpenAI | GPT-4o, GPT-4-turbo, GPT-4, GPT-3.5-turbo | ✅ 已有 |
| Anthropic | Claude-3.5, Claude-3, Claude-4 系列 | ✅ 已有 |
| OpenRouter | 200+ 模型统一入口 | ❌ 需实现 |
| GLM (z.ai) | 智谱 AI 模型 | ❌ 需实现 |

**子任务**:
- [ ] `openrouter.rs` - OpenRouter Provider (统一 API，支持 200+ 模型)
- [ ] `glm.rs` - GLM Provider (智谱 AI)
- [ ] 模型路由 - 根据模型名自动选择 Provider
- [ ] Provider 池化 - 多 API Key 负载均衡

**验收标准**:
- [ ] OpenRouter Provider 编译通过，单元测试通过
- [ ] GLM Provider 编译通过，单元测试通过
- [ ] 模型路由功能测试通过

---

### Phase 2: Core Features (核心功能)
**目标**: 完善内存、会话、上下文管理

| 功能 | 描述 | 状态 |
|------|------|------|
| Memory 增强 | 持久化内存，支持跨会话 | ❌ 需实现 |
| FTS5 搜索 | 全文搜索引擎 | ❌ 需实现 |
| Context 压缩 | 自动压缩长对话 | ❌ 需实现 |
| Session 管理 | 会话创建、切换、历史 | ✅ 基础已有 |

**子任务**:
- [ ] `memory_manager.rs` - 内存管理器 (与原版 `memory_manager.py` 对应)
- [ ] FTS5 索引 - SQLite FTS5 集成
- [ ] `context_compressor.rs` - 上下文压缩器
- [ ] 会话摘要生成 - LLM 生成会话摘要

**验收标准**:
- [ ] Memory Manager 编译通过，单元测试通过
- [ ] FTS5 搜索测试通过
- [ ] Context 压缩功能测试通过

---

### Phase 3: Tools + MCP (工具和扩展)
**目标**: 扩展工具集和支持 MCP 协议

| 工具 | 描述 | 状态 |
|------|------|------|
| Web Search | 网页搜索 | ❌ 需实现 |
| Web Fetch | 网页内容抓取 | ❌ 需实现 |
| Code Execution | 代码执行 | ❌ 需实现 |
| Cron Jobs | 定时任务 | ❌ 需实现 |
| MCP Client | MCP 协议客户端 | ❌ 需实现 |
| MCP Server Bridge | MCP 服务桥接 | ❌ 需实现 |

**子任务**:
- [ ] `web_search_tool.rs` - 网页搜索工具
- [ ] `web_fetch_tool.rs` - 网页内容抓取
- [ ] `code_execution_tool.rs` - 代码执行（沙箱）
- [ ] `cron_scheduler.rs` - 定时任务调度器
- [ ] `mcp_client.rs` - MCP 客户端实现
- [ ] `mcp_server.rs` - MCP 服务端桥接

**验收标准**:
- [ ] Web 搜索工具编译通过，测试通过
- [ ] Cron 调度器编译通过，测试通过
- [ ] MCP Client 基础功能测试通过

---

## 架构设计

### Provider 架构
```
┌─────────────────────────────────────────┐
│              Agent                       │
│  (uses LlmProvider trait)               │
└─────────────────┬───────────────────────┘
                  │
    ┌─────────────┼─────────────┐
    ▼             ▼             ▼
OpenAiProvider  AnthropicProvider  OpenRouterProvider  GLMProvider
    │             │             │                │
    └─────────────┴─────────────┴────────────────┘
                        │
              (根据 model.provider 路由)
```

### Memory 架构
```
┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│  Short-term  │────▶│  Long-term   │────▶│   FTS5       │
│   Memory     │     │   Memory     │     │   Index      │
└──────────────┘     └──────────────┘     └──────────────┘
       │                   │                    │
       └───────────────────┴────────────────────┘
                    SQLite Storage
```

### MCP 架构
```
┌──────────┐     ┌──────────────┐     ┌──────────┐
│  MCP     │────▶│   MCP        │────▶│  Local   │
│  Client  │     │   Bridge     │     │  Tools   │
└──────────┘     └──────────────┘     └──────────┘
       │                   │
       ▼                   ▼
┌──────────────┐     ┌──────────────┐
│  MCP Server  │     │  Tool        │
│  (External)  │     │  Registry    │
└──────────────┘     └──────────────┘
```

---

## 依赖关系

```
Phase 1 (Providers)
    │
    ▼
Phase 2 (Core)    ←─── 依赖 Phase 1 的 Provider
    │
    ▼
Phase 3 (Tools)   ←─── 依赖 Phase 2 的 Memory/Compression
```

---

## 实施策略

1. **分阶段迭代** - 每阶段独立开发、测试、完成后再进入下一阶段
2. **自主执行** - 我自主执行开发，你只做最终验收
3. **每功能一提交** - 每个子功能完成后立即提交，包含单元测试
4. **测试驱动** - 核心功能需要 TDD，先写测试再实现

---

## 当前状态

| Phase | 状态 | 进度 |
|-------|------|------|
| Phase 1: Providers | 进行中 | Anthropic ✅, OpenRouter ⬜, GLM ⬜ |
| Phase 2: Core | 待开始 | - |
| Phase 3: Tools + MCP | 待开始 | - |

---

## 下一步

进入 **Phase 1: OpenRouter Provider** 的详细实现规划。

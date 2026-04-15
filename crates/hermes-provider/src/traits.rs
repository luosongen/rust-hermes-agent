// =============================================================================
// LLM 提供者 trait 定义
// =============================================================================
//
// 该模块重导出 [`hermes_core::LlmProvider`] trait。
//
// ## 为什么重导出？
//
// `LlmProvider` trait 定义在 `hermes-core` crate 中，而非 `hermes-provider`，
// 是为了避免循环依赖问题：
//
// ```text
// hermes-core → hermes-provider（依赖 LlmProvider 的定义）
// hermes-agent → hermes-core（Agent 使用 LlmProvider）
// ```
//
// 如果 `LlmProvider` 定义在 `hermes-provider` 中，而 `hermes-core` 又依赖它，
// 就会形成循环依赖。通过将 trait 定义在共享的 `hermes-core` 中，
// 各 crate 可以独立地导入和使用它，而 `hermes-provider` 只需要负责实现该 trait。
//
// 消费者从 `hermes-provider` 导入 `LlmProvider` 是为了方便——所有与提供者相关的
// 类型都可以从一个 crate 获取。

// LlmProvider is defined in hermes-core so that the Agent can use it
// without creating a circular dependency. Re-export it here for consumers
// who import from hermes-provider.
pub use hermes_core::LlmProvider;

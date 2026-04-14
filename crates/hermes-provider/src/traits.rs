// LlmProvider is defined in hermes-core so that the Agent can use it
// without creating a circular dependency. Re-export it here for consumers
// who import from hermes-provider.
pub use hermes_core::LlmProvider;

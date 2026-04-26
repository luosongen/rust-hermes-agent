//! Tests for new config modules
//!
//! These tests verify serialization/deserialization for the new config modules.

#[cfg(test)]
mod tests {
    use crate::config::provider_core::{AnthropicConfig, OpenAIConfig, OpenRouterConfig};
    use crate::config::{
        AuxiliaryConfig, BackendConfig, BackendSettings, CompressionConfig, Config, CoreProvider,
        DelegationConfig, DisplayConfig, DockerBackend, LocalBackend, McpServerConfig,
        McpServersConfig, McpTransport, PersonalityConfig, PersonalityPreset, ProviderSettings,
        SSHBackend, SttConfig, SttProviderConfig,
    };
    use crate::credential_pool::Secret;

    // Provider tests
    #[test]
    fn test_openrouter_provider_serialization() {
        let config = CoreProvider::OpenRouter(OpenRouterConfig {
            api_key: Secret("test-key".to_string()),
            base_url: None,
            models: vec!["meta-llama/llama-3".to_string()],
        });

        let serialized = toml::to_string(&config).unwrap();
        assert!(serialized.contains("openrouter"));
    }

    #[test]
    fn test_openai_provider_serialization() {
        let config = CoreProvider::OpenAI(OpenAIConfig {
            api_key: Secret("sk-test".to_string()),
            base_url: Some("https://api.openai.com".to_string()),
            models: vec!["gpt-4o".to_string()],
        });

        let serialized = toml::to_string(&config).unwrap();
        assert!(serialized.contains("openai"));
    }

    #[test]
    fn test_anthropic_provider_serialization() {
        let config = CoreProvider::Anthropic(AnthropicConfig {
            api_key: Secret("sk-ant-api03".to_string()),
            base_url: None,
            models: vec!["claude-3-5-sonnet-20241022".to_string()],
        });

        let serialized = toml::to_string(&config).unwrap();
        assert!(serialized.contains("anthropic"));
    }

    // Backend tests
    #[test]
    fn test_docker_backend_serialization() {
        let docker = BackendConfig::Docker(DockerBackend {
            enabled: true,
            image: Some("rust:latest".to_string()),
            container: Some("my-container".to_string()),
        });

        let serialized = toml::to_string(&docker).unwrap();
        assert!(serialized.contains("docker"));
        assert!(serialized.contains("my-container"));
    }

    #[test]
    fn test_local_backend_serialization() {
        let local = BackendConfig::Local(LocalBackend { enabled: true });
        let serialized = toml::to_string(&local).unwrap();
        assert!(serialized.contains("local"));
    }

    #[test]
    fn test_ssh_backend_serialization() {
        let ssh = BackendConfig::SSH(SSHBackend {
            enabled: true,
            host: Some("192.168.1.100".to_string()),
            port: Some(22),
            user: Some("admin".to_string()),
        });

        let serialized = toml::to_string(&ssh).unwrap();
        assert!(serialized.contains("ssh"));
        assert!(serialized.contains("192.168.1.100"));
    }

    // Compression tests
    #[test]
    fn test_compression_config_defaults() {
        let config = CompressionConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.threshold, 60000);
        assert_eq!(config.target_ratio, 0.7);
        assert_eq!(config.protect_last_n, 10);
    }

    #[test]
    fn test_compression_config_custom() {
        let config = CompressionConfig {
            enabled: true,
            threshold: 30000,
            target_ratio: 0.5,
            protect_last_n: 5,
            model: Some("openai/gpt-4o".to_string()),
        };

        assert!(config.enabled);
        assert_eq!(config.threshold, 30000);
        assert_eq!(config.target_ratio, 0.5);
        assert_eq!(config.protect_last_n, 5);
    }

    // Auxiliary tests
    #[test]
    fn test_auxiliary_config_defaults() {
        let config = AuxiliaryConfig::default();
        assert_eq!(config.vision.provider, "openai");
        assert_eq!(config.vision.model, "openai/gpt-4o");
        assert_eq!(config.web_extract.provider, "openai");
        assert_eq!(config.web_extract.model, "openai/gpt-4o-mini");
    }

    // Delegation tests
    #[test]
    fn test_delegation_config_defaults() {
        let config = DelegationConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.max_depth, 3);
        assert_eq!(config.default_model, "openai/gpt-4o");
        assert!(config.default_personality.is_none());
        assert!(config.max_tokens.is_none());
    }

    // Display tests
    #[test]
    fn test_display_config_defaults() {
        let config = DisplayConfig::default();
        assert!(!config.compact);
        assert!(config.tool_progress);
        assert_eq!(config.skin, "default");
    }

    // Personality tests
    #[test]
    fn test_personality_config_defaults() {
        let config = PersonalityConfig::default();
        assert_eq!(config.default, "helpfulness");
        assert!(!config.personalities.is_empty());
        assert_eq!(config.personalities[0].name, "helpfulness");
    }

    #[test]
    fn test_personality_preset_serialization() {
        let preset = PersonalityPreset {
            name: "coder".to_string(),
            system_prompt: "You are an expert programmer.".to_string(),
            model: Some("anthropic/claude-3-5-sonnet-20241022".to_string()),
        };

        let serialized = toml::to_string(&preset).unwrap();
        assert!(serialized.contains("coder"));
        assert!(serialized.contains("expert programmer"));
    }

    // MCP tests
    #[test]
    fn test_mcp_stdio_transport_serialization() {
        let config = McpServerConfig {
            name: "test-server".to_string(),
            enabled: true,
            transport: McpTransport::Stdio {
                command: "npx".to_string(),
                args: vec!["@modelcontextprotocol/server-filesystem".to_string()],
            },
        };

        let serialized = toml::to_string(&config).unwrap();
        assert!(serialized.contains("test-server"));
        assert!(serialized.contains("stdio"));
        assert!(serialized.contains("npx"));
    }

    #[test]
    fn test_mcp_http_transport_serialization() {
        let config = McpServerConfig {
            name: "http-server".to_string(),
            enabled: true,
            transport: McpTransport::Http {
                url: "https://localhost:8080/mcp".to_string(),
            },
        };

        let serialized = toml::to_string(&config).unwrap();
        assert!(serialized.contains("http-server"));
        assert!(serialized.contains("http"));
        assert!(serialized.contains("localhost:8080"));
    }

    #[test]
    fn test_mcp_servers_config_default() {
        let config = McpServersConfig::default();
        assert!(config.servers.is_empty());
    }

    // STT tests
    #[test]
    fn test_stt_config_serialization() {
        let config = SttConfig {
            default: "openai".to_string(),
            providers: vec![SttProviderConfig {
                name: "openai".to_string(),
                provider: "openai".to_string(),
                model: "whisper-1".to_string(),
                api_key: None,
                base_url: None,
                enabled: true,
            }],
        };

        let serialized = toml::to_string(&config).unwrap();
        assert!(serialized.contains("openai"));
        assert!(serialized.contains("whisper-1"));
    }

    #[test]
    fn test_stt_config_defaults() {
        let config = SttConfig::default();
        assert_eq!(config.default, "local");
        assert!(config.providers.is_empty());
    }

    // Integration tests
    #[test]
    fn test_config_includes_new_fields() {
        let config = Config::default();
        // Verify new fields exist and have defaults
        assert_eq!(config.providers.default, "openai/gpt-4o");
        assert!(!config.compression.enabled);
        assert!(!config.delegation.enabled);
        assert!(!config.display.compact);
    }

    #[test]
    fn test_provider_settings_default() {
        let settings = ProviderSettings::default();
        assert_eq!(settings.default, "openai/gpt-4o");
        assert!(settings.priority.is_empty());
        assert_eq!(settings.fallback, "openai/gpt-4o");
        assert!(!settings.smart_router.enabled);
    }

    #[test]
    fn test_backend_settings_default() {
        let settings = BackendSettings::default();
        // Default backend should be Local
        let serialized = toml::to_string(&settings.default).unwrap();
        assert!(serialized.contains("local"));
    }
}

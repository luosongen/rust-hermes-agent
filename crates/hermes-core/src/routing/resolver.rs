use crate::config::SmartRouterConfig;
use super::detector::ComplexityDetector;

/// 路由解析结果
#[derive(Debug, Clone)]
pub struct RouteResolution {
    pub model: Option<String>,
    pub provider: String,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub label: Option<String>,
}

/// 智能路由器 - 根据消息复杂度选择模型
pub struct SmartRouter {
    config: SmartRouterConfig,
    detector: ComplexityDetector,
}

impl SmartRouter {
    /// 创建新的路由器
    pub fn new(config: SmartRouterConfig) -> Self {
        Self {
            config,
            detector: ComplexityDetector::default(),
        }
    }

    /// 检查路由器是否启用
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// 选择廉价模型路由（如果消息足够简单）
    pub fn choose_cheap_model_route(&self, message: &str) -> Option<RouteResolution> {
        if !self.config.enabled {
            return None;
        }

        if !self.detector.is_simple(message) {
            return None;
        }

        let parts: Vec<&str> = self.config.cheap_model.split('/').collect();
        if parts.len() != 2 {
            return None;
        }

        Some(RouteResolution {
            model: Some(self.config.cheap_model.clone()),
            provider: parts[0].to_string(),
            base_url: None,
            api_key: None,
            label: Some("simple_turn".to_string()),
        })
    }

    /// 解析路由决策
    ///
    /// 如果消息简单且启用廉价模型，返回廉价路由；否则返回 None
    pub fn resolve_route(&self, message: &str) -> Option<RouteResolution> {
        self.choose_cheap_model_route(message)
    }
}

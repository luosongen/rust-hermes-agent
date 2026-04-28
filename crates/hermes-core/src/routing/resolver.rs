//! 智能路由解析器
//!
//! 根据消息复杂度自动选择合适的模型。

use crate::config::SmartRouterConfig;
use super::detector::ComplexityDetector;

/// 路由解析结果
#[derive(Debug, Clone)]
pub struct RouteResolution {
    /// 模型名称
    pub model: Option<String>,
    /// 提供者名称
    pub provider: String,
    /// 自定义 API 基础 URL
    pub base_url: Option<String>,
    /// API 密钥
    pub api_key: Option<String>,
    /// 路由标签
    pub label: Option<String>,
}

/// 智能路由器
///
/// 根据消息复杂度自动选择合适的模型，降低成本。
pub struct SmartRouter {
    /// 路由配置
    config: SmartRouterConfig,
    /// 复杂度检测器
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
    /// 如果消息简单且启用廉价模型，返回廉价路由；否则返回 None。
    pub fn resolve_route(&self, message: &str) -> Option<RouteResolution> {
        self.choose_cheap_model_route(message)
    }
}

//! 路由模块 - 智能模型路由选择
//!
//! 根据消息复杂度自动选择合适的模型，降低成本。

pub mod detector;
pub mod resolver;

pub use detector::ComplexityDetector;
pub use resolver::{RouteResolution, SmartRouter};

//! ContextPressureMonitor — 分层上下文压力监控
//!
//! 监控上下文窗口使用率并在不同阈值提供警告：
//! - Normal (0-50%): 无需操作
//! - Moderate (50-75%): 考虑准备压缩
//! - High (75-90%): 建议压缩
//! - Critical (90%+): 即将触发压缩

use serde::{Deserialize, Serialize};

/// 压力级别枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PressureLevel {
    /// 低于中等阈值
    Normal,
    /// 已使用 50-75% 上下文
    Moderate,
    /// 已使用 75-90% 上下文
    High,
    /// 已使用 90%+ 上下文
    Critical,
}

/// 上下文压力监控器
///
/// 监控上下文窗口使用率并提供分层警告。
pub struct ContextPressureMonitor {
    /// 上下文窗口总大小
    context_length: usize,
    /// 中等阈值（默认：50%）
    moderate_threshold: usize,
    /// 高阈值（默认：75%）
    high_threshold: usize,
    /// 临界阈值（默认：90%）
    critical_threshold: usize,
}

impl ContextPressureMonitor {
    /// 创建使用默认阈值（50%、75%、90%）的监控器
    pub fn new(context_length: usize) -> Self {
        Self {
            context_length,
            moderate_threshold: context_length * 50 / 100,
            high_threshold: context_length * 75 / 100,
            critical_threshold: context_length * 90 / 100,
        }
    }

    /// 创建使用自定义中等和高阈值的监控器
    pub fn with_custom_thresholds(context_length: usize, moderate: usize, high: usize) -> Self {
        Self {
            context_length,
            moderate_threshold: moderate,
            high_threshold: high,
            critical_threshold: context_length * 90 / 100,
        }
    }

    /// 根据 token 数量获取当前压力级别
    pub fn get_pressure_level(&self, current_tokens: usize) -> PressureLevel {
        let ratio = current_tokens as f64 / self.context_length as f64;
        if ratio >= 0.90 {
            PressureLevel::Critical
        } else if ratio * self.context_length as f64 >= self.high_threshold as f64 {
            PressureLevel::High
        } else if ratio * self.context_length as f64 >= self.moderate_threshold as f64 {
            PressureLevel::Moderate
        } else {
            PressureLevel::Normal
        }
    }

    /// 获取当前压力级别的警告消息
    pub fn get_warning_message(&self, current_tokens: usize) -> String {
        let level = self.get_pressure_level(current_tokens);
        let percentage = (current_tokens as f64 / self.context_length as f64 * 100.0) as usize;

        match level {
            PressureLevel::Normal => String::new(),
            PressureLevel::Moderate => format!(
                "上下文使用率 {}% — 如果对话继续延长，请考虑准备压缩。",
                percentage
            ),
            PressureLevel::High => format!(
                "高上下文压力 ({}%) — 建议压缩。",
                percentage
            ),
            PressureLevel::Critical => format!(
                "临界上下文压力 ({}%) — 即将触发压缩。",
                percentage
            ),
        }
    }

    /// 检查是否应主动触发压缩
    pub fn should_compress(&self, current_tokens: usize) -> bool {
        self.get_pressure_level(current_tokens) == PressureLevel::Critical
    }

    /// 获取当前使用率（0.0 到 1.0+）
    pub fn usage_ratio(&self, current_tokens: usize) -> f64 {
        current_tokens as f64 / self.context_length as f64
    }

    /// 获取中等阈值
    pub fn moderate_threshold(&self) -> usize {
        self.moderate_threshold
    }

    /// 获取高阈值
    pub fn high_threshold(&self) -> usize {
        self.high_threshold
    }

    /// 获取临界阈值
    pub fn critical_threshold(&self) -> usize {
        self.critical_threshold
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_thresholds() {
        let monitor = ContextPressureMonitor::new(100_000);
        assert_eq!(monitor.moderate_threshold(), 50_000);
        assert_eq!(monitor.high_threshold(), 75_000);
        assert_eq!(monitor.critical_threshold(), 90_000);
    }

    #[test]
    fn test_pressure_level_boundaries() {
        let monitor = ContextPressureMonitor::new(100_000);

        // Exact boundaries
        assert_eq!(monitor.get_pressure_level(0), PressureLevel::Normal);
        assert_eq!(monitor.get_pressure_level(49_999), PressureLevel::Normal);
        assert_eq!(monitor.get_pressure_level(50_000), PressureLevel::Moderate);
        assert_eq!(monitor.get_pressure_level(74_999), PressureLevel::Moderate);
        assert_eq!(monitor.get_pressure_level(75_000), PressureLevel::High);
        assert_eq!(monitor.get_pressure_level(89_999), PressureLevel::High);
        assert_eq!(monitor.get_pressure_level(90_000), PressureLevel::Critical);
        assert_eq!(monitor.get_pressure_level(100_000), PressureLevel::Critical);
    }

    #[test]
    fn test_warning_messages() {
        let monitor = ContextPressureMonitor::new(100_000);

        assert!(monitor.get_warning_message(30_000).is_empty());

        let moderate = monitor.get_warning_message(60_000);
        assert!(moderate.contains("60%") || moderate.contains("Context at"));

        let high = monitor.get_warning_message(80_000);
        assert!(high.contains("High") || high.contains("80%"));

        let critical = monitor.get_warning_message(95_000);
        assert!(critical.contains("Critical") || critical.contains("95%"));
    }

    #[test]
    fn test_should_compress() {
        let monitor = ContextPressureMonitor::new(100_000);

        assert!(!monitor.should_compress(50_000));
        assert!(!monitor.should_compress(89_999));
        assert!(monitor.should_compress(90_000));
        assert!(monitor.should_compress(95_000));
    }

    #[test]
    fn test_usage_ratio() {
        let monitor = ContextPressureMonitor::new(100_000);
        assert!((monitor.usage_ratio(50_000) - 0.5).abs() < 0.001);
        assert!((monitor.usage_ratio(75_000) - 0.75).abs() < 0.001);
        assert!((monitor.usage_ratio(100_000) - 1.0).abs() < 0.001);
    }
}

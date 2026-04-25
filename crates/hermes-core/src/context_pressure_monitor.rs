//! ContextPressureMonitor — Tiered context pressure monitoring
//!
//! Monitors context window usage and provides warnings at different thresholds:
//! - Normal (0-50%): No action needed
//! - Moderate (50-75%): Consider preparing for compression
//! - High (75-90%): Compression recommended
//! - Critical (90%+): Compression will occur soon

use serde::{Deserialize, Serialize};

/// Pressure level enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PressureLevel {
    /// Below moderate threshold
    Normal,
    /// 50-75% of context used
    Moderate,
    /// 75-90% of context used
    High,
    /// 90%+ of context used
    Critical,
}

/// Context pressure monitor
///
/// Monitors context window usage and provides tiered warnings.
pub struct ContextPressureMonitor {
    /// Total context window size
    context_length: usize,
    /// Moderate threshold (default: 50%)
    moderate_threshold: usize,
    /// High threshold (default: 75%)
    high_threshold: usize,
    /// Critical threshold (default: 90%)
    critical_threshold: usize,
}

impl ContextPressureMonitor {
    /// Create monitor with default thresholds (50%, 75%, 90%)
    pub fn new(context_length: usize) -> Self {
        Self {
            context_length,
            moderate_threshold: context_length * 50 / 100,
            high_threshold: context_length * 75 / 100,
            critical_threshold: context_length * 90 / 100,
        }
    }

    /// Create monitor with custom moderate and high thresholds
    pub fn with_custom_thresholds(context_length: usize, moderate: usize, high: usize) -> Self {
        Self {
            context_length,
            moderate_threshold: moderate,
            high_threshold: high,
            critical_threshold: context_length * 90 / 100,
        }
    }

    /// Get current pressure level based on token count
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

    /// Get warning message for current pressure level
    pub fn get_warning_message(&self, current_tokens: usize) -> String {
        let level = self.get_pressure_level(current_tokens);
        let percentage = (current_tokens as f64 / self.context_length as f64 * 100.0) as usize;

        match level {
            PressureLevel::Normal => String::new(),
            PressureLevel::Moderate => format!(
                "Context at {}% — Consider preparing to compress if conversation gets longer.",
                percentage
            ),
            PressureLevel::High => format!(
                "High context pressure ({}%) — Compression recommended.",
                percentage
            ),
            PressureLevel::Critical => format!(
                "Critical context pressure ({}%) — Compression will occur soon.",
                percentage
            ),
        }
    }

    /// Check if compression should be triggered proactively
    pub fn should_compress(&self, current_tokens: usize) -> bool {
        self.get_pressure_level(current_tokens) == PressureLevel::Critical
    }

    /// Get current usage ratio (0.0 to 1.0+)
    pub fn usage_ratio(&self, current_tokens: usize) -> f64 {
        current_tokens as f64 / self.context_length as f64
    }

    /// Get the moderate threshold value
    pub fn moderate_threshold(&self) -> usize {
        self.moderate_threshold
    }

    /// Get the high threshold value
    pub fn high_threshold(&self) -> usize {
        self.high_threshold
    }

    /// Get the critical threshold value
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

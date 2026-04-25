//! ContextPressureMonitor tests

use hermes_core::context_pressure_monitor::{ContextPressureMonitor, PressureLevel};

#[test]
fn test_pressure_levels() {
    let monitor = ContextPressureMonitor::new(100_000);
    // 0-50%: Normal
    assert_eq!(monitor.get_pressure_level(0), PressureLevel::Normal);
    assert_eq!(monitor.get_pressure_level(49_999), PressureLevel::Normal);
    // 50-75%: Moderate
    assert_eq!(monitor.get_pressure_level(50_000), PressureLevel::Moderate);
    assert_eq!(monitor.get_pressure_level(74_999), PressureLevel::Moderate);
    // 75-90%: High
    assert_eq!(monitor.get_pressure_level(75_000), PressureLevel::High);
    assert_eq!(monitor.get_pressure_level(89_999), PressureLevel::High);
    // 90%+: Critical
    assert_eq!(monitor.get_pressure_level(90_000), PressureLevel::Critical);
}

#[test]
fn test_warning_message_generation() {
    let monitor = ContextPressureMonitor::new(100_000);
    // Normal: no warning
    assert!(monitor.get_warning_message(30_000).is_empty());
    // Moderate (at 60%)
    let moderate = monitor.get_warning_message(60_000);
    assert!(moderate.contains("60%") || moderate.contains("Context at"));
    // High (at 80%)
    let high = monitor.get_warning_message(80_000);
    assert!(high.contains("High") || high.contains("80%"));
    // Critical (at 95%)
    let critical = monitor.get_warning_message(95_000);
    assert!(critical.contains("Critical") || critical.contains("95%"));
}

#[test]
fn test_should_compress() {
    let monitor = ContextPressureMonitor::new(100_000);
    assert!(!monitor.should_compress(50_000));
    assert!(!monitor.should_compress(89_999));
    assert!(monitor.should_compress(90_000));
    assert!(monitor.should_compress(100_000));
}

#[test]
fn test_usage_ratio() {
    let monitor = ContextPressureMonitor::new(100_000);
    assert!((monitor.usage_ratio(50_000) - 0.5).abs() < 0.001);
    assert!((monitor.usage_ratio(75_000) - 0.75).abs() < 0.001);
}

#[test]
fn test_custom_thresholds() {
    let monitor = ContextPressureMonitor::with_custom_thresholds(100_000, 40_000, 70_000);
    // 40% should now be Moderate
    assert_eq!(monitor.get_pressure_level(40_000), PressureLevel::Moderate);
    // 70% should now be High
    assert_eq!(monitor.get_pressure_level(70_000), PressureLevel::High);
}

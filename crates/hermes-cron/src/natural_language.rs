//! NaturalLanguageParser — 自然语言转 Cron 表达式
//!
//! 支持中文和英文自然语言描述转换为 cron 表达式

use std::collections::HashMap;

/// 自然语言到 Cron 表达式的解析器
#[derive(Debug, Clone, Default)]
pub struct NaturalLanguageParser {
    // 预留用于未来扩展
}

impl NaturalLanguageParser {
    pub fn new() -> Self {
        Self {}
    }

    /// 将自然语言解析为 cron 表达式
    pub fn parse(&self, input: &str) -> Option<String> {
        let input = input.trim();

        // 尝试中文
        if let Some(expr) = self.parse_chinese(input) {
            return Some(expr);
        }

        // 尝试英文
        if let Some(expr) = self.parse_english(input) {
            return Some(expr);
        }

        None
    }

    fn parse_chinese(&self, input: &str) -> Option<String> {
        // 每天早上9点 -> "0 9 * * *"
        if (input.contains("每天") || input.contains("每日"))
            && (input.contains("早上") || input.contains("上午"))
        {
            if let Some(hour) = extract_hour_cn(input) {
                return Some(format!("0 {} * * *", hour));
            }
        }

        // 下午时间
        if (input.contains("每天") || input.contains("每日")) && input.contains("下午") {
            if let Some(hour) = extract_hour_cn_pm(input) {
                return Some(format!("0 {} * * *", hour));
            }
        }

        // 每隔N分钟
        if input.contains("每隔") && input.contains("分钟") {
            if let Some(mins) = extract_every_n_minutes_cn(input) {
                return Some(format!("*/{} * * * *", mins));
            }
        }

        // 每隔N小时
        if input.contains("每隔") && (input.contains("小时") || input.contains("时")) {
            if let Some(hours) = extract_every_n_hours_cn(input) {
                return Some(format!("0 */{} * * *", hours));
            }
        }

        // 工作日每半小时
        if (input.contains("工作日") || input.contains("平日")) && input.contains("半") {
            return Some("*/30 9-18 * * 1-5".to_string());
        }

        // 每周某天
        if input.contains("每周") || input.contains("每逢") {
            if let Some((day, hour)) = extract_weekly_cn(input) {
                return Some(format!("0 {} * * {}", day, hour));
            }
        }

        None
    }

    fn parse_english(&self, input: &str) -> Option<String> {
        let input_lower = input.to_lowercase();

        // "every 5 minutes"
        if input_lower.contains("every") && input_lower.contains("minute") {
            if let Some(n) = extract_every_n_minutes_en(&input_lower) {
                return Some(format!("*/{} * * * *", n));
            }
        }

        // "every N hours"
        if input_lower.contains("every") && input_lower.contains("hour") {
            if let Some(n) = extract_every_n_hours_en(&input_lower) {
                return Some(format!("0 */{} * * *", n));
            }
        }

        // "daily at 9am" / "every day at 9am"
        if input_lower.contains("daily") || input_lower.contains("every day") {
            if let Some(hour) = extract_hour_en(&input_lower, "am") {
                return Some(format!("0 {} * * *", hour));
            }
        }

        // "weekdays at 9am"
        if input_lower.contains("weekday") {
            if let Some(hour) = extract_hour_en(&input_lower, "am") {
                return Some(format!("0 {} * * 1-5", hour));
            }
        }

        None
    }
}

// ============== Helper functions ==============

fn extract_hour_cn(input: &str) -> Option<u8> {
    // 匹配 "早上9点", "早上10点" 等
    let patterns = [
        ("早上", "点"),
        ("上午", "点"),
    ];

    for (prefix, _suffix) in &patterns {
        if let Some(pos) = input.find(prefix) {
            let after_prefix = &input[pos + prefix.len()..];
            // 提取数字
            let num_str: String = after_prefix.chars().take_while(|c| c.is_ascii_digit()).collect();
            if !num_str.is_empty() {
                if let Ok(hour) = num_str.parse::<u8>() {
                    if hour <= 23 {
                        return Some(hour);
                    }
                }
            }
        }
    }
    None
}

fn extract_hour_cn_pm(input: &str) -> Option<u8> {
    // 匹配 "下午3点" -> 15
    if let Some(pos) = input.find("下午") {
        let after = &input[pos + 2..];
        let num_str: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
        if !num_str.is_empty() {
            if let Ok(hour) = num_str.parse::<u8>() {
                if hour <= 12 {
                    return Some(hour + 12); // PM: add 12
                }
            }
        }
    }
    None
}

fn extract_every_n_minutes_cn(input: &str) -> Option<u8> {
    // 匹配 "每隔5分钟"
    if let Some(start) = input.find("每隔") {
        let after = &input[start + "每隔".len()..];
        let num_str: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
        if !num_str.is_empty() {
            if let Ok(mins) = num_str.parse::<u8>() {
                if mins > 0 && mins <= 59 {
                    return Some(mins);
                }
            }
        }
    }
    None
}

fn extract_every_n_hours_cn(input: &str) -> Option<u8> {
    // 匹配 "每隔1小时"
    if let Some(start) = input.find("每隔") {
        let after = &input[start + "每隔".len()..];
        let num_str: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
        if !num_str.is_empty() {
            if let Ok(hours) = num_str.parse::<u8>() {
                if hours > 0 && hours <= 23 {
                    return Some(hours);
                }
            }
        }
    }
    None
}

fn extract_weekly_cn(input: &str) -> Option<(u8, u8)> {
    let day_map: HashMap<&str, u8> = [
        ("周日", 0), ("星期日", 0), ("周日", 0),
        ("周一", 1), ("星期一", 1),
        ("周二", 2), ("星期二", 2),
        ("周三", 3), ("星期三", 3),
        ("周四", 4), ("星期四", 4),
        ("周五", 5), ("星期五", 5),
        ("周六", 6), ("星期六", 6),
    ].into_iter().collect();

    for (day_name, day_num) in &day_map {
        if let Some(pos) = input.find(day_name) {
            let after = &input[pos + day_name.len()..];
            // 提取时间 "10点"
            if let Some(hour_str_start) = after.find(char::is_numeric) {
                let hour_str: String = after[hour_str_start..].chars().take_while(|c| c.is_ascii_digit()).collect();
                if let Ok(hour) = hour_str.parse::<u8>() {
                    if hour <= 23 {
                        return Some((*day_num, hour));
                    }
                }
            }
        }
    }
    None
}

fn extract_every_n_minutes_en(input: &str) -> Option<u8> {
    // 匹配 "every 5 minutes"
    let pattern = "every ";
    if let Some(start) = input.find(pattern) {
        let after = &input[start + pattern.len()..];
        let num_str: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
        if !num_str.is_empty() {
            if let Ok(n) = num_str.parse::<u8>() {
                if n > 0 && n <= 59 {
                    return Some(n);
                }
            }
        }
    }
    None
}

fn extract_every_n_hours_en(input: &str) -> Option<u8> {
    // 匹配 "every 2 hours"
    let pattern = "every ";
    if let Some(start) = input.find(pattern) {
        let after = &input[start + pattern.len()..];
        let num_str: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
        if !num_str.is_empty() {
            if let Ok(n) = num_str.parse::<u8>() {
                if n > 0 && n <= 23 {
                    return Some(n);
                }
            }
        }
    }
    None
}

fn extract_hour_en(input: &str, suffix: &str) -> Option<u8> {
    // 匹配 " at 9am" 或 " at 9 pm"
    let pattern = " at ";
    if let Some(start) = input.find(pattern) {
        let after = &input[start + pattern.len()..];
        let num_str: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
        if !num_str.is_empty() {
            if let Ok(hour) = num_str.parse::<u8>() {
                let suffix_pos = after.find(suffix).unwrap_or(usize::MAX);
                let num_part_len = num_str.len();
                if suffix_pos < num_part_len + 3 { // "9am" 或 "9 am"
                    if hour <= 12 {
                        // AM: return as-is (12am = 0)
                        if hour == 12 && suffix.starts_with("am") {
                            return Some(0);
                        }
                        // PM: add 12 (except 12pm = 12)
                        if suffix.starts_with("pm") && hour != 12 {
                            return Some(hour + 12);
                        }
                        if suffix.starts_with("am") || hour != 12 {
                            return Some(hour);
                        }
                    }
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chinese_daily_at_9am() {
        let parser = NaturalLanguageParser::new();
        assert_eq!(parser.parse("每天早上9点"), Some("0 9 * * *".to_string()));
        assert_eq!(parser.parse("每日早上9点"), Some("0 9 * * *".to_string()));
    }

    #[test]
    fn test_chinese_every_5_minutes() {
        let parser = NaturalLanguageParser::new();
        assert_eq!(parser.parse("每隔5分钟"), Some("*/5 * * * *".to_string()));
    }

    #[test]
    fn test_chinese_every_hour() {
        let parser = NaturalLanguageParser::new();
        assert_eq!(parser.parse("每隔1小时"), Some("0 */1 * * *".to_string()));
    }

    #[test]
    fn test_chinese_workday_half_hour() {
        let parser = NaturalLanguageParser::new();
        assert_eq!(parser.parse("工作日每半小时"), Some("*/30 9-18 * * 1-5".to_string()));
    }

    #[test]
    fn test_english_every_5_minutes() {
        let parser = NaturalLanguageParser::new();
        assert_eq!(parser.parse("every 5 minutes"), Some("*/5 * * * *".to_string()));
    }

    #[test]
    fn test_english_daily_at_9am() {
        let parser = NaturalLanguageParser::new();
        assert_eq!(parser.parse("daily at 9am"), Some("0 9 * * *".to_string()));
    }

    #[test]
    fn test_english_weekdays() {
        let parser = NaturalLanguageParser::new();
        assert_eq!(parser.parse("weekdays at 9am"), Some("0 9 * * 1-5".to_string()));
    }

    #[test]
    fn test_invalid_input() {
        let parser = NaturalLanguageParser::new();
        assert_eq!(parser.parse("asdfghjkl"), None);
    }
}

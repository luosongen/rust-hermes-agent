//! Usage Pricing — 成本估算与定价数据库
//!
//! 提供各 Provider 的 token 定价和成本计算。

use crate::Usage;
use std::collections::HashMap;

/// 定价层
#[derive(Debug, Clone)]
pub struct PricingTier {
    /// $ / 1M input tokens
    pub input_per_million: f64,
    /// $ / 1M output tokens
    pub output_per_million: f64,
    /// $ / 1M cache read tokens
    pub cache_read_per_million: f64,
    /// $ / 1M cache write tokens
    pub cache_write_per_million: f64,
}

/// 定价数据库
pub struct PricingDatabase {
    tiers: HashMap<String, HashMap<String, PricingTier>>,
}

impl PricingDatabase {
    pub fn new() -> Self {
        let mut db = Self { tiers: HashMap::new() };

        // OpenAI
        db.tiers.insert("openai".to_string(), HashMap::from([
            ("gpt-4o".to_string(), PricingTier {
                input_per_million: 5.00,
                output_per_million: 15.00,
                cache_read_per_million: 1.25,
                cache_write_per_million: 10.00,
            }),
            ("gpt-4o-mini".to_string(), PricingTier {
                input_per_million: 0.15,
                output_per_million: 0.60,
                cache_read_per_million: 0.04,
                cache_write_per_million: 0.50,
            }),
            ("gpt-4-turbo".to_string(), PricingTier {
                input_per_million: 10.00,
                output_per_million: 30.00,
                cache_read_per_million: 1.25,
                cache_write_per_million: 10.00,
            }),
        ]));

        // Anthropic
        db.tiers.insert("anthropic".to_string(), HashMap::from([
            ("claude-3-5-sonnet-20241022".to_string(), PricingTier {
                input_per_million: 3.00,
                output_per_million: 15.00,
                cache_read_per_million: 0.30,
                cache_write_per_million: 3.75,
            }),
            ("claude-3-opus".to_string(), PricingTier {
                input_per_million: 15.00,
                output_per_million: 75.00,
                cache_read_per_million: 1.50,
                cache_write_per_million: 18.75,
            }),
            ("claude-3-haiku".to_string(), PricingTier {
                input_per_million: 0.25,
                output_per_million: 1.25,
                cache_read_per_million: 0.03,
                cache_write_per_million: 0.30,
            }),
        ]));

        // DeepSeek
        db.tiers.insert("deepseek".to_string(), HashMap::from([
            ("deepseek-chat".to_string(), PricingTier {
                input_per_million: 0.14,
                output_per_million: 0.28,
                cache_read_per_million: 0.01,
                cache_write_per_million: 0.14,
            }),
        ]));

        db
    }

    pub fn get_pricing(&self, provider: &str, model: &str) -> Option<&PricingTier> {
        self.tiers.get(provider)?.get(model)
    }
}

impl Default for PricingDatabase {
    fn default() -> Self {
        Self::new()
    }
}

/// 成本计算器
pub struct CostCalculator<'a> {
    pricing: &'a PricingDatabase,
}

impl<'a> CostCalculator<'a> {
    pub fn new(pricing: &'a PricingDatabase) -> Self {
        Self { pricing }
    }

    pub fn calculate(&self, provider: &str, model: &str, usage: &Usage) -> Option<f64> {
        let tier = self.pricing.get_pricing(provider, model)?;

        let input_cost = (usage.input_tokens as f64 / 1_000_000.0) * tier.input_per_million;
        let output_cost = (usage.output_tokens as f64 / 1_000_000.0) * tier.output_per_million;
        let cache_read_cost = usage.cache_read_tokens
            .map(|t| (t as f64 / 1_000_000.0) * tier.cache_read_per_million)
            .unwrap_or(0.0);
        let cache_write_cost = usage.cache_write_tokens
            .map(|t| (t as f64 / 1_000_000.0) * tier.cache_write_per_million)
            .unwrap_or(0.0);

        Some(input_cost + output_cost + cache_read_cost + cache_write_cost)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cost_calculation_gpt4o() {
        let pricing = PricingDatabase::new();
        let calculator = CostCalculator::new(&pricing);

        let usage = Usage {
            input_tokens: 1000,
            output_tokens: 2000,
            cache_read_tokens: Some(500),
            cache_write_tokens: Some(100),
            reasoning_tokens: None,
        };

        // 1000/1M * $5.00 = $0.005
        // 2000/1M * $15.00 = $0.030
        // 500/1M * $1.25 = $0.000625
        // 100/1M * $10.00 = $0.001
        // Total = $0.036625
        let cost = calculator.calculate("openai", "gpt-4o", &usage).unwrap();
        assert!((cost - 0.036625).abs() < 0.0001);
    }

    #[test]
    fn test_unknown_model_returns_none() {
        let pricing = PricingDatabase::new();
        let calculator = CostCalculator::new(&pricing);

        let usage = Usage {
            input_tokens: 100,
            output_tokens: 100,
            cache_read_tokens: None,
            cache_write_tokens: None,
            reasoning_tokens: None,
        };

        assert!(calculator.calculate("openai", "unknown-model", &usage).is_none());
    }
}

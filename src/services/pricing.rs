use std::collections::HashMap;

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct ModelPricing {
    #[serde(default)]
    pub input_cost_per_token: f64,
    #[serde(default)]
    pub output_cost_per_token: f64,
    #[serde(default)]
    pub cache_read_input_token_cost: f64,
    #[serde(default)]
    pub cache_creation_input_token_cost: f64,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PricingSource {
    Litellm,
    #[default]
    Fallback,
}

#[derive(Debug, Clone, Default)]
pub struct PricingTable {
    pub models: HashMap<String, ModelPricing>,
    pub fetched_at: i64,
    pub source: PricingSource,
}

impl PricingTable {
    pub fn parse_from_json(json: &str, source: PricingSource) -> Result<Self, serde_json::Error> {
        let models: HashMap<String, ModelPricing> = serde_json::from_str(json)?;
        Ok(Self {
            models,
            fetched_at: chrono::Utc::now().timestamp(),
            source,
        })
    }

    pub fn cost(
        &self,
        model: &str,
        input: u64,
        output: u64,
        cache_read: u64,
        cache_create: u64,
    ) -> f64 {
        let Some(p) = self.models.get(model) else {
            return 0.0;
        };
        (input as f64) * p.input_cost_per_token
            + (output as f64) * p.output_cost_per_token
            + (cache_read as f64) * p.cache_read_input_token_cost
            + (cache_create as f64) * p.cache_creation_input_token_cost
    }
}

const FALLBACK_JSON: &str = include_str!("../../resources/pricing_fallback.json");

pub fn load_fallback() -> PricingTable {
    PricingTable::parse_from_json(FALLBACK_JSON, PricingSource::Fallback)
        .expect("bundled pricing_fallback.json must be valid JSON")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cost_zero_for_unknown_model() {
        let t = load_fallback();
        assert_eq!(t.cost("nonexistent-model", 1000, 1000, 0, 0), 0.0);
    }

    #[test]
    fn cost_for_sonnet_matches_known_rates() {
        let t = load_fallback();
        // 1M input + 1M output @ Sonnet 4.5 = $3 + $15 = $18
        let cost = t.cost("claude-sonnet-4-5-20250929", 1_000_000, 1_000_000, 0, 0);
        assert!((cost - 18.0).abs() < 0.01, "expected ~18.0, got {}", cost);
    }

    #[test]
    fn cost_includes_cache_tokens() {
        let t = load_fallback();
        // 1M cache-read @ Sonnet = $0.30
        let cost = t.cost("claude-sonnet-4-5-20250929", 0, 0, 1_000_000, 0);
        assert!((cost - 0.30).abs() < 0.001, "expected ~0.30, got {}", cost);
    }

    #[test]
    fn fallback_loads_without_panic() {
        let t = load_fallback();
        assert!(!t.models.is_empty());
        assert!(matches!(t.source, PricingSource::Fallback));
    }
}

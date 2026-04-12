use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Canonical summary payload derived from a probabilistic forecast model.
///
/// The intent is to cache a stable, replayable reduction of a model's raw
/// sampled forecasts rather than depending on live stochastic inference.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForecastSummary {
    pub instrument: String,
    pub eval_date: String,
    pub horizon_days: u32,
    pub lookback_bars: usize,
    pub sample_count: usize,
    pub target_field: String,
    pub forecast_return: ReturnSummary,
    pub probabilities: ProbabilitySummary,
    pub distribution: DistributionSummary,
    pub diagnostics: ForecastDiagnostics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReturnSummary {
    pub mean: f64,
    pub median: f64,
    pub std: f64,
    pub p05: f64,
    pub p25: f64,
    pub p75: f64,
    pub p95: f64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProbabilitySummary {
    #[serde(default)]
    pub return_lt_0: Option<f64>,
    #[serde(default)]
    pub return_lt_neg_1pct: Option<f64>,
    #[serde(default)]
    pub return_lt_neg_2pct: Option<f64>,
    #[serde(default)]
    pub return_lt_neg_5pct: Option<f64>,
    #[serde(default)]
    pub return_gt_1pct: Option<f64>,
    /// Optional extensible probability map for additional thresholds.
    #[serde(default)]
    pub extra: HashMap<String, f64>,
}

impl ProbabilitySummary {
    /// Look up a downside probability for a canonical return threshold.
    ///
    /// Supported thresholds:
    /// -  0.00  -> return_lt_0
    /// - -0.01 -> return_lt_neg_1pct
    /// - -0.02 -> return_lt_neg_2pct
    /// - -0.05 -> return_lt_neg_5pct
    ///
    /// Any other threshold falls back to the `extra` map using a canonical key
    /// like `return_lt_neg_3pct`.
    pub fn downside_probability(&self, threshold: f64) -> Option<f64> {
        match threshold {
            x if (x - 0.0).abs() < 1e-12 => self.return_lt_0,
            x if (x + 0.01).abs() < 1e-12 => self.return_lt_neg_1pct,
            x if (x + 0.02).abs() < 1e-12 => self.return_lt_neg_2pct,
            x if (x + 0.05).abs() < 1e-12 => self.return_lt_neg_5pct,
            x => {
                let pct = (x.abs() * 100.0).round() as i64;
                let key = if x < 0.0 {
                    format!("return_lt_neg_{pct}pct")
                } else {
                    format!("return_lt_pos_{pct}pct")
                };
                self.extra.get(&key).copied()
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistributionSummary {
    pub iqr: f64,
    pub tail_width_90: f64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ForecastDiagnostics {
    #[serde(default)]
    pub input_truncated: bool,
    #[serde(default)]
    pub sampling_temperature: Option<f64>,
    #[serde(default)]
    pub top_p: Option<f64>,
    #[serde(default)]
    pub notes: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn downside_probability_supports_named_fields() {
        let p = ProbabilitySummary {
            return_lt_0: Some(0.6),
            return_lt_neg_1pct: Some(0.4),
            return_lt_neg_2pct: Some(0.2),
            return_lt_neg_5pct: Some(0.05),
            return_gt_1pct: None,
            extra: HashMap::new(),
        };

        assert_eq!(p.downside_probability(0.0), Some(0.6));
        assert_eq!(p.downside_probability(-0.01), Some(0.4));
        assert_eq!(p.downside_probability(-0.02), Some(0.2));
        assert_eq!(p.downside_probability(-0.05), Some(0.05));
    }

    #[test]
    fn downside_probability_falls_back_to_extra_map() {
        let mut extra = HashMap::new();
        extra.insert("return_lt_neg_3pct".to_string(), 0.17);
        let p = ProbabilitySummary {
            return_lt_0: None,
            return_lt_neg_1pct: None,
            return_lt_neg_2pct: None,
            return_lt_neg_5pct: None,
            return_gt_1pct: None,
            extra,
        };
        assert_eq!(p.downside_probability(-0.03), Some(0.17));
    }
}

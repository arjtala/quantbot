use std::collections::HashMap;

use chrono::{Duration, NaiveDate};

use crate::config::{BlendCategory, KronosOverlayConfig, ResolvedKronosThresholds};
use crate::forecast::ForecastSummary;
use crate::overlay::{symbol_category, OverlayAction, OverlayScope};

#[derive(Debug, Clone)]
pub struct KronosTriggerResult {
    pub instrument: String,
    pub category: BlendCategory,
    pub horizon_days: u32,
    pub std_return: f64,
    pub downside_prob: Option<f64>,
    pub std_history_count: usize,
    pub std_quantile: Option<f64>,
}

impl KronosTriggerResult {
    pub fn scale_triggered(&self, cfg: &KronosOverlayConfig) -> bool {
        self.std_quantile
            .is_some_and(|q| q >= cfg.thresholds.std_quantile_trigger)
    }

    pub fn severe_triggered(
        &self,
        resolved: &ResolvedKronosThresholds,
        cfg: &KronosOverlayConfig,
    ) -> bool {
        let severe_std = self
            .std_quantile
            .is_some_and(|q| q >= cfg.thresholds.std_quantile_severe);

        let severe_tail = match self.horizon_days {
            5 => self
                .downside_prob
                .is_some_and(|p| p >= resolved.tail_prob_5d_neg_2pct),
            21 => self
                .downside_prob
                .is_some_and(|p| p >= resolved.tail_prob_21d_neg_5pct),
            _ => false,
        };

        severe_std || severe_tail
    }
}

fn percentile_rank(history: &[f64], value: f64) -> Option<f64> {
    if history.is_empty() {
        return None;
    }
    let count = history.iter().filter(|&&v| v <= value).count();
    Some(count as f64 / history.len() as f64)
}

fn downside_probability_for(summary: &ForecastSummary) -> Option<f64> {
    match summary.horizon_days {
        5 => summary.probabilities.downside_probability(-0.02),
        21 => summary.probabilities.downside_probability(-0.05),
        1 => summary.probabilities.downside_probability(0.0),
        _ => None,
    }
}

/// Compute Kronos-driven overlay actions from cached forecast summaries.
///
/// `history` should contain trailing summaries for each instrument, ideally
/// including the current `eval_date` summary plus prior dates for percentile
/// ranking of forecast dispersion.
pub fn compute_kronos_actions(
    history: &HashMap<String, Vec<ForecastSummary>>,
    eval_date: NaiveDate,
    cfg: &KronosOverlayConfig,
) -> (Vec<OverlayAction>, Vec<KronosTriggerResult>) {
    if !cfg.enabled {
        return (vec![], vec![]);
    }

    let mut actions = Vec::new();
    let mut triggers = Vec::new();

    for (instrument, summaries) in history {
        let Some(current) = summaries.iter().find(|s| {
            s.eval_date == eval_date.to_string() && cfg.horizons.contains(&s.horizon_days)
        }) else {
            continue;
        };

        let std_history: Vec<f64> = summaries
            .iter()
            .filter(|s| s.horizon_days == current.horizon_days && s.eval_date <= current.eval_date)
            .map(|s| s.forecast_return.std)
            .collect();

        let quantile = percentile_rank(&std_history, current.forecast_return.std);
        let category = symbol_category(instrument);
        let resolved = cfg.thresholds_for(category);

        let trigger = KronosTriggerResult {
            instrument: instrument.clone(),
            category,
            horizon_days: current.horizon_days,
            std_return: current.forecast_return.std,
            downside_prob: downside_probability_for(current),
            std_history_count: std_history.len(),
            std_quantile: quantile,
        };

        if trigger.severe_triggered(&resolved, cfg) {
            let until = eval_date + Duration::days(resolved.freeze_days as i64);
            actions.push(OverlayAction::FreezeEntries {
                scope: OverlayScope::AssetClass(category),
                until,
            });
        } else if trigger.scale_triggered(cfg) {
            let until = eval_date + Duration::days(resolved.scale_days as i64);
            actions.push(OverlayAction::ScaleExposure {
                scope: OverlayScope::AssetClass(category),
                factor: resolved.scale_factor,
                until,
            });
        }

        triggers.push(trigger);
    }

    (actions, triggers)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::KronosThresholdConfig;
    use crate::forecast::{
        DistributionSummary, ForecastDiagnostics, ProbabilitySummary, ReturnSummary,
    };

    fn summary(
        instrument: &str,
        eval_date: &str,
        horizon_days: u32,
        std: f64,
        p_neg: Option<f64>,
    ) -> ForecastSummary {
        let mut probs = ProbabilitySummary::default();
        match horizon_days {
            5 => probs.return_lt_neg_2pct = p_neg,
            21 => probs.return_lt_neg_5pct = p_neg,
            1 => probs.return_lt_0 = p_neg,
            _ => {}
        }
        ForecastSummary {
            instrument: instrument.to_string(),
            eval_date: eval_date.to_string(),
            horizon_days,
            lookback_bars: 512,
            sample_count: 64,
            target_field: "close".into(),
            forecast_return: ReturnSummary {
                mean: 0.0,
                median: 0.0,
                std,
                p05: -0.01,
                p25: -0.005,
                p75: 0.005,
                p95: 0.01,
            },
            probabilities: probs,
            distribution: DistributionSummary {
                iqr: 0.01,
                tail_width_90: 0.03,
            },
            diagnostics: ForecastDiagnostics::default(),
        }
    }

    fn cfg() -> KronosOverlayConfig {
        KronosOverlayConfig {
            enabled: true,
            model_name: "NeoQuasar/Kronos-mini".into(),
            model_version: "v1".into(),
            tokenizer_name: "NeoQuasar/Kronos-Tokenizer-2k".into(),
            lookback_bars: 512,
            sample_count: 64,
            temperature: 1.0,
            top_p: 0.9,
            horizons: vec![5, 21],
            target_field: "close".into(),
            cache_dir: "data".into(),
            thresholds: KronosThresholdConfig {
                std_quantile_trigger: 0.80,
                std_quantile_severe: 1.01,
                tail_prob_5d_neg_2pct: 0.40,
                tail_prob_21d_neg_5pct: 0.35,
            },
            gold: None,
            equity: None,
            forex: None,
        }
    }

    #[test]
    fn high_std_quantile_emits_scale() {
        let mut history = HashMap::new();
        history.insert(
            "SPY".into(),
            vec![
                summary("SPY", "2025-03-27", 5, 0.01, Some(0.10)),
                summary("SPY", "2025-03-28", 5, 0.02, Some(0.15)),
                summary("SPY", "2025-03-31", 5, 0.05, Some(0.20)),
            ],
        );
        let eval = NaiveDate::from_ymd_opt(2025, 3, 31).unwrap();
        let (actions, triggers) = compute_kronos_actions(&history, eval, &cfg());
        assert_eq!(triggers.len(), 1);
        assert_eq!(actions.len(), 1);
        assert!(matches!(
            actions[0],
            OverlayAction::ScaleExposure {
                scope: OverlayScope::AssetClass(BlendCategory::Equity),
                ..
            }
        ));
    }

    #[test]
    fn severe_tail_emits_freeze() {
        let mut history = HashMap::new();
        history.insert(
            "GLD".into(),
            vec![
                summary("GLD", "2025-03-27", 5, 0.01, Some(0.10)),
                summary("GLD", "2025-03-28", 5, 0.02, Some(0.12)),
                summary("GLD", "2025-03-31", 5, 0.03, Some(0.50)),
            ],
        );
        let eval = NaiveDate::from_ymd_opt(2025, 3, 31).unwrap();
        let (actions, _) = compute_kronos_actions(&history, eval, &cfg());
        assert_eq!(actions.len(), 1);
        assert!(matches!(
            actions[0],
            OverlayAction::FreezeEntries {
                scope: OverlayScope::AssetClass(BlendCategory::Gold),
                ..
            }
        ));
    }
}

use std::collections::HashMap;

use crate::agents::tsmom::TSMOMAgent;
use crate::config::{BlendCategory, BlendConfig};
use crate::core::signal::{Signal, SignalDirection, SignalType};

/// Per-instrument combination result with full provenance.
#[derive(Debug, Clone)]
pub struct CombinedResult {
    pub instrument: String,
    pub combined_weight: f64,
    pub tsmom_weight: f64,
    pub indicator_weight: f64,
    pub blend_tsmom: f64,
    pub blend_indicator: f64,
    pub blend_category: BlendCategory,
    pub indicator_used: bool,
    pub latency_ms: Option<f64>,
}

/// Map instrument symbol to blend category.
/// GLD/GC=F → Gold, SPY → Equity, *=X → Forex, fallback → Equity.
pub fn blend_category(symbol: &str) -> BlendCategory {
    match symbol {
        "GLD" | "GC=F" => BlendCategory::Gold,
        "SPY" => BlendCategory::Equity,
        s if s.ends_with("=X") => BlendCategory::Forex,
        _ => {
            eprintln!("  WARN: unknown symbol '{symbol}' for blending — defaulting to Equity");
            BlendCategory::Equity
        }
    }
}

/// Returns true if the indicator signal should be used for blending.
fn should_use_indicator(sig: &Signal) -> bool {
    if sig.direction == SignalDirection::Flat {
        return false;
    }
    if sig.confidence == 0.0 {
        return false;
    }
    if sig.metadata.get("llm_success").copied() == Some(0.0) {
        return false;
    }
    true
}

/// Combine TSMOM + indicator signals into portfolio weights.
///
/// Returns a `Vec<CombinedResult>` with one entry per instrument that has
/// at least a TSMOM signal. Instruments with no TSMOM signal are skipped.
pub fn combine_signals(
    tsmom_signals: &HashMap<String, Signal>,
    indicator_signals: &HashMap<String, Signal>,
    blend_config: &BlendConfig,
) -> Vec<CombinedResult> {
    let mut results = Vec::new();

    for (sym, tsmom_sig) in tsmom_signals {
        let tsmom_w = TSMOMAgent::compute_target_weight(tsmom_sig);
        let vol_scalar = tsmom_sig.metadata.get("vol_scalar").copied().unwrap_or(1.0);

        let cat = blend_category(sym);
        let blend_w = blend_config.weights_for(cat);

        let (combined_weight, indicator_w, indicator_used, latency_ms) =
            match indicator_signals.get(sym) {
                Some(ind_sig) if should_use_indicator(ind_sig) => {
                    let ind_w = ind_sig.strength * ind_sig.confidence * vol_scalar;
                    let combined = blend_w.tsmom * tsmom_w + blend_w.indicator * ind_w;
                    let latency = ind_sig.metadata.get("latency_ms").copied();
                    (combined, ind_w, true, latency)
                }
                _ => (tsmom_w, 0.0, false, None),
            };

        results.push(CombinedResult {
            instrument: sym.clone(),
            combined_weight,
            tsmom_weight: tsmom_w,
            indicator_weight: indicator_w,
            blend_tsmom: blend_w.tsmom,
            blend_indicator: blend_w.indicator,
            blend_category: cat,
            indicator_used,
            latency_ms,
        });
    }

    results.sort_by(|a, b| a.instrument.cmp(&b.instrument));
    results
}

/// Build a combined Signal from a CombinedResult and the original signals.
pub fn build_combined_signal(
    result: &CombinedResult,
    tsmom_sig: &Signal,
    indicator_sig: Option<&Signal>,
) -> Signal {
    let direction = if result.combined_weight > 0.0 {
        SignalDirection::Long
    } else if result.combined_weight < 0.0 {
        SignalDirection::Short
    } else {
        SignalDirection::Flat
    };

    let strength = result.combined_weight.abs().min(1.0);
    let strength = if result.combined_weight < 0.0 {
        -strength
    } else {
        strength
    };

    // Weighted average of component confidences
    let confidence = if result.indicator_used {
        let ind_conf = indicator_sig.map(|s| s.confidence).unwrap_or(0.0);
        let total = result.blend_tsmom + result.blend_indicator;
        if total > 0.0 {
            (result.blend_tsmom * tsmom_sig.confidence + result.blend_indicator * ind_conf) / total
        } else {
            tsmom_sig.confidence
        }
    } else {
        tsmom_sig.confidence
    };

    let mut metadata = HashMap::new();
    metadata.insert("tsmom_weight".into(), result.tsmom_weight);
    metadata.insert("indicator_weight".into(), result.indicator_weight);
    metadata.insert("blend_tsmom".into(), result.blend_tsmom);
    metadata.insert("blend_indicator".into(), result.blend_indicator);
    if let Some(vs) = tsmom_sig.metadata.get("vol_scalar") {
        metadata.insert("vol_scalar".into(), *vs);
    }
    if let Some(av) = tsmom_sig.metadata.get("ann_vol") {
        metadata.insert("ann_vol".into(), *av);
    }
    if let Some(lat) = result.latency_ms {
        metadata.insert("latency_ms".into(), lat);
    }
    metadata.insert("indicator_used".into(), if result.indicator_used { 1.0 } else { 0.0 });

    let mut sig = Signal::new(
        result.instrument.clone(),
        direction,
        strength,
        confidence.clamp(0.0, 1.0),
        "combined".into(),
        SignalType::Combined,
    )
    .expect("combined signal values are always valid (clamped)");
    sig.metadata = metadata;
    sig
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::signal::{Signal, SignalDirection, SignalType};

    fn make_signal(
        instrument: &str,
        direction: SignalDirection,
        strength: f64,
        confidence: f64,
        agent_name: &str,
        vol_scalar: f64,
    ) -> Signal {
        let mut sig = Signal::new(
            instrument.into(),
            direction,
            strength,
            confidence,
            agent_name.into(),
            SignalType::Quant,
        )
        .unwrap();
        sig.metadata.insert("vol_scalar".into(), vol_scalar);
        sig
    }

    fn make_indicator_signal(
        instrument: &str,
        direction: SignalDirection,
        strength: f64,
        confidence: f64,
    ) -> Signal {
        Signal::new(
            instrument.into(),
            direction,
            strength,
            confidence,
            "indicator".into(),
            SignalType::Llm,
        )
        .unwrap()
    }

    fn gold_50_50_config() -> BlendConfig {
        let mut weights = HashMap::new();
        weights.insert(
            BlendCategory::Gold,
            crate::config::BlendWeights {
                tsmom: 0.5,
                indicator: 0.5,
            },
        );
        weights.insert(
            BlendCategory::Equity,
            crate::config::BlendWeights {
                tsmom: 1.0,
                indicator: 0.0,
            },
        );
        weights.insert(
            BlendCategory::Forex,
            crate::config::BlendWeights {
                tsmom: 0.1,
                indicator: 0.9,
            },
        );
        BlendConfig {
            enabled: true,
            weights,
        }
    }

    #[test]
    fn basic_gold_50_50() {
        let config = gold_50_50_config();
        let mut tsmom = HashMap::new();
        tsmom.insert("GLD".into(), make_signal("GLD", SignalDirection::Long, 0.8, 1.0, "tsmom", 2.0));
        let mut indicator = HashMap::new();
        indicator.insert("GLD".into(), make_indicator_signal("GLD", SignalDirection::Long, 0.6, 0.7));

        let results = combine_signals(&tsmom, &indicator, &config);
        assert_eq!(results.len(), 1);
        let r = &results[0];

        // tsmom_w = 0.8 * 1.0 * 2.0 = 1.6
        // indicator_w = 0.6 * 0.7 * 2.0 = 0.84
        // combined = 0.5 * 1.6 + 0.5 * 0.84 = 1.22
        assert!((r.tsmom_weight - 1.6).abs() < 1e-10);
        assert!((r.indicator_weight - 0.84).abs() < 1e-10);
        assert!((r.combined_weight - 1.22).abs() < 1e-10);
        assert!(r.indicator_used);
        assert_eq!(r.blend_category, BlendCategory::Gold);
    }

    #[test]
    fn equity_100_0_passthrough() {
        let config = gold_50_50_config();
        let mut tsmom = HashMap::new();
        tsmom.insert("SPY".into(), make_signal("SPY", SignalDirection::Short, -0.5, 0.9, "tsmom", 1.5));
        let mut indicator = HashMap::new();
        indicator.insert("SPY".into(), make_indicator_signal("SPY", SignalDirection::Long, 0.8, 0.8));

        let results = combine_signals(&tsmom, &indicator, &config);
        let r = &results[0];

        // equity = 100% tsmom, 0% indicator
        let tsmom_w = -0.5 * 0.9 * 1.5;
        assert!((r.combined_weight - tsmom_w).abs() < 1e-10);
        // indicator_used is true because the indicator signal is valid,
        // but blend_indicator = 0.0 so it contributes nothing
        assert!(r.indicator_used);
    }

    #[test]
    fn fallback_on_flat_indicator() {
        let config = gold_50_50_config();
        let mut tsmom = HashMap::new();
        tsmom.insert("GLD".into(), make_signal("GLD", SignalDirection::Long, 0.8, 1.0, "tsmom", 2.0));
        let mut indicator = HashMap::new();
        indicator.insert("GLD".into(), make_indicator_signal("GLD", SignalDirection::Flat, 0.0, 0.0));

        let results = combine_signals(&tsmom, &indicator, &config);
        let r = &results[0];
        assert!(!r.indicator_used);
        assert!((r.combined_weight - 1.6).abs() < 1e-10); // TSMOM-only
    }

    #[test]
    fn fallback_on_llm_success_zero() {
        let config = gold_50_50_config();
        let mut tsmom = HashMap::new();
        tsmom.insert("GLD".into(), make_signal("GLD", SignalDirection::Long, 0.8, 1.0, "tsmom", 2.0));
        let mut indicator = HashMap::new();
        let mut ind_sig = make_indicator_signal("GLD", SignalDirection::Long, 0.6, 0.7);
        ind_sig.metadata.insert("llm_success".into(), 0.0);
        indicator.insert("GLD".into(), ind_sig);

        let results = combine_signals(&tsmom, &indicator, &config);
        assert!(!results[0].indicator_used);
    }

    #[test]
    fn fallback_on_missing_indicator() {
        let config = gold_50_50_config();
        let mut tsmom = HashMap::new();
        tsmom.insert("GLD".into(), make_signal("GLD", SignalDirection::Long, 0.8, 1.0, "tsmom", 2.0));
        let indicator = HashMap::new(); // empty

        let results = combine_signals(&tsmom, &indicator, &config);
        assert!(!results[0].indicator_used);
        assert!((results[0].combined_weight - 1.6).abs() < 1e-10);
    }

    #[test]
    fn category_routing_all_six() {
        assert_eq!(blend_category("GLD"), BlendCategory::Gold);
        assert_eq!(blend_category("GC=F"), BlendCategory::Gold);
        assert_eq!(blend_category("SPY"), BlendCategory::Equity);
        assert_eq!(blend_category("GBPUSD=X"), BlendCategory::Forex);
        assert_eq!(blend_category("USDCHF=X"), BlendCategory::Forex);
        assert_eq!(blend_category("USDJPY=X"), BlendCategory::Forex);
    }

    #[test]
    fn opposing_signals() {
        let config = gold_50_50_config();
        let mut tsmom = HashMap::new();
        tsmom.insert("GLD".into(), make_signal("GLD", SignalDirection::Long, 0.8, 1.0, "tsmom", 2.0));
        let mut indicator = HashMap::new();
        indicator.insert("GLD".into(), make_indicator_signal("GLD", SignalDirection::Short, -0.8, 1.0));

        let results = combine_signals(&tsmom, &indicator, &config);
        let r = &results[0];
        // tsmom_w = 0.8 * 1.0 * 2.0 = 1.6
        // indicator_w = -0.8 * 1.0 * 2.0 = -1.6
        // combined = 0.5 * 1.6 + 0.5 * (-1.6) = 0.0
        assert!((r.combined_weight).abs() < 1e-10);
        assert!(r.indicator_used);
    }

    #[test]
    fn both_flat_gives_zero() {
        let config = gold_50_50_config();
        let mut tsmom = HashMap::new();
        tsmom.insert("GLD".into(), make_signal("GLD", SignalDirection::Flat, 0.0, 0.0, "tsmom", 1.0));
        let mut indicator = HashMap::new();
        indicator.insert("GLD".into(), make_indicator_signal("GLD", SignalDirection::Flat, 0.0, 0.0));

        let results = combine_signals(&tsmom, &indicator, &config);
        let r = &results[0];
        assert!((r.combined_weight).abs() < 1e-10);
        assert!(!r.indicator_used);
    }

    #[test]
    fn missing_category_defaults_to_tsmom() {
        // Config with only gold weights — equity/forex use default
        let mut weights = HashMap::new();
        weights.insert(
            BlendCategory::Gold,
            crate::config::BlendWeights {
                tsmom: 0.5,
                indicator: 0.5,
            },
        );
        let config = BlendConfig {
            enabled: true,
            weights,
        };

        let mut tsmom = HashMap::new();
        tsmom.insert("SPY".into(), make_signal("SPY", SignalDirection::Long, 0.8, 1.0, "tsmom", 1.5));
        let mut indicator = HashMap::new();
        indicator.insert("SPY".into(), make_indicator_signal("SPY", SignalDirection::Short, -0.5, 0.8));

        let results = combine_signals(&tsmom, &indicator, &config);
        let r = &results[0];
        // Missing equity → default tsmom=1.0, indicator=0.0
        let tsmom_w = 0.8 * 1.0 * 1.5;
        assert!((r.combined_weight - tsmom_w).abs() < 1e-10);
    }

    #[test]
    fn vol_scalar_fallback() {
        let config = gold_50_50_config();
        let mut tsmom = HashMap::new();
        // Signal without vol_scalar in metadata
        let mut sig = Signal::new(
            "GLD".into(),
            SignalDirection::Long,
            0.5,
            0.8,
            "tsmom".into(),
            SignalType::Quant,
        )
        .unwrap();
        sig.metadata.clear(); // no vol_scalar
        tsmom.insert("GLD".into(), sig);

        let mut indicator = HashMap::new();
        indicator.insert("GLD".into(), make_indicator_signal("GLD", SignalDirection::Long, 0.6, 0.7));

        let results = combine_signals(&tsmom, &indicator, &config);
        let r = &results[0];
        // vol_scalar defaults to 1.0
        // tsmom_w = 0.5 * 0.8 * 1.0 = 0.4
        // indicator_w = 0.6 * 0.7 * 1.0 = 0.42
        // combined = 0.5 * 0.4 + 0.5 * 0.42 = 0.41
        assert!((r.tsmom_weight - 0.4).abs() < 1e-10);
        assert!((r.indicator_weight - 0.42).abs() < 1e-10);
        assert!((r.combined_weight - 0.41).abs() < 1e-10);
    }

    #[test]
    fn build_combined_signal_basic() {
        let config = gold_50_50_config();
        let mut tsmom = HashMap::new();
        tsmom.insert("GLD".into(), make_signal("GLD", SignalDirection::Long, 0.8, 1.0, "tsmom", 2.0));
        let mut indicator = HashMap::new();
        indicator.insert("GLD".into(), make_indicator_signal("GLD", SignalDirection::Long, 0.6, 0.7));

        let results = combine_signals(&tsmom, &indicator, &config);
        let r = &results[0];
        let sig = build_combined_signal(r, &tsmom["GLD"], Some(&indicator["GLD"]));

        assert_eq!(sig.direction, SignalDirection::Long);
        assert_eq!(sig.agent_name, "combined");
        assert_eq!(sig.signal_type, SignalType::Combined);
        assert!(sig.metadata.contains_key("tsmom_weight"));
        assert!(sig.metadata.contains_key("blend_tsmom"));
    }
}

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::agents::SignalAgent;
use crate::core::bar::BarSeries;
use crate::core::signal::{Signal, SignalDirection, SignalType};
use crate::db::Db;

use super::parser::parse_llm_response;
use super::prompt_loader::sha256_short;
use super::ta::TaSnapshot;

/// Per-instrument coverage stats during a replay run.
#[derive(Debug, Clone, Default)]
pub struct InstrumentCoverage {
    pub hits: usize,
    pub misses: usize,
    pub missing_dates: Vec<String>,
}

/// Aggregate coverage report across all instruments.
#[derive(Debug, Clone)]
pub struct CoverageReport {
    pub instruments: HashMap<String, InstrumentCoverage>,
    pub total_hits: usize,
    pub total_misses: usize,
}

impl CoverageReport {
    pub fn hit_rate(&self) -> f64 {
        let total = self.total_hits + self.total_misses;
        if total == 0 {
            0.0
        } else {
            self.total_hits as f64 / total as f64
        }
    }
}

impl std::fmt::Display for CoverageReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let total = self.total_hits + self.total_misses;
        writeln!(
            f,
            "  Cache coverage: {}/{} ({:.1}%)",
            self.total_hits,
            total,
            self.hit_rate() * 100.0,
        )?;
        let mut syms: Vec<&String> = self.instruments.keys().collect();
        syms.sort();
        for sym in syms {
            let cov = &self.instruments[sym];
            let sym_total = cov.hits + cov.misses;
            let pct = if sym_total > 0 {
                cov.hits as f64 / sym_total as f64 * 100.0
            } else {
                0.0
            };
            write!(f, "    {sym:<14} {}/{sym_total} ({pct:.0}%)", cov.hits)?;
            if !cov.missing_dates.is_empty() {
                let show: Vec<&str> = cov.missing_dates.iter().take(5).map(|s| s.as_str()).collect();
                write!(f, "  missing: {}", show.join(", "))?;
                if cov.missing_dates.len() > 5 {
                    write!(f, " +{} more", cov.missing_dates.len() - 5)?;
                }
            }
            writeln!(f)?;
        }
        Ok(())
    }
}

/// Cached indicator agent that replays LLM responses from SQLite.
///
/// Reconstructs cache keys identically to `LlmIndicatorAgent` and looks up
/// responses. Cache misses degrade to Flat with `llm_success=0.0`.
pub struct CachedIndicatorAgent {
    db: Arc<Mutex<Db>>,
    model: String,
    prompt_hash: String,
    coverage: Mutex<HashMap<String, InstrumentCoverage>>,
}

impl CachedIndicatorAgent {
    pub fn new(db: Arc<Mutex<Db>>, model: String, prompt_hash: String) -> Self {
        Self {
            db,
            model,
            prompt_hash,
            coverage: Mutex::new(HashMap::new()),
        }
    }

    /// Return the aggregate coverage report for the replay run.
    pub fn coverage_report(&self) -> CoverageReport {
        let instruments = self.coverage.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let total_hits = instruments.values().map(|c| c.hits).sum();
        let total_misses = instruments.values().map(|c| c.misses).sum();
        CoverageReport {
            instruments,
            total_hits,
            total_misses,
        }
    }

    fn record_hit(&self, instrument: &str) {
        if let Ok(mut cov) = self.coverage.lock() {
            cov.entry(instrument.to_string())
                .or_default()
                .hits += 1;
        }
    }

    fn record_miss(&self, instrument: &str, eval_date: &str) {
        if let Ok(mut cov) = self.coverage.lock() {
            let entry = cov.entry(instrument.to_string()).or_default();
            entry.misses += 1;
            entry.missing_dates.push(eval_date.to_string());
        }
    }
}

impl SignalAgent for CachedIndicatorAgent {
    fn name(&self) -> &str {
        "indicator_llm"
    }

    fn signal_type(&self) -> SignalType {
        SignalType::Llm
    }

    fn generate_signal(&self, bars: &BarSeries, instrument: &str) -> Signal {
        let snapshot = TaSnapshot::compute(bars);
        let user_prompt = format!(
            "Instrument: {}\n\n{}",
            instrument,
            snapshot.format_for_prompt()
        );

        let eval_date = bars
            .bars()
            .last()
            .map(|b| b.date.to_string())
            .unwrap_or_else(|| "unknown".into());
        let ta_hash = sha256_short(&user_prompt);
        let cache_key = format!(
            "{}|{}|{}|{}|{}",
            self.model, self.prompt_hash, instrument, eval_date, ta_hash
        );

        // Look up cached response
        let cached = {
            let db = self.db.lock().unwrap_or_else(|e| e.into_inner());
            db.get_llm_cache(&cache_key).ok().flatten()
        };

        match cached {
            Some(entry) if entry.llm_ok && entry.parse_ok => {
                match parse_llm_response(&entry.response_text) {
                    Ok(parsed) => {
                        self.record_hit(instrument);
                        let mut metadata = HashMap::new();
                        metadata.insert("llm_success".into(), 1.0);
                        if let Some(rsi) = snapshot.rsi_14 {
                            metadata.insert("rsi".into(), rsi);
                        }

                        let mut sig = Signal::new(
                            instrument.into(),
                            parsed.direction,
                            parsed.strength,
                            parsed.confidence,
                            "indicator_llm".into(),
                            SignalType::Llm,
                        )
                        .unwrap_or_else(|_| flat_signal(instrument, &snapshot));
                        sig.horizon_days = parsed.horizon_days;
                        sig.metadata = metadata;
                        sig
                    }
                    Err(_) => {
                        // Cached entry exists but re-parse failed — treat as miss
                        self.record_miss(instrument, &eval_date);
                        flat_signal(instrument, &snapshot)
                    }
                }
            }
            _ => {
                // Cache miss or non-ok entry
                self.record_miss(instrument, &eval_date);
                flat_signal(instrument, &snapshot)
            }
        }
    }
}

fn flat_signal(instrument: &str, snapshot: &TaSnapshot) -> Signal {
    let mut metadata = HashMap::new();
    metadata.insert("llm_success".into(), 0.0);
    if let Some(rsi) = snapshot.rsi_14 {
        metadata.insert("rsi".into(), rsi);
    }

    let mut sig = Signal::new(
        instrument.into(),
        SignalDirection::Flat,
        0.0,
        0.0,
        "indicator_llm".into(),
        SignalType::Llm,
    )
    .expect("flat signal values are always valid");
    sig.metadata = metadata;
    sig
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::bar::Bar;
    use crate::db::LlmCacheEntry;
    use chrono::NaiveDate;

    fn make_bars(prices: &[f64]) -> BarSeries {
        let base_date = NaiveDate::from_ymd_opt(2023, 1, 2).unwrap();
        let bars: Vec<Bar> = prices
            .iter()
            .enumerate()
            .map(|(i, &price)| Bar {
                date: base_date + chrono::Days::new(i as u64),
                open: price,
                high: price * 1.02,
                low: price * 0.98,
                close: price,
                volume: 10000.0,
            })
            .collect();
        BarSeries::new(bars).unwrap()
    }

    fn test_db() -> Arc<Mutex<Db>> {
        Arc::new(Mutex::new(Db::open_memory().unwrap()))
    }

    fn insert_cache_entry(db: &Arc<Mutex<Db>>, cache_key: &str, instrument: &str, response: &str) {
        let entry = LlmCacheEntry {
            cache_key: cache_key.into(),
            llm_model: "test-model".into(),
            prompt_hash: "testhash".into(),
            instrument: instrument.into(),
            eval_date: "2023-03-02".into(),
            ta_hash: "ta123".into(),
            response_text: response.into(),
            llm_ok: true,
            parse_ok: true,
            latency_ms: Some(100),
            created_at: "2023-03-02T12:00:00Z".into(),
        };
        db.lock().unwrap().insert_llm_cache(&entry).unwrap();
    }

    #[test]
    fn cache_hit_returns_signal() {
        let db = test_db();
        let prices: Vec<f64> = (0..60).map(|i| 100.0 + i as f64 * 0.5).collect();
        let bars = make_bars(&prices);

        // Compute the cache key the agent will use
        let snapshot = TaSnapshot::compute(&bars);
        let user_prompt = format!("Instrument: SPY\n\n{}", snapshot.format_for_prompt());
        let eval_date = bars.bars().last().unwrap().date.to_string();
        let ta_hash = sha256_short(&user_prompt);
        let cache_key = format!("test-model|testhash|SPY|{eval_date}|{ta_hash}");

        let response = r#"{"direction":"long","confidence":0.8,"strength":0.6}"#;
        insert_cache_entry(&db, &cache_key, "SPY", response);

        let agent = CachedIndicatorAgent::new(db, "test-model".into(), "testhash".into());
        let sig = agent.generate_signal(&bars, "SPY");

        assert_eq!(sig.direction, SignalDirection::Long);
        assert!((sig.confidence - 0.8).abs() < 1e-10);
        assert!((sig.strength - 0.6).abs() < 1e-10);
        assert_eq!(sig.metadata.get("llm_success"), Some(&1.0));

        let report = agent.coverage_report();
        assert_eq!(report.total_hits, 1);
        assert_eq!(report.total_misses, 0);
    }

    #[test]
    fn cache_miss_returns_flat() {
        let db = test_db();
        let prices: Vec<f64> = (0..60).map(|i| 100.0 + i as f64 * 0.5).collect();
        let bars = make_bars(&prices);

        let agent = CachedIndicatorAgent::new(db, "test-model".into(), "testhash".into());
        let sig = agent.generate_signal(&bars, "SPY");

        assert_eq!(sig.direction, SignalDirection::Flat);
        assert_eq!(sig.metadata.get("llm_success"), Some(&0.0));

        let report = agent.coverage_report();
        assert_eq!(report.total_hits, 0);
        assert_eq!(report.total_misses, 1);
        assert!(!report.instruments["SPY"].missing_dates.is_empty());
    }

    #[test]
    fn key_format_matches_llm_agent() {
        // Verify the cache key format matches what LlmIndicatorAgent produces
        let prices: Vec<f64> = (0..60).map(|i| 100.0 + i as f64 * 0.5).collect();
        let bars = make_bars(&prices);

        let snapshot = TaSnapshot::compute(&bars);
        let user_prompt = format!("Instrument: GLD\n\n{}", snapshot.format_for_prompt());
        let eval_date = bars.bars().last().unwrap().date.to_string();
        let ta_hash = sha256_short(&user_prompt);

        let expected = format!("mymodel|myhash|GLD|{eval_date}|{ta_hash}");

        // CachedIndicatorAgent should build the same key
        let db = test_db();
        // Insert with the expected key
        insert_cache_entry(&db, &expected, "GLD", r#"{"direction":"short","confidence":0.7,"strength":-0.5}"#);

        let agent = CachedIndicatorAgent::new(db, "mymodel".into(), "myhash".into());
        let sig = agent.generate_signal(&bars, "GLD");

        assert_eq!(sig.direction, SignalDirection::Short);
        assert!((sig.strength - (-0.5)).abs() < 1e-10);
    }

    #[test]
    fn coverage_tracking_multiple_instruments() {
        let db = test_db();
        let prices: Vec<f64> = (0..60).map(|i| 100.0 + i as f64 * 0.5).collect();
        let bars = make_bars(&prices);

        // Insert cache for SPY but not GLD
        let snapshot = TaSnapshot::compute(&bars);
        let user_prompt = format!("Instrument: SPY\n\n{}", snapshot.format_for_prompt());
        let eval_date = bars.bars().last().unwrap().date.to_string();
        let ta_hash = sha256_short(&user_prompt);
        let spy_key = format!("m|h|SPY|{eval_date}|{ta_hash}");
        insert_cache_entry(&db, &spy_key, "SPY", r#"{"direction":"long","confidence":0.8,"strength":0.6}"#);

        let agent = CachedIndicatorAgent::new(db, "m".into(), "h".into());
        agent.generate_signal(&bars, "SPY");
        agent.generate_signal(&bars, "GLD"); // will miss

        let report = agent.coverage_report();
        assert_eq!(report.total_hits, 1);
        assert_eq!(report.total_misses, 1);
        assert_eq!(report.instruments["SPY"].hits, 1);
        assert_eq!(report.instruments["GLD"].misses, 1);
    }

    #[test]
    fn parse_failure_degrades_to_flat() {
        let db = test_db();
        let prices: Vec<f64> = (0..60).map(|i| 100.0 + i as f64 * 0.5).collect();
        let bars = make_bars(&prices);

        let snapshot = TaSnapshot::compute(&bars);
        let user_prompt = format!("Instrument: SPY\n\n{}", snapshot.format_for_prompt());
        let eval_date = bars.bars().last().unwrap().date.to_string();
        let ta_hash = sha256_short(&user_prompt);
        let cache_key = format!("test-model|testhash|SPY|{eval_date}|{ta_hash}");

        // Insert garbled response
        insert_cache_entry(&db, &cache_key, "SPY", "not valid json at all");

        let agent = CachedIndicatorAgent::new(db, "test-model".into(), "testhash".into());
        let sig = agent.generate_signal(&bars, "SPY");

        assert_eq!(sig.direction, SignalDirection::Flat);
        assert_eq!(sig.metadata.get("llm_success"), Some(&0.0));

        let report = agent.coverage_report();
        assert_eq!(report.total_misses, 1);
    }

    #[test]
    fn non_ok_cache_entry_treated_as_miss() {
        let db = test_db();
        let prices: Vec<f64> = (0..60).map(|i| 100.0 + i as f64 * 0.5).collect();
        let bars = make_bars(&prices);

        let snapshot = TaSnapshot::compute(&bars);
        let user_prompt = format!("Instrument: SPY\n\n{}", snapshot.format_for_prompt());
        let eval_date = bars.bars().last().unwrap().date.to_string();
        let ta_hash = sha256_short(&user_prompt);
        let cache_key = format!("test-model|testhash|SPY|{eval_date}|{ta_hash}");

        // Insert entry where llm_ok=false
        let entry = LlmCacheEntry {
            cache_key,
            llm_model: "test-model".into(),
            prompt_hash: "testhash".into(),
            instrument: "SPY".into(),
            eval_date: eval_date.clone(),
            ta_hash,
            response_text: "timeout".into(),
            llm_ok: false,
            parse_ok: false,
            latency_ms: None,
            created_at: "2023-03-02T12:00:00Z".into(),
        };
        db.lock().unwrap().insert_llm_cache(&entry).unwrap();

        let agent = CachedIndicatorAgent::new(db, "test-model".into(), "testhash".into());
        let sig = agent.generate_signal(&bars, "SPY");

        assert_eq!(sig.direction, SignalDirection::Flat);
    }

    #[test]
    fn coverage_report_hit_rate() {
        let report = CoverageReport {
            instruments: HashMap::new(),
            total_hits: 3,
            total_misses: 1,
        };
        assert!((report.hit_rate() - 0.75).abs() < 1e-10);
    }

    #[test]
    fn agent_name_and_type() {
        let db = test_db();
        let agent = CachedIndicatorAgent::new(db, "m".into(), "h".into());
        assert_eq!(agent.name(), "indicator_llm");
        assert_eq!(agent.signal_type(), SignalType::Llm);
    }
}

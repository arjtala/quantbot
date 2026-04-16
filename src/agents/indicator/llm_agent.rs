use std::collections::HashMap;
use std::sync::Mutex;

use tokio::runtime::Handle;

use crate::agents::SignalAgent;
use crate::core::bar::BarSeries;
use crate::core::signal::{Signal, SignalDirection, SignalType};
use crate::db::LlmCacheEntry;

use super::llm_client::{LlmClient, LlmConfig};
use super::parser::parse_llm_response;
use super::prompt_loader::{self, sha256_short, LoadedPrompt};
use super::ta::TaSnapshot;

/// LLM-based indicator agent that computes TA features, sends them to an
/// OpenAI-compatible endpoint, and parses the structured response into a Signal.
///
/// On any LLM error, degrades gracefully to a Flat signal with `llm_success=0.0`.
/// After each call, collects a cache entry for deterministic replay.
pub struct LlmIndicatorAgent {
    client: tokio::sync::Mutex<LlmClient>,
    prompt: LoadedPrompt,
    model: String,
    cache_entries: Mutex<Vec<LlmCacheEntry>>,
}

impl LlmIndicatorAgent {
    pub fn new(config: LlmConfig) -> Result<Self, super::llm_client::LlmError> {
        let prompt = prompt_loader::load(config.prompt_path.as_deref());
        let model = config.model.clone();
        let client = LlmClient::new(config)?;
        Ok(Self {
            client: tokio::sync::Mutex::new(client),
            prompt,
            model,
            cache_entries: Mutex::new(Vec::new()),
        })
    }

    #[cfg(test)]
    pub fn new_test(config: LlmConfig) -> Self {
        let prompt = prompt_loader::load(config.prompt_path.as_deref());
        let model = config.model.clone();
        let client = LlmClient::new_test(config);
        Self {
            client: tokio::sync::Mutex::new(client),
            prompt,
            model,
            cache_entries: Mutex::new(Vec::new()),
        }
    }

    /// Return the loaded prompt metadata for audit/recording.
    pub fn loaded_prompt(&self) -> &LoadedPrompt {
        &self.prompt
    }

    /// Async implementation: compute TA → call LLM → parse → Signal.
    /// Also stores a cache entry for each call.
    async fn generate_signal_async(&self, bars: &BarSeries, instrument: &str) -> Signal {
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
            self.model, self.prompt.hash, instrument, eval_date, ta_hash
        );

        let start = std::time::Instant::now();
        let result = {
            let mut client = self.client.lock().await;
            client.chat(&self.prompt.text, &user_prompt).await
        };
        let latency_ms = start.elapsed().as_millis() as u64;

        let (signal, response_text, llm_ok, parse_ok) = match result {
            Ok(raw) => match parse_llm_response(&raw) {
                Ok(parsed) => {
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
                    (sig, raw, true, true)
                }
                Err(e) => {
                    eprintln!("  WARN: LLM parse error for {instrument}: {e}");
                    (flat_signal(instrument, &snapshot), raw, true, false)
                }
            },
            Err(e) => {
                eprintln!("  WARN: LLM request failed for {instrument}: {e}");
                (
                    flat_signal(instrument, &snapshot),
                    e.to_string(),
                    false,
                    false,
                )
            }
        };

        // Store cache entry (never block on failure)
        let entry = LlmCacheEntry {
            cache_key,
            llm_model: self.model.clone(),
            prompt_hash: self.prompt.hash.clone(),
            instrument: instrument.to_string(),
            eval_date,
            ta_hash,
            response_text,
            llm_ok,
            parse_ok,
            latency_ms: Some(latency_ms),
            created_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Micros, true),
        };
        if let Ok(mut entries) = self.cache_entries.lock() {
            entries.push(entry);
        }

        signal
    }
}

impl SignalAgent for LlmIndicatorAgent {
    fn name(&self) -> &str {
        "indicator_llm"
    }

    fn signal_type(&self) -> SignalType {
        SignalType::Llm
    }

    fn generate_signal(&self, bars: &BarSeries, instrument: &str) -> Signal {
        // Bridge async → sync using block_in_place (safe under multi-thread runtime)
        tokio::task::block_in_place(|| {
            Handle::current().block_on(self.generate_signal_async(bars, instrument))
        })
    }

    fn take_cache_entries(&self) -> Vec<LlmCacheEntry> {
        let mut entries = self.cache_entries.lock().unwrap_or_else(|e| e.into_inner());
        std::mem::take(&mut *entries)
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

    fn test_config(base_url: &str) -> LlmConfig {
        LlmConfig {
            base_url: base_url.to_string(),
            model: "test-model".to_string(),
            temperature: 0.3,
            max_tokens: 512,
            timeout_secs: 5,
            max_retries: 0,
            prompt_path: None,
        }
    }

    #[tokio::test]
    async fn llm_agent_success() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/v1/chat/completions")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"choices":[{"message":{"role":"assistant","content":"{\"direction\":\"long\",\"confidence\":0.8,\"strength\":0.6,\"horizon_days\":14,\"reasoning\":\"bullish\"}"}}]}"#)
            .create_async()
            .await;

        let agent = LlmIndicatorAgent::new_test(test_config(&server.url()));
        let prices: Vec<f64> = (0..60).map(|i| 100.0 + i as f64 * 0.5).collect();
        let bars = make_bars(&prices);

        let sig = agent.generate_signal_async(&bars, "SPY").await;
        assert_eq!(sig.direction, SignalDirection::Long);
        assert!((sig.confidence - 0.8).abs() < 1e-10);
        assert_eq!(sig.metadata.get("llm_success"), Some(&1.0));
        mock.assert_async().await;

        // Verify cache entry was collected
        let entries = agent.take_cache_entries();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].instrument, "SPY");
        assert!(entries[0].llm_ok);
        assert!(entries[0].parse_ok);
        assert!(entries[0].latency_ms.is_some());
        assert_eq!(entries[0].llm_model, "test-model");
    }

    #[tokio::test]
    async fn llm_agent_error_degrades_to_flat() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/v1/chat/completions")
            .with_status(500)
            .with_body("internal error")
            .create_async()
            .await;

        let agent = LlmIndicatorAgent::new_test(test_config(&server.url()));
        let prices: Vec<f64> = (0..60).map(|i| 100.0 + i as f64 * 0.5).collect();
        let bars = make_bars(&prices);

        let sig = agent.generate_signal_async(&bars, "SPY").await;
        assert_eq!(sig.direction, SignalDirection::Flat);
        assert_eq!(sig.metadata.get("llm_success"), Some(&0.0));
        mock.assert_async().await;

        // Verify error cache entry
        let entries = agent.take_cache_entries();
        assert_eq!(entries.len(), 1);
        assert!(!entries[0].llm_ok);
        assert!(!entries[0].parse_ok);
    }

    #[tokio::test]
    async fn llm_agent_bad_json_degrades_to_flat() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/v1/chat/completions")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"choices":[{"message":{"role":"assistant","content":"I think the market will go up"}}]}"#)
            .create_async()
            .await;

        let agent = LlmIndicatorAgent::new_test(test_config(&server.url()));
        let prices: Vec<f64> = (0..60).map(|i| 100.0 + i as f64 * 0.5).collect();
        let bars = make_bars(&prices);

        let sig = agent.generate_signal_async(&bars, "SPY").await;
        assert_eq!(sig.direction, SignalDirection::Flat);
        assert_eq!(sig.metadata.get("llm_success"), Some(&0.0));
        mock.assert_async().await;

        // Verify parse-failure cache entry
        let entries = agent.take_cache_entries();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].llm_ok);
        assert!(!entries[0].parse_ok);
    }

    #[tokio::test]
    async fn cache_key_deterministic() {
        let mut server = mockito::Server::new_async().await;
        server
            .mock("POST", "/v1/chat/completions")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"choices":[{"message":{"role":"assistant","content":"{\"direction\":\"flat\",\"confidence\":0.5,\"strength\":0.0}"}}]}"#)
            .create_async()
            .await;

        let agent = LlmIndicatorAgent::new_test(test_config(&server.url()));
        let prices: Vec<f64> = (0..60).map(|i| 100.0 + i as f64 * 0.5).collect();
        let bars = make_bars(&prices);

        agent.generate_signal_async(&bars, "GLD").await;
        let entries1 = agent.take_cache_entries();

        // Regenerate with same inputs
        // Need a new mock since the first one was consumed
        server
            .mock("POST", "/v1/chat/completions")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"choices":[{"message":{"role":"assistant","content":"{\"direction\":\"flat\",\"confidence\":0.5,\"strength\":0.0}"}}]}"#)
            .create_async()
            .await;

        agent.generate_signal_async(&bars, "GLD").await;
        let entries2 = agent.take_cache_entries();

        assert_eq!(entries1[0].cache_key, entries2[0].cache_key);
        assert_eq!(entries1[0].ta_hash, entries2[0].ta_hash);
    }

    #[tokio::test]
    async fn cache_key_changes_with_instrument() {
        let mut server = mockito::Server::new_async().await;
        let response = r#"{"choices":[{"message":{"role":"assistant","content":"{\"direction\":\"flat\",\"confidence\":0.5,\"strength\":0.0}"}}]}"#;
        server
            .mock("POST", "/v1/chat/completions")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(response)
            .expect(2)
            .create_async()
            .await;

        let agent = LlmIndicatorAgent::new_test(test_config(&server.url()));
        let prices: Vec<f64> = (0..60).map(|i| 100.0 + i as f64 * 0.5).collect();
        let bars = make_bars(&prices);

        agent.generate_signal_async(&bars, "SPY").await;
        agent.generate_signal_async(&bars, "GLD").await;
        let entries = agent.take_cache_entries();

        assert_eq!(entries.len(), 2);
        assert_ne!(entries[0].cache_key, entries[1].cache_key);
        assert_ne!(entries[0].ta_hash, entries[1].ta_hash);
    }

    #[tokio::test]
    async fn take_cache_entries_drains() {
        let mut server = mockito::Server::new_async().await;
        server
            .mock("POST", "/v1/chat/completions")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"choices":[{"message":{"role":"assistant","content":"{\"direction\":\"flat\",\"confidence\":0.5,\"strength\":0.0}"}}]}"#)
            .create_async()
            .await;

        let agent = LlmIndicatorAgent::new_test(test_config(&server.url()));
        let prices: Vec<f64> = (0..60).map(|i| 100.0 + i as f64 * 0.5).collect();
        let bars = make_bars(&prices);

        agent.generate_signal_async(&bars, "SPY").await;
        let entries = agent.take_cache_entries();
        assert_eq!(entries.len(), 1);

        // Second take should be empty
        let entries2 = agent.take_cache_entries();
        assert!(entries2.is_empty());
    }
}

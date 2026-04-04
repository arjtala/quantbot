use std::collections::HashMap;

use tokio::runtime::Handle;
use tokio::sync::Mutex;

use crate::agents::SignalAgent;
use crate::core::bar::BarSeries;
use crate::core::signal::{Signal, SignalDirection, SignalType};

use super::llm_client::{LlmClient, LlmConfig};
use super::parser::parse_llm_response;
use super::prompt_loader::{self, LoadedPrompt};
use super::ta::TaSnapshot;

/// LLM-based indicator agent that computes TA features, sends them to an
/// OpenAI-compatible endpoint, and parses the structured response into a Signal.
///
/// On any LLM error, degrades gracefully to a Flat signal with `llm_success=0.0`.
pub struct LlmIndicatorAgent {
    client: Mutex<LlmClient>,
    prompt: LoadedPrompt,
}

impl LlmIndicatorAgent {
    pub fn new(config: LlmConfig) -> Result<Self, super::llm_client::LlmError> {
        let prompt = prompt_loader::load(config.prompt_path.as_deref());
        let client = LlmClient::new(config)?;
        Ok(Self {
            client: Mutex::new(client),
            prompt,
        })
    }

    #[cfg(test)]
    pub fn new_test(config: LlmConfig) -> Self {
        let prompt = prompt_loader::load(config.prompt_path.as_deref());
        let client = LlmClient::new_test(config);
        Self {
            client: Mutex::new(client),
            prompt,
        }
    }

    /// Return the loaded prompt metadata for audit/recording.
    pub fn loaded_prompt(&self) -> &LoadedPrompt {
        &self.prompt
    }

    /// Async implementation: compute TA → call LLM → parse → Signal.
    async fn generate_signal_async(&self, bars: &BarSeries, instrument: &str) -> Signal {
        let snapshot = TaSnapshot::compute(bars);
        let user_prompt = format!(
            "Instrument: {}\n\n{}",
            instrument,
            snapshot.format_for_prompt()
        );

        let result = {
            let mut client = self.client.lock().await;
            client.chat(&self.prompt.text, &user_prompt).await
        };

        match result {
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
                    sig
                }
                Err(e) => {
                    eprintln!("  WARN: LLM parse error for {instrument}: {e}");
                    flat_signal(instrument, &snapshot)
                }
            },
            Err(e) => {
                eprintln!("  WARN: LLM request failed for {instrument}: {e}");
                flat_signal(instrument, &snapshot)
            }
        }
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
    }
}

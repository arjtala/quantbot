use std::sync::OnceLock;

use regex::Regex;
use serde::Deserialize;
use thiserror::Error;

use crate::core::signal::SignalDirection;

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("no JSON found in LLM response")]
    NoJson,
    #[error("invalid direction: {0}")]
    InvalidDirection(String),
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Parsed LLM response with validated fields.
#[derive(Debug, Clone)]
pub struct LlmResponse {
    pub direction: SignalDirection,
    pub confidence: f64,
    pub strength: f64,
    pub horizon_days: u32,
    pub reasoning: String,
}

/// Raw serde target — fields are optional for flexible parsing.
#[derive(Debug, Deserialize)]
struct RawLlmResponse {
    direction: String,
    #[serde(default)]
    confidence: Option<f64>,
    #[serde(default)]
    strength: Option<f64>,
    #[serde(default = "default_horizon")]
    horizon_days: Option<u32>,
    #[serde(default)]
    reasoning: Option<String>,
}

fn default_horizon() -> Option<u32> {
    Some(21)
}

static THINK_RE: OnceLock<Regex> = OnceLock::new();
static JSON_BLOCK_RE: OnceLock<Regex> = OnceLock::new();

fn think_regex() -> &'static Regex {
    THINK_RE.get_or_init(|| Regex::new(r"(?s)<think>.*?</think>").unwrap())
}

fn json_block_regex() -> &'static Regex {
    JSON_BLOCK_RE.get_or_init(|| Regex::new(r"(?s)\{[^{}]*(?:\{[^{}]*\}[^{}]*)*\}").unwrap())
}

/// Parse an LLM response string into a structured `LlmResponse`.
///
/// Pipeline:
/// 1. Strip `<think>...</think>` blocks (reasoning models)
/// 2. Strip markdown code fences
/// 3. Try parsing the full string as JSON
/// 4. Fallback: regex extract first JSON object
pub fn parse_llm_response(raw: &str) -> Result<LlmResponse, ParseError> {
    // Step 1: Strip <think> blocks
    let stripped = think_regex().replace_all(raw, "");
    let stripped = stripped.trim();

    // Step 2: Strip markdown fences
    let stripped = strip_markdown_fences(stripped);
    let stripped = stripped.trim();

    // Step 3: Try direct JSON parse
    if let Ok(parsed) = serde_json::from_str::<RawLlmResponse>(stripped) {
        return validate_raw(parsed);
    }

    // Step 4: Regex fallback — find first JSON-like block
    if let Some(m) = json_block_regex().find(stripped) {
        let json_str = m.as_str();
        let parsed: RawLlmResponse = serde_json::from_str(json_str)?;
        return validate_raw(parsed);
    }

    Err(ParseError::NoJson)
}

fn strip_markdown_fences(s: &str) -> &str {
    let s = s.trim();
    if let Some(rest) = s.strip_prefix("```json") {
        if let Some(inner) = rest.strip_suffix("```") {
            return inner.trim();
        }
    }
    if let Some(rest) = s.strip_prefix("```") {
        if let Some(inner) = rest.strip_suffix("```") {
            return inner.trim();
        }
    }
    s
}

fn validate_raw(raw: RawLlmResponse) -> Result<LlmResponse, ParseError> {
    let direction = parse_direction(&raw.direction)?;
    let confidence = raw.confidence.unwrap_or(0.5).clamp(0.0, 1.0);
    let strength = raw.strength.unwrap_or(0.0).clamp(-1.0, 1.0);
    let horizon_days = raw.horizon_days.unwrap_or(21);
    let reasoning = raw.reasoning.unwrap_or_default();

    Ok(LlmResponse {
        direction,
        confidence,
        strength,
        horizon_days,
        reasoning,
    })
}

fn parse_direction(s: &str) -> Result<SignalDirection, ParseError> {
    match s.to_lowercase().trim() {
        "long" | "buy" => Ok(SignalDirection::Long),
        "short" | "sell" => Ok(SignalDirection::Short),
        "flat" | "neutral" | "hold" => Ok(SignalDirection::Flat),
        _ => Err(ParseError::InvalidDirection(s.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_clean_json() {
        let raw = r#"{"direction":"long","confidence":0.8,"strength":0.6,"horizon_days":14,"reasoning":"bullish trend"}"#;
        let resp = parse_llm_response(raw).unwrap();
        assert_eq!(resp.direction, SignalDirection::Long);
        assert!((resp.confidence - 0.8).abs() < 1e-10);
        assert!((resp.strength - 0.6).abs() < 1e-10);
        assert_eq!(resp.horizon_days, 14);
        assert_eq!(resp.reasoning, "bullish trend");
    }

    #[test]
    fn parse_with_think_block() {
        let raw = r#"<think>Let me analyze...</think>{"direction":"short","confidence":0.7,"strength":-0.5}"#;
        let resp = parse_llm_response(raw).unwrap();
        assert_eq!(resp.direction, SignalDirection::Short);
        assert!((resp.strength - (-0.5)).abs() < 1e-10);
    }

    #[test]
    fn parse_with_markdown_fences() {
        let raw = "```json\n{\"direction\":\"flat\",\"confidence\":0.3,\"strength\":0.0}\n```";
        let resp = parse_llm_response(raw).unwrap();
        assert_eq!(resp.direction, SignalDirection::Flat);
    }

    #[test]
    fn parse_embedded_json_in_text() {
        let raw = "Here is my analysis:\n{\"direction\":\"long\",\"confidence\":0.9,\"strength\":0.8}\nThat's my view.";
        let resp = parse_llm_response(raw).unwrap();
        assert_eq!(resp.direction, SignalDirection::Long);
    }

    #[test]
    fn parse_clamps_confidence() {
        let raw = r#"{"direction":"long","confidence":1.5,"strength":0.5}"#;
        let resp = parse_llm_response(raw).unwrap();
        assert!((resp.confidence - 1.0).abs() < 1e-10);
    }

    #[test]
    fn parse_clamps_strength() {
        let raw = r#"{"direction":"short","confidence":0.5,"strength":-2.0}"#;
        let resp = parse_llm_response(raw).unwrap();
        assert!((resp.strength - (-1.0)).abs() < 1e-10);
    }

    #[test]
    fn parse_defaults_missing_fields() {
        let raw = r#"{"direction":"neutral"}"#;
        let resp = parse_llm_response(raw).unwrap();
        assert_eq!(resp.direction, SignalDirection::Flat);
        assert!((resp.confidence - 0.5).abs() < 1e-10);
        assert!((resp.strength - 0.0).abs() < 1e-10);
        assert_eq!(resp.horizon_days, 21);
    }

    #[test]
    fn parse_direction_variants() {
        for (input, expected) in [
            ("Long", SignalDirection::Long),
            ("SHORT", SignalDirection::Short),
            ("flat", SignalDirection::Flat),
            ("Neutral", SignalDirection::Flat),
            ("buy", SignalDirection::Long),
            ("sell", SignalDirection::Short),
            ("hold", SignalDirection::Flat),
        ] {
            let raw = format!(r#"{{"direction":"{}"}}"#, input);
            let resp = parse_llm_response(&raw).unwrap();
            assert_eq!(resp.direction, expected, "failed for input: {input}");
        }
    }

    #[test]
    fn parse_invalid_direction() {
        let raw = r#"{"direction":"sideways"}"#;
        let result = parse_llm_response(raw);
        assert!(matches!(result, Err(ParseError::InvalidDirection(_))));
    }

    #[test]
    fn parse_no_json() {
        let result = parse_llm_response("I think the market will go up.");
        assert!(matches!(result, Err(ParseError::NoJson)));
    }

    #[test]
    fn parse_think_block_with_json_inside() {
        // The think block contains JSON-like text, but the real JSON is after
        let raw = r#"<think>{"ignore": true}</think>{"direction":"long","confidence":0.6,"strength":0.4}"#;
        let resp = parse_llm_response(raw).unwrap();
        assert_eq!(resp.direction, SignalDirection::Long);
    }
}

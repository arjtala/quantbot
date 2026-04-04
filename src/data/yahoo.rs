use std::time::Instant;

use anyhow::{bail, Context, Result};
use chrono::NaiveDate;
use serde::Deserialize;

use crate::core::bar::Bar;

/// Yahoo Finance v8 chart API client with rate limiting and null filtering.
pub struct YahooClient {
    http: reqwest::Client,
    base_url: String,
    last_request: Option<Instant>,
    rate_limit_ms: u64,
}

/// Errors specific to Yahoo API responses.
#[derive(Debug, thiserror::Error)]
pub enum YahooError {
    #[error("HTTP {status}: {body}")]
    Http { status: u16, body: String },
    #[error("no data returned for {symbol}")]
    NoData { symbol: String },
    #[error("request failed: {0}")]
    Request(#[from] reqwest::Error),
}

// ── Yahoo v8 JSON response types ──────────────────────────────────────

#[derive(Debug, Deserialize)]
struct ChartResponse {
    chart: ChartWrapper,
}

#[derive(Debug, Deserialize)]
struct ChartWrapper {
    result: Option<Vec<ChartResult>>,
    error: Option<ChartError>,
}

#[derive(Debug, Deserialize)]
struct ChartError {
    description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChartResult {
    timestamp: Option<Vec<i64>>,
    indicators: Indicators,
}

#[derive(Debug, Deserialize)]
struct Indicators {
    quote: Vec<QuoteData>,
}

#[derive(Debug, Deserialize)]
struct QuoteData {
    open: Vec<Option<f64>>,
    high: Vec<Option<f64>>,
    low: Vec<Option<f64>>,
    close: Vec<Option<f64>>,
    volume: Vec<Option<f64>>,
}

impl Default for YahooClient {
    fn default() -> Self {
        Self::new()
    }
}

impl YahooClient {
    /// Create a client pointing at the real Yahoo Finance API.
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::new(),
            base_url: "https://query2.finance.yahoo.com".to_string(),
            last_request: None,
            rate_limit_ms: 500,
        }
    }

    /// Create a client with a custom base URL (for testing with mockito).
    /// Uses `.no_proxy()` to prevent HTTP proxy from intercepting localhost requests.
    #[cfg(test)]
    pub fn new_with_base_url(base_url: &str) -> Self {
        Self {
            http: reqwest::Client::builder().no_proxy().build().unwrap(),
            base_url: base_url.to_string(),
            last_request: None,
            rate_limit_ms: 0, // no rate limiting in tests
        }
    }

    /// Fetch daily bars for `symbol` in the date range `[from, to]`.
    ///
    /// Rows with any null OHLC value are filtered out. Volume defaults to 0
    /// if missing (common for FX instruments).
    pub async fn fetch_daily_bars(
        &mut self,
        symbol: &str,
        from: NaiveDate,
        to: NaiveDate,
    ) -> Result<Vec<Bar>> {
        self.rate_limit().await;

        let period1 = from
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc()
            .timestamp();
        let period2 = to
            .and_hms_opt(23, 59, 59)
            .unwrap()
            .and_utc()
            .timestamp();

        let url = format!(
            "{}/v8/finance/chart/{}?period1={}&period2={}&interval=1d",
            self.base_url, symbol, period1, period2
        );

        let resp = self
            .http
            .get(&url)
            .header("User-Agent", "quantbot/0.1")
            .send()
            .await
            .context("Yahoo API request failed")?;

        self.last_request = Some(Instant::now());

        let status = resp.status().as_u16();
        if status == 429 {
            bail!("Yahoo rate limited (HTTP 429) — try again later");
        }
        if status >= 400 {
            let body = resp.text().await.unwrap_or_default();
            return Err(YahooError::Http { status, body }.into());
        }

        let chart_resp: ChartResponse = resp
            .json()
            .await
            .context("failed to parse Yahoo JSON response")?;

        if let Some(err) = chart_resp.chart.error {
            bail!(
                "Yahoo API error: {}",
                err.description.unwrap_or_else(|| "unknown".into())
            );
        }

        let results = chart_resp
            .chart
            .result
            .unwrap_or_default();

        let result = results
            .into_iter()
            .next()
            .ok_or_else(|| YahooError::NoData {
                symbol: symbol.to_string(),
            })?;

        let timestamps = result.timestamp.unwrap_or_default();
        let quote = result
            .indicators
            .quote
            .into_iter()
            .next()
            .ok_or_else(|| YahooError::NoData {
                symbol: symbol.to_string(),
            })?;

        let mut bars = Vec::new();
        for (i, &ts) in timestamps.iter().enumerate() {
            let open = match quote.open.get(i).copied().flatten() {
                Some(v) => v,
                None => continue,
            };
            let high = match quote.high.get(i).copied().flatten() {
                Some(v) => v,
                None => continue,
            };
            let low = match quote.low.get(i).copied().flatten() {
                Some(v) => v,
                None => continue,
            };
            let close = match quote.close.get(i).copied().flatten() {
                Some(v) => v,
                None => continue,
            };
            // Volume may be missing for FX — default to 0
            let volume = quote
                .volume
                .get(i)
                .copied()
                .flatten()
                .unwrap_or(0.0);

            let date = chrono::DateTime::from_timestamp(ts, 0)
                .map(|dt| dt.date_naive())
                .with_context(|| format!("invalid timestamp {} at index {i}", ts))?;

            bars.push(Bar {
                date,
                open,
                high,
                low,
                close,
                volume,
            });
        }

        // Ensure chronological order
        bars.sort_by_key(|b| b.date);

        // Deduplicate by date (Yahoo sometimes returns duplicates)
        bars.dedup_by_key(|b| b.date);

        Ok(bars)
    }

    /// Enforce minimum interval between requests.
    async fn rate_limit(&self) {
        if self.rate_limit_ms == 0 {
            return;
        }
        if let Some(last) = self.last_request {
            let elapsed = last.elapsed().as_millis() as u64;
            if elapsed < self.rate_limit_ms {
                let wait = self.rate_limit_ms - elapsed;
                tokio::time::sleep(std::time::Duration::from_millis(wait)).await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito;

    fn sample_response() -> String {
        r#"{
            "chart": {
                "result": [{
                    "timestamp": [1704153600, 1704240000, 1704326400],
                    "indicators": {
                        "quote": [{
                            "open":   [100.0, 101.0, 102.0],
                            "high":   [101.5, 102.5, 103.0],
                            "low":    [99.0,  100.5, 101.0],
                            "close":  [101.0, 102.0, 102.5],
                            "volume": [50000, 60000, 55000]
                        }]
                    }
                }],
                "error": null
            }
        }"#
        .to_string()
    }

    fn response_with_nulls() -> String {
        r#"{
            "chart": {
                "result": [{
                    "timestamp": [1704153600, 1704240000, 1704326400],
                    "indicators": {
                        "quote": [{
                            "open":   [100.0, null,  102.0],
                            "high":   [101.5, 102.5, 103.0],
                            "low":    [99.0,  100.5, 101.0],
                            "close":  [101.0, 102.0, 102.5],
                            "volume": [50000, null,  55000]
                        }]
                    }
                }],
                "error": null
            }
        }"#
        .to_string()
    }

    #[tokio::test]
    async fn fetch_parses_bars() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", mockito::Matcher::Regex(r"/v8/finance/chart/SPY.*".to_string()))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(sample_response())
            .create_async()
            .await;

        let mut client = YahooClient::new_with_base_url(&server.url());
        let from = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
        let to = NaiveDate::from_ymd_opt(2024, 1, 5).unwrap();

        let bars = client.fetch_daily_bars("SPY", from, to).await.unwrap();
        assert_eq!(bars.len(), 3);
        assert!((bars[0].open - 100.0).abs() < 1e-10);
        assert!((bars[0].close - 101.0).abs() < 1e-10);
        assert!((bars[2].high - 103.0).abs() < 1e-10);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn nulls_filtered_out() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", mockito::Matcher::Regex(r"/v8/finance/chart/TEST.*".to_string()))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(response_with_nulls())
            .create_async()
            .await;

        let mut client = YahooClient::new_with_base_url(&server.url());
        let from = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
        let to = NaiveDate::from_ymd_opt(2024, 1, 5).unwrap();

        let bars = client.fetch_daily_bars("TEST", from, to).await.unwrap();
        // Row with null open is filtered out
        assert_eq!(bars.len(), 2);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn http_error_reported() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", mockito::Matcher::Regex(r"/v8/finance/chart/BAD.*".to_string()))
            .with_status(404)
            .with_body("not found")
            .create_async()
            .await;

        let mut client = YahooClient::new_with_base_url(&server.url());
        let from = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
        let to = NaiveDate::from_ymd_opt(2024, 1, 5).unwrap();

        let err = client.fetch_daily_bars("BAD", from, to).await;
        assert!(err.is_err());

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn empty_result_is_error() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", mockito::Matcher::Regex(r"/v8/finance/chart/EMPTY.*".to_string()))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"chart":{"result":[],"error":null}}"#)
            .create_async()
            .await;

        let mut client = YahooClient::new_with_base_url(&server.url());
        let from = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
        let to = NaiveDate::from_ymd_opt(2024, 1, 5).unwrap();

        let err = client.fetch_daily_bars("EMPTY", from, to).await;
        assert!(err.is_err());

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn volume_defaults_to_zero_for_fx() {
        let body = r#"{
            "chart": {
                "result": [{
                    "timestamp": [1704153600],
                    "indicators": {
                        "quote": [{
                            "open":   [1.27],
                            "high":   [1.28],
                            "low":    [1.26],
                            "close":  [1.275],
                            "volume": [null]
                        }]
                    }
                }],
                "error": null
            }
        }"#;

        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", mockito::Matcher::Regex(r"/v8/finance/chart/GBPUSD.*".to_string()))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(body)
            .create_async()
            .await;

        let mut client = YahooClient::new_with_base_url(&server.url());
        let from = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
        let to = NaiveDate::from_ymd_opt(2024, 1, 3).unwrap();

        let bars = client.fetch_daily_bars("GBPUSD=X", from, to).await.unwrap();
        assert_eq!(bars.len(), 1);
        assert!((bars[0].volume - 0.0).abs() < 1e-10);

        mock.assert_async().await;
    }
}

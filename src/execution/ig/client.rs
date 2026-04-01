use std::time::{Duration, Instant};

use reqwest::header::{HeaderMap, HeaderValue};

use crate::config::IgConfig;
use crate::execution::ig::errors::IgError;
use crate::execution::ig::types::*;

const MAX_RETRIES: u32 = 2;
const RATE_LIMIT_INTERVAL: Duration = Duration::from_millis(1050);

pub struct IgClient {
    http: reqwest::Client,
    base_url: String,
    api_key: String,
    username: String,
    password: String,
    cst: Option<String>,
    security_token: Option<String>,
    last_request: Option<Instant>,
}

impl IgClient {
    pub fn new(config: &IgConfig) -> Result<Self, IgError> {
        let api_key =
            std::env::var("IG_API_KEY").map_err(|_| IgError::AuthFailed("IG_API_KEY not set".into()))?;
        let username = std::env::var("IG_USERNAME")
            .map_err(|_| IgError::AuthFailed("IG_USERNAME not set".into()))?;
        let password = std::env::var("IG_PASSWORD")
            .map_err(|_| IgError::AuthFailed("IG_PASSWORD not set".into()))?;

        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(IgError::Network)?;

        Ok(Self {
            http,
            base_url: config.base_url().to_string(),
            api_key,
            username,
            password,
            cst: None,
            security_token: None,
            last_request: None,
        })
    }

    /// For testing: construct with explicit credentials and base URL.
    #[cfg(test)]
    pub fn new_with_credentials(
        base_url: String,
        api_key: String,
        username: String,
        password: String,
    ) -> Self {
        Self {
            http: reqwest::Client::new(),
            base_url,
            api_key,
            username,
            password,
            cst: None,
            security_token: None,
            last_request: None,
        }
    }

    pub fn is_authenticated(&self) -> bool {
        self.cst.is_some() && self.security_token.is_some()
    }

    /// POST /session (version 2) — stores CST + X-SECURITY-TOKEN from response headers.
    pub async fn authenticate(&mut self) -> Result<SessionResponse, IgError> {
        self.enforce_rate_limit().await;

        let url = format!("{}/session", self.base_url);
        let body = SessionRequest {
            identifier: self.username.clone(),
            password: self.password.clone(),
        };

        let resp = self
            .http
            .post(&url)
            .header("X-IG-API-KEY", &self.api_key)
            .header("VERSION", "2")
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(IgError::Network)?;

        self.record_request();

        let status = resp.status().as_u16();
        if status == 401 || status == 403 {
            return Err(IgError::AuthFailed(format!("HTTP {status}")));
        }
        if status >= 400 {
            let text = resp.text().await.unwrap_or_default();
            return Err(IgError::ApiError {
                status,
                message: text,
            });
        }

        // Extract auth tokens from headers
        let cst = resp
            .headers()
            .get("CST")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string())
            .ok_or_else(|| IgError::AuthFailed("missing CST header".into()))?;

        let security_token = resp
            .headers()
            .get("X-SECURITY-TOKEN")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string())
            .ok_or_else(|| IgError::AuthFailed("missing X-SECURITY-TOKEN header".into()))?;

        let session: SessionResponse = resp.json().await.map_err(IgError::Network)?;

        self.cst = Some(cst);
        self.security_token = Some(security_token);

        Ok(session)
    }

    /// Authenticate if not already, re-auth on 401.
    pub async fn ensure_authenticated(&mut self) -> Result<(), IgError> {
        if !self.is_authenticated() {
            self.authenticate().await?;
        }
        Ok(())
    }

    /// GET /positions (version 2)
    pub async fn get_positions(&mut self) -> Result<PositionsResponse, IgError> {
        let url = format!("{}/positions", self.base_url);
        let resp = self.authed_request(reqwest::Method::GET, &url, "2", None::<&()>).await?;
        let positions: PositionsResponse = resp.json().await.map_err(IgError::Network)?;
        Ok(positions)
    }

    /// POST /positions/otc (version 2) — place a new position.
    pub async fn create_position(
        &mut self,
        req: &CreatePositionRequest,
    ) -> Result<DealReferenceResponse, IgError> {
        let url = format!("{}/positions/otc", self.base_url);
        let resp = self.authed_request(reqwest::Method::POST, &url, "2", Some(req)).await?;
        let deal_ref: DealReferenceResponse = resp.json().await.map_err(IgError::Network)?;
        Ok(deal_ref)
    }

    /// POST /positions/otc with _method=DELETE header (IG's DELETE-with-body convention).
    pub async fn close_position(
        &mut self,
        req: &ClosePositionRequest,
    ) -> Result<DealReferenceResponse, IgError> {
        let url = format!("{}/positions/otc", self.base_url);

        self.ensure_authenticated().await?;
        self.enforce_rate_limit().await;

        let resp = self
            .http
            .post(&url)
            .headers(self.auth_headers("1"))
            .header("_method", "DELETE")
            .json(req)
            .send()
            .await
            .map_err(IgError::Network)?;

        self.record_request();
        let resp = self.handle_response(resp).await?;
        let deal_ref: DealReferenceResponse = resp.json().await.map_err(IgError::Network)?;
        Ok(deal_ref)
    }

    /// GET /confirms/{deal_ref} (version 1)
    pub async fn get_deal_confirmation(
        &mut self,
        deal_ref: &str,
    ) -> Result<DealConfirmation, IgError> {
        let url = format!("{}/confirms/{}", self.base_url, deal_ref);
        let resp = self.authed_request(reqwest::Method::GET, &url, "1", None::<&()>).await?;
        let confirmation: DealConfirmation = resp.json().await.map_err(IgError::Network)?;
        Ok(confirmation)
    }

    // ─── Private helpers ─────────────────────────────────────────

    fn auth_headers(&self, version: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert("X-IG-API-KEY", HeaderValue::from_str(&self.api_key).unwrap());
        headers.insert("VERSION", HeaderValue::from_str(version).unwrap());
        headers.insert(
            "Content-Type",
            HeaderValue::from_static("application/json"),
        );
        if let Some(cst) = &self.cst {
            headers.insert("CST", HeaderValue::from_str(cst).unwrap());
        }
        if let Some(token) = &self.security_token {
            headers.insert(
                "X-SECURITY-TOKEN",
                HeaderValue::from_str(token).unwrap(),
            );
        }
        headers
    }

    /// Send an authenticated request with retry on 5xx and re-auth on 401.
    async fn authed_request<T: serde::Serialize>(
        &mut self,
        method: reqwest::Method,
        url: &str,
        version: &str,
        body: Option<&T>,
    ) -> Result<reqwest::Response, IgError> {
        self.ensure_authenticated().await?;

        for attempt in 0..=MAX_RETRIES {
            self.enforce_rate_limit().await;

            let mut req_builder = self.http.request(method.clone(), url).headers(self.auth_headers(version));
            if let Some(b) = body {
                req_builder = req_builder.json(b);
            }

            let resp = req_builder.send().await.map_err(IgError::Network)?;
            self.record_request();

            let status = resp.status().as_u16();

            // Re-auth once on 401
            if status == 401 && attempt == 0 {
                eprintln!("  IG: 401 received, re-authenticating...");
                self.cst = None;
                self.security_token = None;
                self.authenticate().await?;
                continue;
            }

            // Retry on 5xx (not on last attempt)
            if status >= 500 && attempt < MAX_RETRIES {
                let backoff = Duration::from_secs(1 << attempt);
                eprintln!("  IG: {status} on attempt {}, retrying in {:?}...", attempt + 1, backoff);
                tokio::time::sleep(backoff).await;
                continue;
            }

            return self.handle_response(resp).await;
        }

        unreachable!("retry loop should return")
    }

    async fn handle_response(&self, resp: reqwest::Response) -> Result<reqwest::Response, IgError> {
        let status = resp.status().as_u16();

        if status == 401 || status == 403 {
            return Err(IgError::AuthFailed(format!("HTTP {status}")));
        }
        if status == 429 {
            return Err(IgError::RateLimited);
        }
        if status >= 400 {
            let text = resp.text().await.unwrap_or_default();
            return Err(IgError::ApiError {
                status,
                message: text,
            });
        }

        Ok(resp)
    }

    async fn enforce_rate_limit(&self) {
        if let Some(last) = self.last_request {
            let elapsed = last.elapsed();
            if elapsed < RATE_LIMIT_INTERVAL {
                tokio::time::sleep(RATE_LIMIT_INTERVAL - elapsed).await;
            }
        }
    }

    fn record_request(&mut self) {
        self.last_request = Some(Instant::now());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn auth_success() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/session")
            .with_status(200)
            .with_header("CST", "test-cst-token")
            .with_header("X-SECURITY-TOKEN", "test-security-token")
            .with_body(r#"{"accountId":"Z69YJL"}"#)
            .create_async()
            .await;

        let mut client = IgClient::new_with_credentials(
            server.url(),
            "test-key".into(),
            "test-user".into(),
            "test-pass".into(),
        );

        let session = client.authenticate().await.unwrap();
        assert_eq!(session.account_id, "Z69YJL");
        assert!(client.is_authenticated());
        assert_eq!(client.cst.as_deref(), Some("test-cst-token"));
        assert_eq!(client.security_token.as_deref(), Some("test-security-token"));

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn auth_failure_401() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/session")
            .with_status(401)
            .with_body(r#"{"errorCode":"INVALID_CREDENTIALS"}"#)
            .create_async()
            .await;

        let mut client = IgClient::new_with_credentials(
            server.url(),
            "test-key".into(),
            "test-user".into(),
            "bad-pass".into(),
        );

        let result = client.authenticate().await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), IgError::AuthFailed(_)));
        assert!(!client.is_authenticated());

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn get_positions_success() {
        let mut server = mockito::Server::new_async().await;

        // Auth mock
        server
            .mock("POST", "/session")
            .with_status(200)
            .with_header("CST", "cst")
            .with_header("X-SECURITY-TOKEN", "sec")
            .with_body(r#"{"accountId":"Z69YJL"}"#)
            .create_async()
            .await;

        // Positions mock
        let positions_body = r#"{
            "positions": [{
                "position": {
                    "dealId": "DEAL1",
                    "direction": "BUY",
                    "size": 0.5,
                    "level": 1.2650,
                    "currency": "GBP"
                },
                "market": {
                    "epic": "CS.D.GBPUSD.TODAY.IP",
                    "instrumentName": "GBP/USD"
                }
            }]
        }"#;
        let mock = server
            .mock("GET", "/positions")
            .match_header("CST", "cst")
            .match_header("X-SECURITY-TOKEN", "sec")
            .with_status(200)
            .with_body(positions_body)
            .create_async()
            .await;

        let mut client = IgClient::new_with_credentials(
            server.url(),
            "key".into(),
            "user".into(),
            "pass".into(),
        );

        let resp = client.get_positions().await.unwrap();
        assert_eq!(resp.positions.len(), 1);
        assert_eq!(resp.positions[0].position.deal_id, "DEAL1");
        assert_eq!(resp.positions[0].market.epic, "CS.D.GBPUSD.TODAY.IP");
        assert_eq!(resp.positions[0].position.size, 0.5);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn create_position_and_confirm() {
        let mut server = mockito::Server::new_async().await;

        // Auth
        server
            .mock("POST", "/session")
            .with_status(200)
            .with_header("CST", "cst")
            .with_header("X-SECURITY-TOKEN", "sec")
            .with_body(r#"{"accountId":"Z69YJL"}"#)
            .create_async()
            .await;

        // Create position
        let create_mock = server
            .mock("POST", "/positions/otc")
            .with_status(200)
            .with_body(r#"{"dealReference":"REF123"}"#)
            .create_async()
            .await;

        // Confirm
        let confirm_mock = server
            .mock("GET", "/confirms/REF123")
            .with_status(200)
            .with_body(r#"{
                "dealId": "DEAL456",
                "dealReference": "REF123",
                "dealStatus": "ACCEPTED",
                "reason": "SUCCESS",
                "level": 1.2650,
                "size": 0.5
            }"#)
            .create_async()
            .await;

        let mut client = IgClient::new_with_credentials(
            server.url(),
            "key".into(),
            "user".into(),
            "pass".into(),
        );

        let create_req = CreatePositionRequest {
            epic: "CS.D.GBPUSD.TODAY.IP".into(),
            direction: "BUY".into(),
            size: 0.5,
            order_type: "MARKET".into(),
            currency_code: "GBP".into(),
            expiry: "DFB".into(),
            force_open: false,
            guaranteed_stop: false,
        };

        let deal_ref = client.create_position(&create_req).await.unwrap();
        assert_eq!(deal_ref.deal_reference, "REF123");

        let confirm = client.get_deal_confirmation("REF123").await.unwrap();
        assert_eq!(confirm.deal_id, "DEAL456");
        assert_eq!(confirm.deal_status, "ACCEPTED");
        assert_eq!(confirm.level, Some(1.265));

        create_mock.assert_async().await;
        confirm_mock.assert_async().await;
    }

    #[tokio::test]
    async fn retry_on_5xx() {
        let mut server = mockito::Server::new_async().await;

        // Auth
        server
            .mock("POST", "/session")
            .with_status(200)
            .with_header("CST", "cst")
            .with_header("X-SECURITY-TOKEN", "sec")
            .with_body(r#"{"accountId":"Z69YJL"}"#)
            .create_async()
            .await;

        // First call: 500, second call: 200
        let fail_mock = server
            .mock("GET", "/positions")
            .with_status(500)
            .with_body("internal error")
            .expect(1)
            .create_async()
            .await;

        let success_mock = server
            .mock("GET", "/positions")
            .with_status(200)
            .with_body(r#"{"positions":[]}"#)
            .expect(1)
            .create_async()
            .await;

        let mut client = IgClient::new_with_credentials(
            server.url(),
            "key".into(),
            "user".into(),
            "pass".into(),
        );

        let result = client.get_positions().await.unwrap();
        assert!(result.positions.is_empty());

        fail_mock.assert_async().await;
        success_mock.assert_async().await;
    }

    #[tokio::test]
    async fn no_retry_on_4xx() {
        let mut server = mockito::Server::new_async().await;

        // Auth
        server
            .mock("POST", "/session")
            .with_status(200)
            .with_header("CST", "cst")
            .with_header("X-SECURITY-TOKEN", "sec")
            .with_body(r#"{"accountId":"Z69YJL"}"#)
            .create_async()
            .await;

        // 400 — should NOT retry
        let mock = server
            .mock("POST", "/positions/otc")
            .with_status(400)
            .with_body(r#"{"errorCode":"INVALID_INPUT"}"#)
            .expect(1)
            .create_async()
            .await;

        let mut client = IgClient::new_with_credentials(
            server.url(),
            "key".into(),
            "user".into(),
            "pass".into(),
        );

        let req = CreatePositionRequest {
            epic: "BAD.EPIC".into(),
            direction: "BUY".into(),
            size: 0.5,
            order_type: "MARKET".into(),
            currency_code: "GBP".into(),
            expiry: "DFB".into(),
            force_open: false,
            guaranteed_stop: false,
        };

        let result = client.create_position(&req).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), IgError::ApiError { status: 400, .. }));

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn reauth_on_401_during_request() {
        let mut server = mockito::Server::new_async().await;

        // First auth
        server
            .mock("POST", "/session")
            .with_status(200)
            .with_header("CST", "cst1")
            .with_header("X-SECURITY-TOKEN", "sec1")
            .with_body(r#"{"accountId":"Z69YJL"}"#)
            .expect(2)  // called twice: initial + re-auth
            .create_async()
            .await;

        // First positions call: 401, triggers re-auth
        let auth_fail = server
            .mock("GET", "/positions")
            .with_status(401)
            .with_body("session expired")
            .expect(1)
            .create_async()
            .await;

        // Second positions call after re-auth: success
        let success = server
            .mock("GET", "/positions")
            .with_status(200)
            .with_body(r#"{"positions":[]}"#)
            .expect(1)
            .create_async()
            .await;

        let mut client = IgClient::new_with_credentials(
            server.url(),
            "key".into(),
            "user".into(),
            "pass".into(),
        );

        let result = client.get_positions().await.unwrap();
        assert!(result.positions.is_empty());

        auth_fail.assert_async().await;
        success.assert_async().await;
    }

    #[tokio::test]
    async fn close_position_uses_delete_method_header() {
        let mut server = mockito::Server::new_async().await;

        // Auth
        server
            .mock("POST", "/session")
            .with_status(200)
            .with_header("CST", "cst")
            .with_header("X-SECURITY-TOKEN", "sec")
            .with_body(r#"{"accountId":"Z69YJL"}"#)
            .create_async()
            .await;

        // Close position — POST with _method=DELETE header
        let mock = server
            .mock("POST", "/positions/otc")
            .match_header("_method", "DELETE")
            .with_status(200)
            .with_body(r#"{"dealReference":"CLOSE_REF"}"#)
            .create_async()
            .await;

        let mut client = IgClient::new_with_credentials(
            server.url(),
            "key".into(),
            "user".into(),
            "pass".into(),
        );

        let req = ClosePositionRequest {
            deal_id: "DEAL1".into(),
            direction: "SELL".into(),
            size: 0.5,
            order_type: "MARKET".into(),
        };

        let result = client.close_position(&req).await.unwrap();
        assert_eq!(result.deal_reference, "CLOSE_REF");

        mock.assert_async().await;
    }
}

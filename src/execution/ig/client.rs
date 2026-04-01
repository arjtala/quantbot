// IG REST client — skeleton for PR 1, completed in PR 2.

use crate::config::IgConfig;
use crate::execution::ig::errors::IgError;
use crate::execution::ig::types::*;

pub struct IgClient {
    http: reqwest::Client,
    base_url: String,
    api_key: String,
    _username: String,
    _password: String,
    cst: Option<String>,
    security_token: Option<String>,
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
            .build()
            .map_err(IgError::Network)?;

        Ok(Self {
            http,
            base_url: config.base_url().to_string(),
            api_key,
            _username: username,
            _password: password,
            cst: None,
            security_token: None,
        })
    }

    pub async fn authenticate(&mut self) -> Result<SessionResponse, IgError> {
        let _ = (&self.http, &self.base_url, &self.api_key);
        todo!("PR 2: POST /session")
    }

    pub async fn get_positions(&self) -> Result<PositionsResponse, IgError> {
        let _ = (&self.cst, &self.security_token);
        todo!("PR 2: GET /positions")
    }

    pub async fn create_position(
        &self,
        _req: &CreatePositionRequest,
    ) -> Result<DealReferenceResponse, IgError> {
        todo!("PR 2: POST /positions/otc")
    }

    pub async fn close_position(
        &self,
        _req: &ClosePositionRequest,
    ) -> Result<DealReferenceResponse, IgError> {
        todo!("PR 2: DELETE /positions/otc")
    }

    pub async fn get_deal_confirmation(
        &self,
        deal_ref: &str,
    ) -> Result<DealConfirmation, IgError> {
        todo!("PR 2: GET /confirms/{deal_ref}")
    }
}

// IG REST API DTOs — to be filled in PR 2.

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct SessionRequest {
    pub identifier: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionResponse {
    pub current_account_id: String,
}

#[derive(Debug, Deserialize)]
pub struct PositionsResponse {
    pub positions: Vec<IgPosition>,
}

#[derive(Debug, Deserialize)]
pub struct IgPosition {
    pub position: PositionData,
    pub market: MarketData,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PositionData {
    pub deal_id: String,
    pub direction: String,
    pub size: f64,
    pub level: f64,
    pub currency: String,
}

#[derive(Debug, Deserialize)]
pub struct MarketData {
    pub epic: String,
    #[serde(rename = "instrumentName")]
    pub instrument_name: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatePositionRequest {
    pub epic: String,
    pub direction: String,
    pub size: f64,
    pub order_type: String,
    pub currency_code: String,
    pub expiry: String,
    pub force_open: bool,
    pub guaranteed_stop: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClosePositionRequest {
    pub deal_id: String,
    pub direction: String,
    pub size: f64,
    pub order_type: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DealReferenceResponse {
    pub deal_reference: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DealConfirmation {
    pub deal_id: String,
    pub deal_reference: String,
    pub deal_status: String,
    pub reason: String,
    pub level: Option<f64>,
    pub size: Option<f64>,
}

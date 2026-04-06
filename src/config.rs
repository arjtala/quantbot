use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[cfg(feature = "track-b")]
use crate::agents::indicator::llm_client::LlmConfig;
use crate::agents::risk::RiskConfig;
use crate::execution::router::{ContractSpec, ExecutionRouter};

// ─── Blend Config (track-b) ─────────────────────────────────────

#[cfg(feature = "track-b")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BlendCategory {
    Gold,
    Equity,
    Forex,
}

#[cfg(feature = "track-b")]
impl std::fmt::Display for BlendCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BlendCategory::Gold => write!(f, "gold"),
            BlendCategory::Equity => write!(f, "equity"),
            BlendCategory::Forex => write!(f, "forex"),
        }
    }
}

#[cfg(feature = "track-b")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlendWeights {
    pub tsmom: f64,
    pub indicator: f64,
}

#[cfg(feature = "track-b")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatingConfig {
    #[serde(default = "default_min_confidence")]
    pub min_confidence: f64,
    #[serde(default)]
    pub min_abs_strength: f64,
}

#[cfg(feature = "track-b")]
fn default_min_confidence() -> f64 {
    0.0
}

#[cfg(feature = "track-b")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlendConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub weights: HashMap<BlendCategory, BlendWeights>,
    #[serde(default)]
    pub gating: Option<GatingConfig>,
}

#[cfg(feature = "track-b")]
impl BlendConfig {
    /// Safe lookup: returns category weights if found, else TSMOM-only default.
    pub fn weights_for(&self, cat: BlendCategory) -> &BlendWeights {
        static TSMOM_ONLY: BlendWeights = BlendWeights {
            tsmom: 1.0,
            indicator: 0.0,
        };
        self.weights.get(&cat).unwrap_or(&TSMOM_ONLY)
    }
}

// ─── App Config ──────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub execution: ExecutionConfig,
    pub risk: Option<RiskConfig>,
    #[cfg(feature = "track-b")]
    pub llm: Option<LlmConfig>,
    #[cfg(feature = "track-b")]
    pub blending: Option<BlendConfig>,
}

impl AppConfig {
    pub fn load(path: &Path) -> Result<Self> {
        let contents = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read config file: {}", path.display()))?;
        let config: AppConfig =
            toml::from_str(&contents).with_context(|| "failed to parse TOML config")?;
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<()> {
        if self.execution.engine == EngineType::Ig {
            let ig = self
                .execution
                .ig
                .as_ref()
                .context("execution.ig section required when engine = \"ig\"")?;
            if ig.account_id.is_empty() {
                anyhow::bail!("execution.ig.account_id must not be empty");
            }
            if ig.instruments.is_empty() {
                anyhow::bail!("execution.ig.instruments must not be empty");
            }
        }
        #[cfg(feature = "track-b")]
        if let Some(blend) = &self.blending {
            if blend.enabled {
                for cat in [BlendCategory::Gold, BlendCategory::Equity, BlendCategory::Forex] {
                    if !blend.weights.contains_key(&cat) {
                        eprintln!("  WARN: blending enabled but no weights for {cat} — defaulting to TSMOM-only");
                    }
                }
                for (cat, w) in &blend.weights {
                    if w.tsmom + w.indicator <= 0.0 {
                        anyhow::bail!("blending.weights.{cat}: tsmom + indicator must be > 0");
                    }
                }
            }
        }
        Ok(())
    }
}

// ─── Execution Config ────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionConfig {
    pub engine: EngineType,
    pub ig: Option<IgConfig>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EngineType {
    Ig,
    Paper,
}

// ─── IG Config ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IgConfig {
    pub environment: IgEnvironment,
    pub account_id: String,
    pub instruments: HashMap<String, InstrumentConfig>,
}

impl IgConfig {
    pub fn base_url(&self) -> &str {
        match self.environment {
            IgEnvironment::Demo => "https://demo-api.ig.com/gateway/deal",
            IgEnvironment::Live => "https://api.ig.com/gateway/deal",
        }
    }

    /// Build an ExecutionRouter using config-driven point values for IG spread-bet sizing.
    ///
    /// This produces deal sizes in IG units (£/point for equities, £/pip for FX)
    /// rather than raw quantities. The backtest router uses different point values
    /// (e.g., GC=F = 100 for futures contracts) and should NOT be used for live sizing.
    pub fn to_execution_router(&self) -> ExecutionRouter {
        let mut specs = HashMap::new();

        // Use IG defaults as base, then override with config values
        let defaults = ContractSpec::ig_defaults();

        for (symbol, inst) in &self.instruments {
            let base = defaults
                .get(symbol)
                .cloned()
                .unwrap_or_else(|| ContractSpec::default_equity(symbol));
            specs.insert(
                symbol.clone(),
                ContractSpec {
                    symbol: symbol.clone(),
                    asset_class: base.asset_class,
                    point_value: inst.point_value(),
                    min_deal_size: inst.min_size,
                    lot_step: inst.size_step,
                    margin_pct: base.margin_pct,
                    spread_bps: base.spread_bps,
                },
            );
        }

        ExecutionRouter::new(specs)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum IgEnvironment {
    Demo,
    Live,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstrumentConfig {
    pub epic: String,
    pub min_size: f64,
    pub size_step: f64,
    pub currency_code: Option<String>,
    pub expiry: Option<String>,
    /// Point value for IG spread-bet sizing.
    ///
    /// Converts notional exposure to IG deal size:
    ///   `deal_size = (weight × NAV) / (price × ig_point_value)`
    ///
    /// - FX major pairs (pip = 0.0001): 10000.0
    /// - FX JPY pairs (pip = 0.01): 100.0
    /// - Equity/commodity per-point: 1.0
    pub ig_point_value: Option<f64>,
}

impl InstrumentConfig {
    pub fn currency(&self) -> &str {
        self.currency_code.as_deref().unwrap_or("GBP")
    }

    pub fn expiry(&self) -> &str {
        self.expiry.as_deref().unwrap_or("DFB")
    }

    /// Point value for IG spread-bet sizing. Defaults to 1.0.
    pub fn point_value(&self) -> f64 {
        self.ig_point_value.unwrap_or(1.0)
    }
}

// ─── Tests ───────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_toml() -> &'static str {
        r#"
[execution]
engine = "ig"

[execution.ig]
environment = "DEMO"
account_id = "Z69YJL"

[execution.ig.instruments."SPY"]
epic = "IX.D.SPTRD.DAILY.IP"
min_size = 0.1
size_step = 0.1

[execution.ig.instruments."GC=F"]
epic = "CC.D.GC.USS.IP"
min_size = 1.0
size_step = 1.0

[execution.ig.instruments."GLD"]
epic = "UC.D.GLDUS.DAILY.IP"
min_size = 1.0
size_step = 1.0

[execution.ig.instruments."GBPUSD=X"]
epic = "CS.D.GBPUSD.TODAY.IP"
min_size = 0.5
size_step = 0.1

[execution.ig.instruments."USDCHF=X"]
epic = "CS.D.USDCHF.TODAY.IP"
min_size = 0.5
size_step = 0.1

[execution.ig.instruments."USDJPY=X"]
epic = "CS.D.USDJPY.TODAY.IP"
min_size = 0.5
size_step = 0.1
"#
    }

    #[test]
    fn parse_valid_config() {
        let config: AppConfig = toml::from_str(sample_toml()).unwrap();
        assert_eq!(config.execution.engine, EngineType::Ig);

        let ig = config.execution.ig.unwrap();
        assert_eq!(ig.environment, IgEnvironment::Demo);
        assert_eq!(ig.account_id, "Z69YJL");
        assert_eq!(ig.instruments.len(), 6);

        let spy = &ig.instruments["SPY"];
        assert_eq!(spy.epic, "IX.D.SPTRD.DAILY.IP");
        assert_eq!(spy.min_size, 0.1);
        assert_eq!(spy.currency(), "GBP");
        assert_eq!(spy.expiry(), "DFB");
    }

    #[test]
    fn parse_paper_config() {
        let toml_str = r#"
[execution]
engine = "paper"
"#;
        let config: AppConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.execution.engine, EngineType::Paper);
        assert!(config.execution.ig.is_none());
    }

    #[test]
    fn ig_base_url() {
        let config: AppConfig = toml::from_str(sample_toml()).unwrap();
        let ig = config.execution.ig.unwrap();
        assert_eq!(ig.base_url(), "https://demo-api.ig.com/gateway/deal");
    }

    #[test]
    fn validate_ig_missing_section() {
        let toml_str = r#"
[execution]
engine = "ig"
"#;
        let config: AppConfig = toml::from_str(toml_str).unwrap();
        assert!(config.validate().is_err());
    }

    #[test]
    fn validate_ig_empty_account() {
        let toml_str = r#"
[execution]
engine = "ig"

[execution.ig]
environment = "DEMO"
account_id = ""

[execution.ig.instruments."SPY"]
epic = "IX.D.SPTRD.DAILY.IP"
min_size = 0.1
size_step = 0.1
"#;
        let config: AppConfig = toml::from_str(toml_str).unwrap();
        assert!(config.validate().is_err());
    }

    #[test]
    fn validate_ig_empty_instruments() {
        let toml_str = r#"
[execution]
engine = "ig"

[execution.ig]
environment = "DEMO"
account_id = "Z69YJL"

[execution.ig.instruments]
"#;
        let config: AppConfig = toml::from_str(toml_str).unwrap();
        assert!(config.validate().is_err());
    }

    #[test]
    fn custom_currency_and_expiry() {
        let toml_str = r#"
[execution]
engine = "paper"

[execution.ig]
environment = "LIVE"
account_id = "TEST"

[execution.ig.instruments."SPY"]
epic = "IX.D.SPTRD.DAILY.IP"
min_size = 0.1
size_step = 0.1
currency_code = "USD"
expiry = "MAR-26"
"#;
        let config: AppConfig = toml::from_str(toml_str).unwrap();
        let ig = config.execution.ig.unwrap();
        let spy = &ig.instruments["SPY"];
        assert_eq!(spy.currency(), "USD");
        assert_eq!(spy.expiry(), "MAR-26");
        assert_eq!(ig.base_url(), "https://api.ig.com/gateway/deal");
    }

    #[test]
    fn load_from_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, sample_toml()).unwrap();

        let config = AppConfig::load(&path).unwrap();
        assert_eq!(config.execution.engine, EngineType::Ig);
    }

    #[test]
    fn load_missing_file() {
        let result = AppConfig::load(Path::new("/nonexistent/config.toml"));
        assert!(result.is_err());
    }

    #[test]
    fn parse_risk_config() {
        let toml_str = r#"
[execution]
engine = "paper"

[risk]
max_gross_leverage = 3.0
max_position_pct = 0.30
max_drawdown_pct = 0.20
"#;
        let config: AppConfig = toml::from_str(toml_str).unwrap();
        let risk = config.risk.unwrap();
        assert_eq!(risk.max_gross_leverage, 3.0);
        assert_eq!(risk.max_position_pct, 0.30);
        assert_eq!(risk.max_drawdown_pct, 0.20);
    }

    #[test]
    fn risk_config_defaults() {
        let toml_str = r#"
[execution]
engine = "paper"

[risk]
"#;
        let config: AppConfig = toml::from_str(toml_str).unwrap();
        let risk = config.risk.unwrap();
        assert_eq!(risk.max_gross_leverage, 2.5);
        assert_eq!(risk.max_position_pct, 0.25);
        assert_eq!(risk.max_drawdown_pct, 0.15);
    }

    #[test]
    fn risk_config_omitted() {
        let config: AppConfig = toml::from_str(sample_toml()).unwrap();
        assert!(config.risk.is_none());
    }

    #[test]
    fn quoted_keys_with_special_chars() {
        let config: AppConfig = toml::from_str(sample_toml()).unwrap();
        let ig = config.execution.ig.unwrap();
        assert!(ig.instruments.contains_key("GC=F"));
        assert!(ig.instruments.contains_key("GBPUSD=X"));
    }

    #[test]
    #[cfg(feature = "track-b")]
    fn parse_llm_config() {
        let toml_str = r#"
[execution]
engine = "paper"

[llm]
base_url = "http://localhost:11434"
model = "llama3"
"#;
        let config: AppConfig = toml::from_str(toml_str).unwrap();
        let llm = config.llm.unwrap();
        assert_eq!(llm.base_url, "http://localhost:11434");
        assert_eq!(llm.model, "llama3");
        assert_eq!(llm.temperature, 0.3);
        assert_eq!(llm.max_tokens, 4096);
    }

    #[test]
    #[cfg(feature = "track-b")]
    fn parse_blend_config() {
        let toml_str = r#"
[execution]
engine = "paper"

[blending]
enabled = true

[blending.weights.gold]
tsmom = 0.50
indicator = 0.50

[blending.weights.equity]
tsmom = 1.00
indicator = 0.00

[blending.weights.forex]
tsmom = 0.10
indicator = 0.90
"#;
        let config: AppConfig = toml::from_str(toml_str).unwrap();
        let blend = config.blending.unwrap();
        assert!(blend.enabled);
        assert_eq!(blend.weights.len(), 3);

        let gold = &blend.weights[&BlendCategory::Gold];
        assert_eq!(gold.tsmom, 0.50);
        assert_eq!(gold.indicator, 0.50);

        let equity = &blend.weights[&BlendCategory::Equity];
        assert_eq!(equity.tsmom, 1.00);
        assert_eq!(equity.indicator, 0.00);

        let forex = &blend.weights[&BlendCategory::Forex];
        assert_eq!(forex.tsmom, 0.10);
        assert_eq!(forex.indicator, 0.90);
    }

    #[test]
    #[cfg(feature = "track-b")]
    fn blend_missing_category_fallback() {
        let toml_str = r#"
[execution]
engine = "paper"

[blending]
enabled = true

[blending.weights.gold]
tsmom = 0.50
indicator = 0.50
"#;
        let config: AppConfig = toml::from_str(toml_str).unwrap();
        let blend = config.blending.unwrap();
        // Missing equity/forex → weights_for returns TSMOM-only default
        let equity_w = blend.weights_for(BlendCategory::Equity);
        assert_eq!(equity_w.tsmom, 1.0);
        assert_eq!(equity_w.indicator, 0.0);
    }

    #[test]
    #[cfg(feature = "track-b")]
    fn blend_weights_for_default() {
        let blend = BlendConfig {
            enabled: true,
            weights: HashMap::new(),
            gating: None,
        };
        let w = blend.weights_for(BlendCategory::Gold);
        assert_eq!(w.tsmom, 1.0);
        assert_eq!(w.indicator, 0.0);
    }

    #[test]
    #[cfg(feature = "track-b")]
    fn parse_gating_config() {
        let toml_str = r#"
[execution]
engine = "paper"

[blending]
enabled = true

[blending.weights.gold]
tsmom = 0.50
indicator = 0.50

[blending.gating]
min_confidence = 0.70
min_abs_strength = 0.30
"#;
        let config: AppConfig = toml::from_str(toml_str).unwrap();
        let blend = config.blending.unwrap();
        assert!(blend.enabled);
        let gating = blend.gating.unwrap();
        assert_eq!(gating.min_confidence, 0.70);
        assert_eq!(gating.min_abs_strength, 0.30);
    }
}

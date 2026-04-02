use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::execution::router::{ContractSpec, ExecutionRouter};

// ─── App Config ──────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub execution: ExecutionConfig,
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
    fn quoted_keys_with_special_chars() {
        let config: AppConfig = toml::from_str(sample_toml()).unwrap();
        let ig = config.execution.ig.unwrap();
        assert!(ig.instruments.contains_key("GC=F"));
        assert!(ig.instruments.contains_key("GBPUSD=X"));
    }
}

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process;

use anyhow::{bail, Context, Result};
use chrono::NaiveDate;
use clap::{Parser, Subcommand};

#[cfg(feature = "track-b")]
use quantbot::agents::combiner;
#[cfg(feature = "track-b")]
use quantbot::agents::indicator::DummyIndicatorAgent;
#[cfg(feature = "track-b")]
use quantbot::agents::indicator::llm_agent::LlmIndicatorAgent;
#[cfg(feature = "track-b")]
use quantbot::agents::indicator::cached_agent::CachedIndicatorAgent;
#[cfg(feature = "track-b")]
use quantbot::agents::SignalAgent;
use quantbot::agents::risk::{RiskAgent, RiskDecision};
use quantbot::agents::tsmom::TSMOMAgent;
use quantbot::audit::{
    self, AuditLogger, RunId, RunSummary, TargetEntry,
};
use quantbot::backtest::engine::{BacktestConfig, BacktestEngine, TargetSnapshot};
use quantbot::backtest::metrics::BacktestResult;
use quantbot::config::{AppConfig, EngineType};
use quantbot::core::portfolio::OrderSide;
use quantbot::core::signal::SignalDirection;
use quantbot::core::universe::TRADEABLE_UNIVERSE;
use quantbot::data::freshness;
use quantbot::data::loader::CsvLoader;
use quantbot::data::updater::DataUpdater;
use quantbot::data::yahoo::YahooClient;
use quantbot::db::Db;
use quantbot::execution::circuit_breaker::CircuitBreaker;
use quantbot::execution::ig::engine::IgExecutionEngine;
use quantbot::execution::mtm;
use quantbot::execution::paper::PaperExecutionEngine;
use quantbot::execution::reconcile;
use quantbot::execution::router::SizedOrder;
use quantbot::execution::traits::{ExecutionEngine, OrderRequest};
use quantbot::recording::{Recorder, SignalRecord};

#[derive(Parser)]
#[command(name = "quantbot", version, about = "Quantitative trading system")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run TSMOM backtest on historical CSV data
    Backtest(BacktestArgs),
    /// Generate target positions and orders for today (paper-trade mode)
    PaperTrade(PaperTradeArgs),
    /// Live trade with IG execution
    Live(LiveArgs),
    /// Show current IG positions
    Positions(PositionsArgs),
    /// Query trading history from SQLite database
    History(HistoryArgs),
    /// Update CSV data from Yahoo Finance
    Data(DataArgs),
    /// Evaluate strategies offline
    #[cfg(feature = "track-b")]
    Eval(EvalArgs),
    /// Batch-fill LLM cache for eval replay
    #[cfg(feature = "track-b")]
    Cache(CacheArgs),
}

#[derive(Parser)]
struct PositionsArgs {
    /// Path to TOML configuration file
    #[arg(long)]
    config: PathBuf,

    /// Output as JSON
    #[arg(long)]
    json: bool,
}

#[derive(Parser)]
struct HistoryArgs {
    /// Path to SQLite database
    #[arg(long, default_value = "data/quantbot.db")]
    db: PathBuf,

    /// Show details for a specific run ID
    #[arg(long)]
    run: Option<String>,

    /// Filter by instrument
    #[arg(long)]
    instrument: Option<String>,

    /// Filter orders by status (e.g. Accepted, Rejected, Pending)
    #[arg(long)]
    status: Option<String>,

    /// Filter runs by date prefix (e.g. 2026-04-03)
    #[arg(long)]
    date: Option<String>,

    /// Number of recent runs to show (default: 10)
    #[arg(long, default_value_t = 10)]
    last: usize,

    /// Output as JSON
    #[arg(long)]
    json: bool,
}

#[derive(Parser)]
struct DataArgs {
    /// Comma-separated instrument symbols (default: all CSVs or tradeable universe)
    #[arg(long, value_delimiter = ',')]
    instruments: Option<Vec<String>>,

    /// Update only the 6 tradeable instruments
    #[arg(long)]
    tradeable_only: bool,

    /// Path to directory containing CSV data files
    #[arg(long, default_value = "data", env = "QUANTBOT_DATA")]
    data_dir: PathBuf,

    /// Output as JSON
    #[arg(long)]
    json: bool,
}

#[cfg(feature = "track-b")]
#[derive(Parser)]
struct EvalArgs {
    #[command(subcommand)]
    command: EvalCommand,
}

#[cfg(feature = "track-b")]
#[derive(Subcommand)]
enum EvalCommand {
    /// Replay cached LLM responses through the backtest engine
    Replay(EvalReplayArgs),
}

#[cfg(feature = "track-b")]
#[derive(Parser)]
struct EvalReplayArgs {
    /// Path to TOML configuration file (must have [blending] section)
    #[arg(long)]
    config: PathBuf,

    /// LLM model name (must match cached entries)
    #[arg(long)]
    model: String,

    /// Prompt hash (first 16 hex chars of SHA-256)
    #[arg(long)]
    prompt_hash: String,

    /// Start date (YYYY-MM-DD) — bars loaded from this date
    #[arg(long)]
    start: Option<NaiveDate>,

    /// End date (YYYY-MM-DD) — bars loaded up to this date
    #[arg(long)]
    end: Option<NaiveDate>,

    /// Evaluation start date — warmup period before this excluded from metrics
    #[arg(long)]
    eval_start: Option<NaiveDate>,

    /// Comma-separated instrument symbols (default: tradeable universe)
    #[arg(long, value_delimiter = ',')]
    instruments: Option<Vec<String>>,

    /// Path to directory containing CSV data files
    #[arg(long, default_value = "data", env = "QUANTBOT_DATA")]
    data_dir: PathBuf,

    /// Initial portfolio cash
    #[arg(long, default_value_t = 1_000_000.0)]
    initial_cash: f64,

    /// Minimum bars of history required before trading
    #[arg(long, default_value_t = 252)]
    min_history: usize,

    /// Output results as JSON
    #[arg(long)]
    json: bool,
}

#[cfg(feature = "track-b")]
#[derive(Parser)]
struct CacheArgs {
    #[command(subcommand)]
    command: CacheCommand,
}

#[cfg(feature = "track-b")]
#[derive(Subcommand)]
enum CacheCommand {
    /// Batch-fill LLM cache for all (instrument, date) pairs in a range
    Fill(CacheFillArgs),
}

#[cfg(feature = "track-b")]
#[derive(Parser)]
struct CacheFillArgs {
    /// Path to TOML configuration file (must have [llm] section)
    #[arg(long)]
    config: PathBuf,

    /// Start date (YYYY-MM-DD)
    #[arg(long)]
    start: NaiveDate,

    /// End date (YYYY-MM-DD)
    #[arg(long)]
    end: NaiveDate,

    /// Comma-separated instrument symbols
    #[arg(long, value_delimiter = ',')]
    instruments: Option<Vec<String>>,

    /// Use only the 6 tradeable instruments
    #[arg(long)]
    tradeable_only: bool,

    /// Abort after this many consecutive LLM failures (default: 10)
    #[arg(long, default_value_t = 10)]
    max_failures: usize,

    /// Abort on the first LLM failure
    #[arg(long)]
    require_success: bool,

    /// Print per-call progress lines
    #[arg(long)]
    progress: bool,

    /// Path to directory containing CSV data files
    #[arg(long, default_value = "data", env = "QUANTBOT_DATA")]
    data_dir: PathBuf,

    /// Minimum bars of history before a date is eligible (default: 60)
    #[arg(long, default_value_t = 60)]
    min_history: usize,
}

#[derive(Parser)]
struct BacktestArgs {
    /// Comma-separated instrument symbols (default: tradeable universe)
    #[arg(long, value_delimiter = ',')]
    instruments: Option<Vec<String>>,

    /// Start date (YYYY-MM-DD)
    #[arg(long)]
    start: Option<NaiveDate>,

    /// End date (YYYY-MM-DD)
    #[arg(long)]
    end: Option<NaiveDate>,

    /// Evaluation start date — warmup period before this is excluded from metrics
    #[arg(long)]
    eval_start: Option<NaiveDate>,

    /// Path to directory containing CSV data files
    #[arg(long, default_value = "data", env = "QUANTBOT_DATA")]
    data_dir: PathBuf,

    /// Initial portfolio cash
    #[arg(long, default_value_t = 1_000_000.0)]
    initial_cash: f64,

    /// Annualized volatility target
    #[arg(long, default_value_t = 0.40)]
    vol_target: f64,

    /// Maximum gross leverage
    #[arg(long, default_value_t = 2.0)]
    max_gross_leverage: f64,

    /// Maximum position size as fraction of NAV
    #[arg(long, default_value_t = 0.20)]
    max_position_pct: f64,

    /// Minimum bars of history required before trading
    #[arg(long, default_value_t = 252)]
    min_history: usize,

    /// Output results as JSON
    #[arg(long)]
    json: bool,

    /// Write output to file instead of stdout
    #[arg(long, short)]
    output: Option<PathBuf>,
}

#[derive(Parser)]
struct LiveArgs {
    /// Path to TOML configuration file
    #[arg(long)]
    config: PathBuf,

    /// Dry run — compute targets and print orders without executing
    #[arg(long)]
    dry_run: bool,

    /// Path to state file for position persistence
    #[arg(long, default_value = "data/live-state.json")]
    state_file: PathBuf,

    /// Path to directory containing CSV data files
    #[arg(long, default_value = "data", env = "QUANTBOT_DATA")]
    data_dir: PathBuf,

    /// Initial portfolio cash (NAV)
    #[arg(long, default_value_t = 1_000_000.0)]
    initial_cash: f64,

    /// Annualized volatility target
    #[arg(long, default_value_t = 0.40)]
    vol_target: f64,

    /// Maximum gross leverage
    #[arg(long, default_value_t = 2.0)]
    max_gross_leverage: f64,

    /// Maximum position size as fraction of NAV
    #[arg(long, default_value_t = 0.20)]
    max_position_pct: f64,

    /// Minimum bars of history required before trading
    #[arg(long, default_value_t = 252)]
    min_history: usize,

    /// Output results as JSON
    #[arg(long)]
    json: bool,

    /// Filter to a single instrument (safety valve for testing)
    #[arg(long)]
    instrument: Option<String>,

    /// Maximum number of orders to place in a single run
    #[arg(long, default_value_t = 6)]
    max_orders: usize,

    /// Maximum size per order (prevents oversized trades)
    #[arg(long)]
    max_size: Option<f64>,

    /// Flatten all positions and exit
    #[arg(long)]
    flatten: bool,

    /// Allow stale data (skip freshness check)
    #[arg(long)]
    allow_stale: bool,

    /// Maximum days of data staleness before refusing to trade (default: 3)
    #[arg(long, default_value_t = 3)]
    max_stale_days: u32,
}

#[derive(Parser)]
struct PaperTradeArgs {
    /// Comma-separated instrument symbols (default: tradeable universe)
    #[arg(long, value_delimiter = ',')]
    instruments: Option<Vec<String>>,

    /// Path to directory containing CSV data files
    #[arg(long, default_value = "data", env = "QUANTBOT_DATA")]
    data_dir: PathBuf,

    /// Path to TOML configuration file (enables blending when [blending] section present)
    #[cfg(feature = "track-b")]
    #[arg(long)]
    config: Option<PathBuf>,

    /// Initial portfolio cash (NAV)
    #[arg(long, default_value_t = 1_000_000.0)]
    initial_cash: f64,

    /// Annualized volatility target
    #[arg(long, default_value_t = 0.40)]
    vol_target: f64,

    /// Maximum gross leverage
    #[arg(long, default_value_t = 2.0)]
    max_gross_leverage: f64,

    /// Maximum position size as fraction of NAV
    #[arg(long, default_value_t = 0.20)]
    max_position_pct: f64,

    /// Minimum bars of history required before trading
    #[arg(long, default_value_t = 252)]
    min_history: usize,

    /// Output results as JSON
    #[arg(long)]
    json: bool,

    /// Path to state file for position persistence
    #[arg(long, default_value = "data/paper-state.json")]
    state_file: PathBuf,

    /// Reset state (start from flat, ignoring saved positions)
    #[arg(long)]
    reset: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct PaperTradeState {
    date: NaiveDate,
    nav: f64,
    quantities: HashMap<String, f64>,
}

fn load_state(path: &Path) -> Result<Option<PaperTradeState>> {
    if !path.exists() {
        return Ok(None);
    }
    let contents = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let state: PaperTradeState = serde_json::from_str(&contents)
        .with_context(|| format!("failed to parse state file {}", path.display()))?;
    Ok(Some(state))
}

fn save_state(path: &Path, state: &PaperTradeState) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create directory {}", parent.display()))?;
    }
    let json = serde_json::to_string_pretty(state).context("failed to serialize state")?;
    std::fs::write(path, json).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Command::Backtest(args) => run_backtest(args),
        Command::PaperTrade(args) => run_paper_trade(args),
        Command::Live(args) => run_live(args).await,
        Command::Positions(args) => run_positions(args).await,
        Command::History(args) => run_history(args),
        Command::Data(args) => run_data(args).await,
        #[cfg(feature = "track-b")]
        Command::Eval(args) => match args.command {
            EvalCommand::Replay(replay_args) => run_eval_replay(replay_args),
        },
        #[cfg(feature = "track-b")]
        Command::Cache(args) => match args.command {
            CacheCommand::Fill(fill_args) => run_cache_fill(fill_args).await,
        },
    };

    if let Err(e) = result {
        eprintln!("Error: {e:#}");
        process::exit(1);
    }
}

fn run_backtest(args: BacktestArgs) -> Result<()> {
    // Resolve instruments
    let symbols: Vec<String> = match args.instruments {
        Some(syms) => syms,
        None => TRADEABLE_UNIVERSE
            .iter()
            .map(|i| i.symbol.clone())
            .collect(),
    };

    // Validate data directory
    if !args.data_dir.is_dir() {
        bail!("Data directory does not exist: {}", args.data_dir.display());
    }

    // Load bars
    let loader = CsvLoader::new(&args.data_dir);
    let mut bars: HashMap<String, _> = HashMap::new();
    let mut load_errors: Vec<String> = Vec::new();

    for sym in &symbols {
        match loader.load_bars(sym, args.start, args.end) {
            Ok(series) => {
                eprintln!("  Loaded {} bars for {}", series.bars().len(), sym);
                bars.insert(sym.clone(), series);
            }
            Err(e) => {
                eprintln!("  Warning: failed to load {sym}: {e}");
                load_errors.push(sym.clone());
            }
        }
    }

    if bars.is_empty() {
        bail!("Failed to load data for any instrument");
    }

    if !load_errors.is_empty() {
        eprintln!(
            "  Continuing with {}/{} instruments",
            bars.len(),
            symbols.len()
        );
    }

    // Build config and run
    let config = BacktestConfig {
        initial_cash: args.initial_cash,
        vol_target: args.vol_target,
        max_gross_leverage: args.max_gross_leverage,
        max_position_pct: args.max_position_pct,
    };

    let engine = BacktestEngine::new(config);
    let agent = TSMOMAgent::new();

    eprintln!("  Running backtest...");
    let snapshots = engine.run(&agent, &bars, args.min_history, args.eval_start);

    let result = BacktestResult::from_snapshots(&snapshots)
        .context("Not enough snapshots to compute metrics")?;

    // Output
    let output_text = if args.json {
        serde_json::to_string_pretty(&result).context("Failed to serialize results")?
    } else {
        result.summary()
    };

    if let Some(path) = args.output {
        std::fs::write(&path, &output_text)
            .with_context(|| format!("failed to write {}", path.display()))?;
        eprintln!("  Results written to {}", path.display());
    } else {
        println!("{output_text}");
    }

    Ok(())
}

#[cfg(feature = "track-b")]
struct InstrumentAttribution {
    pnl_usd: f64,
    sum_abs_notional: f64,
    trade_count: usize,
    indicator_used_days: usize,
    total_days: usize,
}

#[cfg(feature = "track-b")]
fn compute_attribution(
    snapshots: &[quantbot::backtest::engine::Snapshot],
) -> HashMap<String, InstrumentAttribution> {
    let mut attr: HashMap<String, InstrumentAttribution> = HashMap::new();
    let mut prev_notional: HashMap<String, f64> = HashMap::new();

    for (i, snap) in snapshots.iter().enumerate() {
        // Collect all instruments seen in positions or signals
        let mut syms: Vec<&String> = snap
            .position_notionals
            .keys()
            .chain(snap.signals.keys())
            .collect();
        syms.sort();
        syms.dedup();

        for sym in &syms {
            let notional = snap.position_notionals.get(*sym).copied().unwrap_or(0.0);
            let entry = attr.entry((*sym).clone()).or_insert(InstrumentAttribution {
                pnl_usd: 0.0,
                sum_abs_notional: 0.0,
                trade_count: 0,
                indicator_used_days: 0,
                total_days: 0,
            });

            if i > 0 {
                let prev = prev_notional.get(*sym).copied().unwrap_or(0.0);
                entry.pnl_usd += notional - prev;
            }
            entry.sum_abs_notional += notional.abs();
            entry.total_days += 1;

            if let Some(sig) = snap.signals.get(*sym) {
                if sig.metadata.get("indicator_used").copied().unwrap_or(0.0) == 1.0 {
                    entry.indicator_used_days += 1;
                }
            }
        }

        // Update prev_notional
        prev_notional.clear();
        for sym in &syms {
            let notional = snap.position_notionals.get(*sym).copied().unwrap_or(0.0);
            prev_notional.insert((*sym).clone(), notional);
        }

        // Count fills
        for fill in &snap.fills {
            let entry =
                attr.entry(fill.order.instrument.clone())
                    .or_insert(InstrumentAttribution {
                        pnl_usd: 0.0,
                        sum_abs_notional: 0.0,
                        trade_count: 0,
                        indicator_used_days: 0,
                        total_days: 0,
                    });
            entry.trade_count += 1;
        }
    }
    attr
}

#[cfg(feature = "track-b")]
fn run_eval_replay(args: EvalReplayArgs) -> Result<()> {
    use std::sync::{Arc, Mutex};

    // Load config and validate blending is enabled
    let app_config = AppConfig::load(&args.config)
        .with_context(|| format!("failed to load config from {}", args.config.display()))?;

    let blend_config = app_config
        .blending
        .as_ref()
        .filter(|b| b.enabled)
        .context("blending must be enabled in config for eval replay")?;

    // Resolve instruments
    let symbols: Vec<String> = match args.instruments {
        Some(syms) => syms,
        None => TRADEABLE_UNIVERSE
            .iter()
            .map(|i| i.symbol.clone())
            .collect(),
    };

    if !args.data_dir.is_dir() {
        bail!("Data directory does not exist: {}", args.data_dir.display());
    }

    // Open DB and check coverage
    let db_path = args.data_dir.join("quantbot.db");
    let db = Db::open(&db_path)
        .with_context(|| format!("failed to open database at {}", db_path.display()))?;

    let coverage = db
        .llm_cache_coverage(&args.model, &args.prompt_hash)
        .context("failed to query LLM cache coverage")?;

    eprintln!("  Cache coverage for model={} prompt_hash={}:", args.model, args.prompt_hash);
    let mut total_cached = 0usize;
    for sym in &symbols {
        let count = coverage.get(sym).copied().unwrap_or(0);
        total_cached += count;
        let status = if count == 0 { " (NONE)" } else { "" };
        eprintln!("    {sym:<14} {count} cached entries{status}");
    }

    if total_cached == 0 {
        eprintln!("  WARNING: zero cached entries found — replay will produce TSMOM-only results");
    }

    // Build CachedIndicatorAgent
    let db = Arc::new(Mutex::new(db));
    let indicator = CachedIndicatorAgent::new(
        Arc::clone(&db),
        args.model.clone(),
        args.prompt_hash.clone(),
    );

    // Load bars
    let loader = CsvLoader::new(&args.data_dir);
    let mut bars: HashMap<String, _> = HashMap::new();

    for sym in &symbols {
        match loader.load_bars(sym, args.start, args.end) {
            Ok(series) => {
                eprintln!("  Loaded {} bars for {}", series.bars().len(), sym);
                bars.insert(sym.clone(), series);
            }
            Err(e) => {
                eprintln!("  Warning: failed to load {sym}: {e}");
            }
        }
    }

    if bars.is_empty() {
        bail!("Failed to load data for any instrument");
    }

    // Build engine and run blended backtest
    let config = BacktestConfig {
        initial_cash: args.initial_cash,
        ..BacktestConfig::default()
    };

    let engine = BacktestEngine::new(config);
    let tsmom = TSMOMAgent::new();

    eprintln!("  Running blended replay...");
    let snapshots = engine.run_blended(
        &tsmom,
        &indicator,
        &bars,
        blend_config,
        args.min_history,
        args.eval_start,
    );

    let result = BacktestResult::from_snapshots(&snapshots)
        .context("Not enough snapshots to compute metrics")?;

    // Also run TSMOM-only for comparison
    eprintln!("  Running TSMOM-only baseline...");
    let tsmom_snapshots = engine.run(&tsmom, &bars, args.min_history, args.eval_start);
    let tsmom_result = BacktestResult::from_snapshots(&tsmom_snapshots);

    // Coverage report
    let cov_report = indicator.coverage_report();

    if args.json {
        let mut output = serde_json::Map::new();
        output.insert(
            "blended".into(),
            serde_json::to_value(&result).unwrap(),
        );
        if let Some(ref tr) = tsmom_result {
            output.insert(
                "tsmom_only".into(),
                serde_json::to_value(tr).unwrap(),
            );
        }
        output.insert("cache_hit_rate".into(), serde_json::Value::from(cov_report.hit_rate()));
        output.insert("cache_hits".into(), serde_json::Value::from(cov_report.total_hits as u64));
        output.insert("cache_misses".into(), serde_json::Value::from(cov_report.total_misses as u64));

        // Per-instrument attribution
        let blended_attr = compute_attribution(&snapshots);
        let baseline_attr = compute_attribution(&tsmom_snapshots);
        let initial_nav = args.initial_cash;

        let mut attr_map = serde_json::Map::new();
        for (sym, b) in &blended_attr {
            let base_pnl = baseline_attr.get(sym).map_or(0.0, |x| x.pnl_usd);
            let avg_abs_pct = if b.total_days > 0 {
                (b.sum_abs_notional / b.total_days as f64) / initial_nav * 100.0
            } else {
                0.0
            };
            let ind_used_pct = if b.total_days > 0 {
                100.0 * b.indicator_used_days as f64 / b.total_days as f64
            } else {
                0.0
            };
            let mut entry = serde_json::Map::new();
            entry.insert("blended_contribution".into(), serde_json::Value::from(b.pnl_usd));
            entry.insert("tsmom_contribution".into(), serde_json::Value::from(base_pnl));
            entry.insert("delta".into(), serde_json::Value::from(b.pnl_usd - base_pnl));
            entry.insert("avg_abs_position_pct".into(), serde_json::Value::from(avg_abs_pct));
            entry.insert("trade_count".into(), serde_json::Value::from(b.trade_count as u64));
            entry.insert("indicator_used_pct".into(), serde_json::Value::from(ind_used_pct));
            entry.insert("total_days".into(), serde_json::Value::from(b.total_days as u64));
            attr_map.insert(sym.clone(), serde_json::Value::Object(entry));
        }
        output.insert("attribution".into(), serde_json::Value::Object(attr_map));

        // Asset-class rollup in JSON
        {
            use quantbot::agents::combiner::blend_category;
            use quantbot::config::BlendCategory;

            let mut class_rollup = serde_json::Map::new();
            for cat in [BlendCategory::Gold, BlendCategory::Equity, BlendCategory::Forex] {
                let mut bc = 0.0_f64;
                let mut tc = 0.0_f64;
                let mut trades = 0usize;
                for (sym, b) in &blended_attr {
                    if blend_category(sym) == cat {
                        bc += b.pnl_usd;
                        trades += b.trade_count;
                    }
                }
                for (sym, b) in &baseline_attr {
                    if blend_category(sym) == cat {
                        tc += b.pnl_usd;
                    }
                }
                let mut entry = serde_json::Map::new();
                entry.insert("blended_contribution".into(), serde_json::Value::from(bc));
                entry.insert("tsmom_contribution".into(), serde_json::Value::from(tc));
                entry.insert("delta".into(), serde_json::Value::from(bc - tc));
                entry.insert("trade_count".into(), serde_json::Value::from(trades as u64));
                class_rollup.insert(cat.to_string(), serde_json::Value::Object(entry));
            }
            output.insert("attribution_by_class".into(), serde_json::Value::Object(class_rollup));
        }

        let json = serde_json::to_string_pretty(&output).context("failed to serialize")?;
        println!("{json}");
    } else {
        println!("═══ BLENDED REPLAY ═══");
        println!("{}", result.summary());

        if let Some(ref tr) = tsmom_result {
            println!("═══ TSMOM-ONLY BASELINE ═══");
            println!("{}", tr.summary());

            println!("═══ COMPARISON ═══");
            println!(
                "  Sharpe:  blended {:.3} vs TSMOM {:.3} (delta {:+.3})",
                result.sharpe_ratio,
                tr.sharpe_ratio,
                result.sharpe_ratio - tr.sharpe_ratio,
            );
            println!(
                "  Return:  blended {:.1}% vs TSMOM {:.1}%",
                result.annualized_return * 100.0,
                tr.annualized_return * 100.0,
            );
            println!(
                "  Max DD:  blended {:.1}% vs TSMOM {:.1}%",
                result.max_drawdown * 100.0,
                tr.max_drawdown * 100.0,
            );
            println!(
                "  Trades:  blended {} vs TSMOM {}",
                result.total_trades, tr.total_trades,
            );
        }

        // Per-instrument attribution
        let blended_attr = compute_attribution(&snapshots);
        let baseline_attr = compute_attribution(&tsmom_snapshots);

        println!();
        println!("═══ PER-INSTRUMENT ATTRIBUTION ═══");
        println!(
            "{:<14} {:>12} {:>12} {:>10} {:>10} {:>8} {:>10}",
            "Instrument", "Blnd Contr", "TSMOM Contr", "Delta", "Avg|Pos|%", "Trades", "Ind Used%"
        );

        let initial_nav = args.initial_cash;
        let mut sorted_syms: Vec<&String> = blended_attr.keys().collect();
        sorted_syms.sort_by(|a, b| {
            let da = blended_attr[*a].pnl_usd
                - baseline_attr.get(*a).map_or(0.0, |x| x.pnl_usd);
            let db = blended_attr[*b].pnl_usd
                - baseline_attr.get(*b).map_or(0.0, |x| x.pnl_usd);
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut total_blended = 0.0;
        let mut total_baseline = 0.0;
        for sym in &sorted_syms {
            let b = &blended_attr[*sym];
            let base_pnl = baseline_attr.get(*sym).map_or(0.0, |x| x.pnl_usd);
            let delta = b.pnl_usd - base_pnl;
            let avg_abs_pct = if b.total_days > 0 {
                (b.sum_abs_notional / b.total_days as f64) / initial_nav * 100.0
            } else {
                0.0
            };
            let ind_pct = if b.total_days > 0 {
                format!(
                    "{:.0}%",
                    100.0 * b.indicator_used_days as f64 / b.total_days as f64
                )
            } else {
                "-".to_string()
            };
            println!(
                "{:<14} {:>12.0} {:>12.0} {:>+10.0} {:>9.1}% {:>8} {:>10}",
                sym, b.pnl_usd, base_pnl, delta, avg_abs_pct, b.trade_count, ind_pct,
            );
            total_blended += b.pnl_usd;
            total_baseline += base_pnl;
        }

        println!(
            "{:<14} {:>12.0} {:>12.0} {:>+10.0}",
            "TOTAL",
            total_blended,
            total_baseline,
            total_blended - total_baseline,
        );

        // Asset-class rollup
        {
            use quantbot::agents::combiner::blend_category;
            use quantbot::config::BlendCategory;

            let mut class_blended: HashMap<BlendCategory, f64> = HashMap::new();
            let mut class_baseline: HashMap<BlendCategory, f64> = HashMap::new();
            let mut class_trades: HashMap<BlendCategory, usize> = HashMap::new();

            for (sym, b) in &blended_attr {
                let cat = blend_category(sym);
                *class_blended.entry(cat).or_default() += b.pnl_usd;
                *class_trades.entry(cat).or_default() += b.trade_count;
            }
            for (sym, b) in &baseline_attr {
                let cat = blend_category(sym);
                *class_baseline.entry(cat).or_default() += b.pnl_usd;
            }

            println!();
            println!(
                "{:<14} {:>12} {:>12} {:>10} {:>30} {:>8}",
                "Asset Class", "Blnd Contr", "TSMOM Contr", "Delta", "", "Trades"
            );
            for cat in [BlendCategory::Gold, BlendCategory::Equity, BlendCategory::Forex] {
                let bc = class_blended.get(&cat).copied().unwrap_or(0.0);
                let tc = class_baseline.get(&cat).copied().unwrap_or(0.0);
                let trades = class_trades.get(&cat).copied().unwrap_or(0);
                println!(
                    "{:<14} {:>12.0} {:>12.0} {:>+10.0} {:>30} {:>8}",
                    format!("[{cat}]"), bc, tc, bc - tc, "", trades,
                );
            }
        }

        // Sanity check: sum of per-instrument PnL vs portfolio-level
        let portfolio_pnl = result.total_return * initial_nav;
        let pnl_diff_pct = if portfolio_pnl.abs() > 1.0 {
            ((total_blended - portfolio_pnl) / portfolio_pnl.abs() * 100.0).abs()
        } else {
            0.0
        };
        if pnl_diff_pct > 1.0 {
            println!(
                "  WARNING: per-instrument PnL sum ({:.0}) differs from portfolio PnL ({:.0}) by {:.1}%",
                total_blended, portfolio_pnl, pnl_diff_pct,
            );
        }

        println!();
        println!("═══ CACHE COVERAGE ═══");
        print!("{cov_report}");
    }

    Ok(())
}

#[cfg(feature = "track-b")]
async fn run_cache_fill(args: CacheFillArgs) -> Result<()> {
    use quantbot::agents::indicator::llm_client::LlmClient;
    use quantbot::agents::indicator::parser::parse_llm_response;
    use quantbot::agents::indicator::prompt_loader::{self, sha256_short};
    use quantbot::agents::indicator::ta::TaSnapshot;
    use quantbot::core::bar::BarSeries;
    use quantbot::db::LlmCacheEntry;

    // Phase 1: Setup
    let app_config = AppConfig::load(&args.config)
        .with_context(|| format!("failed to load config from {}", args.config.display()))?;

    let llm_config = app_config
        .llm
        .context("config must have an [llm] section for cache fill")?;

    let loaded_prompt = prompt_loader::load(llm_config.prompt_path.as_deref());
    let model = llm_config.model.clone();
    let mut client = LlmClient::new(llm_config).context("failed to create LLM client")?;

    // Resolve instruments
    let symbols: Vec<String> = if let Some(syms) = args.instruments {
        syms
    } else if args.tradeable_only {
        TRADEABLE_UNIVERSE.iter().map(|i| i.symbol.clone()).collect()
    } else {
        TRADEABLE_UNIVERSE.iter().map(|i| i.symbol.clone()).collect()
    };

    if !args.data_dir.is_dir() {
        bail!("Data directory does not exist: {}", args.data_dir.display());
    }

    let db_path = args.data_dir.join("quantbot.db");
    let db = Db::open(&db_path)
        .with_context(|| format!("failed to open database at {}", db_path.display()))?;

    eprintln!("  Cache fill setup:");
    eprintln!("    Model:       {model}");
    eprintln!("    Prompt hash: {}", loaded_prompt.hash);
    eprintln!("    Date range:  {} to {}", args.start, args.end);
    eprintln!("    Instruments: {}", symbols.join(", "));
    eprintln!("    Min history: {}", args.min_history);

    // Phase 2: Build work list
    struct WorkItem {
        instrument: String,
        eval_date: String,
        user_prompt: String,
        cache_key: String,
        ta_hash: String,
    }

    let loader = CsvLoader::new(&args.data_dir);
    let mut work_items: Vec<WorkItem> = Vec::new();
    let mut already_cached = 0usize;
    let mut eligible_per_instrument: HashMap<String, usize> = HashMap::new();

    for sym in &symbols {
        // Load all bars (no date filter — need full history for TA)
        let all_bars = match loader.load_bars(sym, None, None) {
            Ok(series) => series,
            Err(e) => {
                eprintln!("  Warning: failed to load {sym}: {e}");
                continue;
            }
        };

        let bars_slice = all_bars.bars();
        let mut instrument_eligible = 0usize;

        for (i, bar) in bars_slice.iter().enumerate() {
            if bar.date < args.start || bar.date > args.end {
                continue;
            }
            // Need at least min_history bars (index i means i+1 bars in [0..=i])
            if i + 1 < args.min_history {
                continue;
            }

            instrument_eligible += 1;

            // Build sub-series for TA computation
            let sub_bars = BarSeries::new(bars_slice[0..=i].to_vec())
                .expect("sub-series from valid bars");
            let snapshot = TaSnapshot::compute(&sub_bars);
            let user_prompt = format!(
                "Instrument: {}\n\n{}",
                sym,
                snapshot.format_for_prompt()
            );

            let eval_date = bar.date.to_string();
            let ta_hash = sha256_short(&user_prompt);
            let cache_key = format!(
                "{}|{}|{}|{}|{}",
                model, loaded_prompt.hash, sym, eval_date, ta_hash
            );

            // Check existing cache
            match db.get_llm_cache(&cache_key)? {
                Some(entry) if entry.llm_ok && entry.parse_ok => {
                    already_cached += 1;
                    continue;
                }
                Some(_) => {
                    // Failed entry — delete to allow retry
                    db.delete_llm_cache(&cache_key)?;
                }
                None => {}
            }

            work_items.push(WorkItem {
                instrument: sym.clone(),
                eval_date,
                user_prompt,
                cache_key,
                ta_hash,
            });
        }

        eligible_per_instrument.insert(sym.clone(), instrument_eligible);
        eprintln!(
            "  Loaded {} bars for {} ({} eligible dates)",
            bars_slice.len(),
            sym,
            instrument_eligible
        );
    }

    let total_pairs: usize = eligible_per_instrument.values().sum();
    eprintln!();
    eprintln!(
        "  Work list: {} new calls, {} already cached, {} total eligible",
        work_items.len(),
        already_cached,
        total_pairs
    );

    if work_items.is_empty() {
        eprintln!("  Nothing to do — all pairs already cached.");
        return Ok(());
    }

    // Phase 3: Execute LLM calls
    let total_work = work_items.len();
    let mut successes = 0usize;
    let mut failures = 0usize;
    let mut consecutive_failures = 0usize;
    let start_time = std::time::Instant::now();

    for (idx, item) in work_items.iter().enumerate() {
        let call_start = std::time::Instant::now();
        let result = client.chat(&loaded_prompt.text, &item.user_prompt).await;
        let latency_ms = call_start.elapsed().as_millis() as u64;

        let (response_text, llm_ok, parse_ok) = match result {
            Ok(raw) => {
                let p_ok = parse_llm_response(&raw).is_ok();
                (raw, true, p_ok)
            }
            Err(e) => (e.to_string(), false, false),
        };

        let entry = LlmCacheEntry {
            cache_key: item.cache_key.clone(),
            llm_model: model.clone(),
            prompt_hash: loaded_prompt.hash.clone(),
            instrument: item.instrument.clone(),
            eval_date: item.eval_date.clone(),
            ta_hash: item.ta_hash.clone(),
            response_text,
            llm_ok,
            parse_ok,
            latency_ms: Some(latency_ms),
            created_at: chrono::Utc::now()
                .to_rfc3339_opts(chrono::SecondsFormat::Micros, true),
        };
        db.insert_llm_cache(&entry)?;

        let ok = llm_ok && parse_ok;
        if ok {
            successes += 1;
            consecutive_failures = 0;
        } else {
            failures += 1;
            consecutive_failures += 1;
        }

        if args.progress {
            let pct = (idx + 1) as f64 / total_work as f64 * 100.0;
            let status = if ok { "OK" } else { "FAIL" };
            eprintln!(
                "  [{}/{}] ({:.0}%) {} {} {} latency={}ms",
                idx + 1,
                total_work,
                pct,
                item.instrument,
                item.eval_date,
                status,
                latency_ms,
            );
        }

        if args.require_success && !ok {
            bail!(
                "Cache fill aborted: --require-success and call failed for {} {}",
                item.instrument,
                item.eval_date
            );
        }

        if consecutive_failures >= args.max_failures {
            bail!(
                "Cache fill aborted: {} consecutive failures (max_failures={})",
                consecutive_failures,
                args.max_failures
            );
        }
    }

    let duration = start_time.elapsed();

    // Phase 4: Summary
    let coverage = db
        .llm_cache_coverage(&model, &loaded_prompt.hash)
        .context("failed to query coverage")?;

    println!();
    println!("Cache fill complete:");
    println!("  Total pairs:    {}", total_pairs);
    println!("  Already cached: {} (skipped)", already_cached);
    println!("  New calls:      {}", total_work);
    println!("  Successes:      {}", successes);
    println!("  Failures:       {}", failures);
    println!("  Duration:       {:.1}s", duration.as_secs_f64());
    println!();
    println!("Coverage by instrument:");
    for sym in &symbols {
        let cached = coverage.get(sym).copied().unwrap_or(0);
        let eligible = eligible_per_instrument.get(sym).copied().unwrap_or(0);
        let pct = if eligible > 0 {
            cached as f64 / eligible as f64 * 100.0
        } else {
            0.0
        };
        println!("  {sym:<14} {cached}/{eligible} ({pct:.0}%)");
    }

    Ok(())
}

fn run_paper_trade(args: PaperTradeArgs) -> Result<()> {
    let symbols: Vec<String> = match args.instruments {
        Some(syms) => syms,
        None => TRADEABLE_UNIVERSE
            .iter()
            .map(|i| i.symbol.clone())
            .collect(),
    };

    if !args.data_dir.is_dir() {
        bail!("Data directory does not exist: {}", args.data_dir.display());
    }

    // Load all bars (no date filter — need full history for lookback)
    let loader = CsvLoader::new(&args.data_dir);
    let mut bars: HashMap<String, _> = HashMap::new();

    for sym in &symbols {
        match loader.load_bars(sym, None, None) {
            Ok(series) => {
                eprintln!("  Loaded {} bars for {}", series.bars().len(), sym);
                bars.insert(sym.clone(), series);
            }
            Err(e) => {
                eprintln!("  Warning: failed to load {sym}: {e}");
            }
        }
    }

    if bars.is_empty() {
        bail!("Failed to load data for any instrument");
    }

    let config = BacktestConfig {
        initial_cash: args.initial_cash,
        vol_target: args.vol_target,
        max_gross_leverage: args.max_gross_leverage,
        max_position_pct: args.max_position_pct,
    };

    let engine = BacktestEngine::new(config);
    let agent = TSMOMAgent::new();

    // Load prior state
    let (current_quantities, nav, prior_state) = if args.reset {
        (HashMap::new(), args.initial_cash, None)
    } else {
        match load_state(&args.state_file)? {
            Some(state) => {
                eprintln!(
                    "  Loaded state from {} ({}, {} positions)",
                    args.state_file.display(),
                    state.date,
                    state.quantities.len()
                );
                let nav = state.nav;
                let quantities = state.quantities.clone();
                (quantities, nav, Some(state))
            }
            None => (HashMap::new(), args.initial_cash, None),
        }
    };

    // ── Optional blending (track-b with --config) ─────────────────
    #[cfg(feature = "track-b")]
    let (snapshot, _combined_results): (TargetSnapshot, Option<Vec<combiner::CombinedResult>>) = {
        let app_config = args
            .config
            .as_ref()
            .map(|p| AppConfig::load(p))
            .transpose()
            .with_context(|| "failed to load config for blending")?;

        let blend_enabled = app_config
            .as_ref()
            .and_then(|c| c.blending.as_ref())
            .is_some_and(|b| b.enabled);

        if blend_enabled {
            let app_config = app_config.as_ref().unwrap();
            let blend_config = app_config.blending.as_ref().unwrap();
            eprintln!("  Blending mode: ENABLED");

            // Create indicator agent
            let indicator: Box<dyn SignalAgent> = match &app_config.llm {
                Some(llm_config) => match LlmIndicatorAgent::new(llm_config.clone()) {
                    Ok(a) => {
                        eprintln!("  Using LLM indicator agent (model: {})", llm_config.model);
                        Box::new(a)
                    }
                    Err(e) => {
                        eprintln!("  WARN: LLM agent init failed: {e}, falling back to RSI");
                        Box::new(DummyIndicatorAgent::new())
                    }
                },
                None => Box::new(DummyIndicatorAgent::new()),
            };

            // Generate TSMOM signals
            let mut tsmom_signals = HashMap::new();
            let mut tsmom_weights = HashMap::new();
            for (sym, series) in &bars {
                if series.bars().len() < args.min_history {
                    continue;
                }
                let sig = agent.generate_signal(series, sym);
                let weight = if sig.direction != SignalDirection::Flat {
                    TSMOMAgent::compute_target_weight(&sig)
                } else {
                    0.0
                };
                tsmom_weights.insert(sym.clone(), weight);
                tsmom_signals.insert(sym.clone(), sig);
            }

            // Generate indicator signals with latency
            let mut indicator_map = HashMap::new();
            let mut syms: Vec<&String> = bars.keys().collect();
            syms.sort();
            for sym in syms {
                let start = std::time::Instant::now();
                let mut sig = indicator.generate_signal(&bars[sym], sym);
                let latency = start.elapsed().as_secs_f64() * 1000.0;
                sig.metadata.insert("latency_ms".into(), latency);
                indicator_map.insert(sym.clone(), sig);
            }

            // Drain and write LLM cache entries
            let cache_entries = indicator.take_cache_entries();
            if !cache_entries.is_empty() {
                let db_path = args.data_dir.join("quantbot.db");
                if let Ok(db) = Db::open(&db_path) {
                    let mut written = 0;
                    for entry in &cache_entries {
                        if let Err(e) = db.insert_llm_cache(entry) {
                            eprintln!("  WARN: cache write failed for {}: {e}", entry.instrument);
                        } else {
                            written += 1;
                        }
                    }
                    if written > 0 {
                        eprintln!("  Cached {written} LLM response(s) to SQLite");
                    }
                }
            }

            // Combine
            let results = combiner::combine_signals(&tsmom_signals, &indicator_map, blend_config);
            let mut combined_weights = HashMap::new();
            let mut combined_signals = HashMap::new();
            for r in &results {
                combined_weights.insert(r.instrument.clone(), r.combined_weight);
                let combined_sig = combiner::build_combined_signal(
                    r,
                    &tsmom_signals[&r.instrument],
                    indicator_map.get(&r.instrument),
                );
                combined_signals.insert(r.instrument.clone(), combined_sig);
            }

            let snap = engine.generate_targets_with_overrides(
                combined_signals,
                combined_weights,
                &bars,
                &current_quantities,
                nav,
            );

            // Print blending summary
            println!();
            println!("  BLENDED WEIGHTS");
            println!(
                "  {:<14} {:<10} {:>8} {:>10} {:>10} {:>6}",
                "Instrument", "Category", "TSMOM", "Indicator", "Combined", "Used?"
            );
            for r in &results {
                let used_str = if r.indicator_used {
                    "yes"
                } else if r.blend_indicator == 0.0 {
                    "no (100% tsmom)"
                } else {
                    "no (fallback)"
                };
                println!(
                    "  {:<14} {:<10} {:>+8.2} {:>+10.2} {:>+10.2} {:>6}",
                    r.instrument, r.blend_category, r.tsmom_weight, r.indicator_weight,
                    r.combined_weight, used_str,
                );
            }
            println!();

            (snap, Some(results))
        } else {
            let snap =
                engine.generate_targets(&agent, &bars, &current_quantities, nav, args.min_history);
            (snap, None)
        }
    };

    #[cfg(not(feature = "track-b"))]
    let snapshot =
        engine.generate_targets(&agent, &bars, &current_quantities, nav, args.min_history);

    // Save new state
    save_state(
        &args.state_file,
        &PaperTradeState {
            date: snapshot.date,
            nav, // v1: NAV doesn't change without live prices
            quantities: snapshot.target_quantities.clone(),
        },
    )?;
    eprintln!("  State saved to {}", args.state_file.display());

    if args.json {
        let json =
            serde_json::to_string_pretty(&snapshot).context("Failed to serialize targets")?;
        println!("{json}");
    } else {
        print_paper_trade_report(&snapshot, nav, prior_state.as_ref());
    }

    Ok(())
}

async fn run_live(args: LiveArgs) -> Result<()> {
    let run_id = RunId::now();
    let mut audit = AuditLogger::new(run_id, Path::new("data/audit"));

    let app_config = AppConfig::load(&args.config)
        .with_context(|| format!("failed to load config from {}", args.config.display()))?;

    // ── Handle --flatten early ──────────────────────────────────
    if args.flatten {
        if args.dry_run {
            bail!("--flatten and --dry-run are mutually exclusive");
        }
        return run_flatten(&app_config).await;
    }

    let ig_config = app_config
        .execution
        .ig
        .as_ref()
        .context("IG config required for live trading")?;

    // ── Resolve instruments (respect --instrument filter) ───────
    let all_symbols: Vec<String> = ig_config.instruments.keys().cloned().collect();

    let symbols: Vec<String> = match &args.instrument {
        Some(sym) => {
            if !all_symbols.contains(sym) {
                bail!("instrument '{}' not found in config", sym);
            }
            vec![sym.clone()]
        }
        None => all_symbols,
    };

    if !args.data_dir.is_dir() {
        bail!("Data directory does not exist: {}", args.data_dir.display());
    }

    // ── Load bar data ──────────────────────────────────────────
    let loader = CsvLoader::new(&args.data_dir);
    let mut bars: HashMap<String, _> = HashMap::new();

    for sym in &symbols {
        match loader.load_bars(sym, None, None) {
            Ok(series) => {
                eprintln!("  Loaded {} bars for {}", series.bars().len(), sym);
                bars.insert(sym.clone(), series);
            }
            Err(e) => {
                eprintln!("  Warning: failed to load {sym}: {e}");
            }
        }
    }

    if bars.is_empty() {
        bail!("Failed to load data for any instrument");
    }

    // ── Freshness gate ──────────────────────────────────────────
    if !args.allow_stale {
        let today = chrono::Utc::now().date_naive();
        let symbols_with_dates: Vec<(String, Option<NaiveDate>)> = symbols
            .iter()
            .map(|sym| {
                let last = bars.get(sym).and_then(|s| s.bars().last().map(|b| b.date));
                (sym.clone(), last)
            })
            .collect();

        let stale_errors =
            freshness::check_all_fresh(&symbols_with_dates, today, args.max_stale_days);

        if !stale_errors.is_empty() {
            eprintln!("  Data freshness check FAILED:");
            for err in &stale_errors {
                eprintln!("    {err}");
            }
            eprintln!(
                "\n  Run `quantbot data --tradeable-only` to update, or pass --allow-stale to override."
            );
            bail!(
                "{} instrument(s) have stale data — refusing to trade",
                stale_errors.len()
            );
        }
        eprintln!("  Data freshness check passed");
    }

    // ── Compute NAV (MTM for IG, state for Paper) ───────────
    // For IG: fetch live positions, compute mark-to-market NAV from
    //   initial_cash + unrealized P&L (signed_size * (current_price - open_level))
    // For Paper: use saved state NAV or initial_cash
    let ig_engine = if matches!(app_config.execution.engine, EngineType::Ig) {
        Some(
            IgExecutionEngine::new(ig_config)
                .map_err(|e| anyhow::anyhow!("failed to create IG engine: {e}"))?,
        )
    } else {
        None
    };

    let (nav, mtm_result) = if let Some(engine) = &ig_engine {
        eprintln!("  Authenticating for MTM NAV...");
        engine
            .health_check()
            .await
            .context("health check failed")?;
        let positions = engine
            .get_positions()
            .await
            .context("failed to fetch positions for MTM")?;

        // Extract open levels (first deal per instrument)
        let mut open_levels: HashMap<String, f64> = HashMap::new();
        for pos in &positions {
            open_levels
                .entry(pos.instrument.clone())
                .or_insert(pos.open_level);
        }

        // Extract current prices from loaded bars (latest close)
        let mut current_prices: HashMap<String, f64> = HashMap::new();
        for (sym, series) in &bars {
            if let Some(last) = series.bars().last() {
                current_prices.insert(sym.clone(), last.close);
            }
        }

        let actual_signed = reconcile::positions_to_signed(&positions);
        let result = mtm::mark_to_market(
            args.initial_cash,
            &actual_signed,
            &open_levels,
            &current_prices,
        );

        eprintln!(
            "  MTM NAV: ${} (unrealized P&L: ${}, {} position(s))",
            format_number(result.nav),
            format_number(result.unrealized_pnl),
            result.positions.len(),
        );

        (result.nav, Some(result))
    } else {
        // Paper engine: use state NAV or initial_cash
        let nav = match load_state(&args.state_file)? {
            Some(state) => {
                eprintln!("  Loaded NAV from state: ${}", format_number(state.nav));
                state.nav
            }
            None => args.initial_cash,
        };
        (nav, None)
    };

    let engine_name = match app_config.execution.engine {
        EngineType::Ig => "ig",
        EngineType::Paper => "paper",
    };
    let ig_env = ig_config.environment;
    audit.log_run_start(
        "live",
        engine_name,
        args.dry_run,
        &args.config.display().to_string(),
        &symbols,
        nav,
        Some(match ig_env {
            quantbot::config::IgEnvironment::Demo => "DEMO",
            quantbot::config::IgEnvironment::Live => "LIVE",
        }),
        Some(&args.state_file.display().to_string()),
    );

    // Log MTM NAV audit event (after run_start)
    if let Some(ref mtm) = mtm_result {
        audit.log_nav_mtm(
            args.initial_cash,
            mtm.unrealized_pnl,
            mtm.nav,
            &mtm.positions,
        );
    }

    let config = BacktestConfig {
        initial_cash: args.initial_cash,
        vol_target: args.vol_target,
        max_gross_leverage: args.max_gross_leverage,
        max_position_pct: args.max_position_pct,
    };
    // Use IG-calibrated point values so generate_targets() produces deal sizes
    // in IG spread-bet units (£/pip for FX, £/point for equity/commodity)
    let ig_router = ig_config.to_execution_router();
    let bt_engine = BacktestEngine::new_with_router(config, ig_router);
    let agent = TSMOMAgent::new();

    // ── Generate indicator signals (track-b only) ────────────────
    #[cfg(feature = "track-b")]
    let mut prompt_info: Option<(String, String, String)> = None; // (hash, source, model)
    #[cfg(feature = "track-b")]
    let llm_cache_entries: Vec<quantbot::db::LlmCacheEntry>;
    #[cfg(feature = "track-b")]
    let indicator_signals: Vec<(String, quantbot::core::signal::Signal)> = {
        let indicator: Box<dyn SignalAgent> = match &app_config.llm {
            Some(llm_config) => match LlmIndicatorAgent::new(llm_config.clone()) {
                Ok(agent) => {
                    eprintln!("  Using LLM indicator agent (model: {})", llm_config.model);
                    let lp = agent.loaded_prompt();
                    prompt_info = Some((
                        lp.hash.clone(),
                        lp.source.to_string(),
                        llm_config.model.clone(),
                    ));
                    audit.log_prompt_info(&lp.hash, &lp.source.to_string(), &llm_config.model);
                    Box::new(agent)
                }
                Err(e) => {
                    eprintln!("  WARN: LLM agent init failed: {e}, falling back to RSI");
                    Box::new(DummyIndicatorAgent::new())
                }
            },
            None => Box::new(DummyIndicatorAgent::new()),
        };
        let mut sigs = Vec::new();
        let mut syms: Vec<&String> = bars.keys().collect();
        syms.sort();
        for sym in syms {
            let start = std::time::Instant::now();
            let mut sig = indicator.generate_signal(&bars[sym], sym);
            let latency = start.elapsed().as_secs_f64() * 1000.0;
            sig.metadata.insert("latency_ms".into(), latency);
            sigs.push((sym.clone(), sig));
        }
        llm_cache_entries = indicator.take_cache_entries();
        sigs
    };

    // ── Generate targets (with optional blending) ─────────────────
    #[cfg(feature = "track-b")]
    let (snapshot, combined_results): (TargetSnapshot, Option<Vec<combiner::CombinedResult>>) =
        if app_config
            .blending
            .as_ref()
            .is_some_and(|b| b.enabled)
        {
            let blend_config = app_config.blending.as_ref().unwrap();
            eprintln!("  Blending mode: ENABLED");

            // Generate TSMOM signals (same logic as generate_targets)
            let mut tsmom_signals = HashMap::new();
            let mut tsmom_weights = HashMap::new();
            for (sym, series) in &bars {
                if series.bars().len() < args.min_history {
                    continue;
                }
                let sig = agent.generate_signal(series, sym);
                let weight = if sig.direction != SignalDirection::Flat {
                    TSMOMAgent::compute_target_weight(&sig)
                } else {
                    0.0
                };
                tsmom_weights.insert(sym.clone(), weight);
                tsmom_signals.insert(sym.clone(), sig);
            }

            // Build indicator signal map
            let indicator_map: HashMap<String, quantbot::core::signal::Signal> = indicator_signals
                .iter()
                .map(|(sym, sig)| (sym.clone(), sig.clone()))
                .collect();

            // Combine
            let results = combiner::combine_signals(&tsmom_signals, &indicator_map, blend_config);

            // Build override maps
            let mut combined_weights = HashMap::new();
            let mut combined_signals = HashMap::new();
            for r in &results {
                combined_weights.insert(r.instrument.clone(), r.combined_weight);
                let combined_sig = combiner::build_combined_signal(
                    r,
                    &tsmom_signals[&r.instrument],
                    indicator_map.get(&r.instrument),
                );
                combined_signals.insert(r.instrument.clone(), combined_sig);
            }

            let snap = bt_engine.generate_targets_with_overrides(
                combined_signals,
                combined_weights,
                &bars,
                &HashMap::new(),
                nav,
            );
            (snap, Some(results))
        } else {
            eprintln!("  Blending mode: disabled (TSMOM-only)");
            let snap =
                bt_engine.generate_targets(&agent, &bars, &HashMap::new(), nav, args.min_history);
            (snap, None)
        };

    #[cfg(not(feature = "track-b"))]
    let snapshot =
        bt_engine.generate_targets(&agent, &bars, &HashMap::new(), nav, args.min_history);

    // Log targets
    let target_entries: Vec<TargetEntry> = snapshot
        .target_quantities
        .iter()
        .map(|(sym, qty)| TargetEntry {
            instrument: sym.clone(),
            signed_deal_size: (*qty * 10.0).round() / 10.0,
            weight: snapshot.target_weights.get(sym).copied().unwrap_or(0.0),
        })
        .collect();
    audit.log_targets(&snapshot.date.to_string(), nav, &target_entries);

    // ── SQLite recording ────────────────────────────────────────
    let db_path = args.data_dir.join("quantbot.db");
    let (recorder, peak_nav) = match Db::open(&db_path) {
        Ok(db) => {
            // Read peak NAV for risk checks before creating recorder
            let peak = db.get_peak_nav().unwrap_or_else(|e| {
                eprintln!("  WARN: SQLite get_peak_nav failed: {e}");
                None
            });

            let config_json = serde_json::json!({
                "engine": engine_name,
                "dry_run": args.dry_run,
                "instruments": &symbols,
                "config_path": args.config.display().to_string(),
            })
            .to_string();
            let rec = Recorder::new(db, audit.run_id(), &config_json, nav);

            // Build signal records from TSMOM snapshot
            #[allow(unused_mut)]
            let mut signal_records: Vec<SignalRecord> = snapshot
                .signals
                .iter()
                .filter(|(_, sig)| sig.agent_name != "combined")
                .map(|(sym, sig)| SignalRecord {
                    instrument: sym.clone(),
                    agent_name: sig.agent_name.clone(),
                    direction: sig.direction,
                    strength: sig.strength,
                    confidence: sig.confidence,
                    weight: target_entries
                        .iter()
                        .find(|t| t.instrument == *sym)
                        .map(|t| t.weight)
                        .unwrap_or(0.0),
                })
                .collect();

            // When blending is active, snapshot.signals are "combined" — also record raw TSMOM
            #[cfg(feature = "track-b")]
            if let Some(ref results) = combined_results {
                // Record raw TSMOM signals (from the combiner results)
                for r in results {
                    // Raw tsmom weight is available in the combiner result
                    signal_records.push(SignalRecord {
                        instrument: r.instrument.clone(),
                        agent_name: "tsmom".into(),
                        direction: if r.tsmom_weight > 0.0 {
                            SignalDirection::Long
                        } else if r.tsmom_weight < 0.0 {
                            SignalDirection::Short
                        } else {
                            SignalDirection::Flat
                        },
                        strength: r.tsmom_weight.abs().min(1.0)
                            * if r.tsmom_weight < 0.0 { -1.0 } else { 1.0 },
                        confidence: 1.0, // TSMOM confidence is baked into weight
                        weight: r.tsmom_weight,
                    });
                }
            }

            // Add indicator signals (track-b only)
            #[cfg(feature = "track-b")]
            for (sym, sig) in &indicator_signals {
                signal_records.push(SignalRecord {
                    instrument: sym.clone(),
                    agent_name: sig.agent_name.clone(),
                    direction: sig.direction,
                    strength: sig.strength,
                    confidence: sig.confidence,
                    weight: 0.0, // advisory only
                });
            }

            // Add combined signals (track-b blending only)
            #[cfg(feature = "track-b")]
            if let Some(ref results) = combined_results {
                for r in results {
                    if let Some(sig) = snapshot.signals.get(&r.instrument) {
                        signal_records.push(SignalRecord {
                            instrument: r.instrument.clone(),
                            agent_name: "combined".into(),
                            direction: sig.direction,
                            strength: sig.strength,
                            confidence: sig.confidence,
                            weight: r.combined_weight,
                        });
                    }
                }
            }

            rec.record_signals(&signal_records);
            rec.record_target_positions(&target_entries);

            // Record prompt provenance (track-b only)
            #[cfg(feature = "track-b")]
            if let Some((ref hash, ref source, ref model)) = prompt_info {
                rec.record_prompt_info(hash, source, model);
            }

            // Record LLM cache entries (track-b only)
            #[cfg(feature = "track-b")]
            if !llm_cache_entries.is_empty() {
                rec.record_llm_cache_entries(&llm_cache_entries);
            }

            (Some(rec), peak)
        }
        Err(e) => {
            eprintln!("  WARN: failed to open SQLite database {}: {e}", db_path.display());
            (None, None)
        }
    };

    // ── Risk check (hard veto) ───────────────────────────────────
    if let Some(risk_config) = &app_config.risk {
        let risk_agent = RiskAgent::new(risk_config.clone());
        let effective_peak = peak_nav.unwrap_or(nav).max(nav);

        let (decision, detail) = risk_agent.check_all(&snapshot.orders, nav, effective_peak);
        audit.log_risk_check(&detail);

        eprintln!(
            "  Risk check: leverage={:.2} max_pos={:.1}% drawdown={:.1}% → {}",
            detail.gross_leverage,
            detail.max_position_leverage * 100.0,
            detail.drawdown_pct * 100.0,
            detail.decision,
        );

        if let RiskDecision::Veto { reason } = decision {
            eprintln!("  RISK VETO: {reason}");

            // Record veto outcome
            if let Some(rec) = recorder.as_ref() {
                rec.record_run_end("RISK_VETO", 0);
            }

            let summary = RunSummary {
                run_id: audit.run_id().to_string(),
                outcome: "RISK_VETO".to_string(),
                duration_ms: 0,
                orders_placed: 0,
                orders_confirmed: 0,
                orders_rejected: 0,
                dust_skipped: 0,
                mismatches: 0,
                audit_write_failed: audit.write_failed,
                db_write_failed: recorder.as_ref().is_some_and(|r| r.write_failed()),
                audit_path: audit.path().display().to_string(),
            };
            audit.log_run_end("RISK_VETO", &summary);

            if args.json {
                if let Ok(json) = serde_json::to_string_pretty(&summary) {
                    println!("{json}");
                }
            } else {
                eprintln!("  {summary}");
            }

            bail!("risk veto: {reason}");
        }

        // Update peak NAV (only if we passed the check)
        if let Ok(db) = Db::open(&db_path) {
            if let Err(e) = db.update_peak_nav(effective_peak) {
                eprintln!("  WARN: SQLite update_peak_nav failed: {e}");
            }
        }
    }

    // ── Construct engine and run rebalance ───────────────────
    // Print indicator signals before rebalance (they appear after TSMOM in the report)
    #[cfg(feature = "track-b")]
    {
        println!();
        println!("  INDICATOR SIGNALS (advisory)");
        println!(
            "  {:<14} {:<15} {:<10} {:>8} {:>10} {:>6}",
            "Instrument", "Agent", "Direction", "Strength", "Confidence", "RSI"
        );
        for (sym, sig) in &indicator_signals {
            let dir_str = match sig.direction {
                SignalDirection::Long => "Long",
                SignalDirection::Short => "Short",
                SignalDirection::Flat => "Flat",
            };
            let rsi_str = match sig.metadata.get("rsi") {
                Some(rsi) => format!("{:.1}", rsi),
                None => "-".into(),
            };
            println!(
                "  {:<14} {:<15} {:<10} {:>+8.2} {:>10.2} {:>6}",
                sym, sig.agent_name, dir_str, sig.strength, sig.confidence, rsi_str,
            );
        }
        println!();

        // Print blending summary if enabled
        if let Some(ref results) = combined_results {
            println!("  BLENDED WEIGHTS");
            println!(
                "  {:<14} {:<10} {:>8} {:>10} {:>10} {:>6}",
                "Instrument", "Category", "TSMOM", "Indicator", "Combined", "Used?"
            );
            for r in results {
                let used_str = if r.indicator_used {
                    "yes"
                } else if r.blend_indicator == 0.0 {
                    "no (100% tsmom)"
                } else {
                    "no (fallback)"
                };
                println!(
                    "  {:<14} {:<10} {:>+8.2} {:>+10.2} {:>+10.2} {:>6}",
                    r.instrument, r.blend_category, r.tsmom_weight, r.indicator_weight,
                    r.combined_weight, used_str,
                );
            }
            println!();
        }
    }

    let result = match app_config.execution.engine {
        EngineType::Paper => {
            let engine = PaperExecutionEngine::new();
            run_rebalance(&engine, &snapshot, &symbols, ig_config, &args, nav, engine_name, &mut audit, recorder.as_ref()).await
        }
        EngineType::Ig => {
            // Reuse IG engine created for MTM (already authenticated)
            let engine = ig_engine.expect("IG engine already created for MTM");
            run_rebalance(&engine, &snapshot, &symbols, ig_config, &args, nav, engine_name, &mut audit, recorder.as_ref()).await
        }
    };

    // run_rebalance logs run_end internally; propagate errors after
    result
}

#[allow(clippy::too_many_arguments)]
async fn run_rebalance(
    engine: &impl ExecutionEngine,
    snapshot: &TargetSnapshot,
    symbols: &[String],
    ig_config: &quantbot::config::IgConfig,
    args: &LiveArgs,
    nav: f64,
    engine_name: &str,
    audit: &mut AuditLogger,
    recorder: Option<&Recorder>,
) -> Result<()> {
    let start_time = std::time::Instant::now();
    let mut orders_placed: usize = 0;
    let mut orders_confirmed: usize = 0;
    let mut orders_rejected: usize = 0;

    // Helper: log run_end + optional --json summary and return result
    let finish = |audit: &mut AuditLogger,
                  outcome: &str,
                  orders_placed: usize,
                  orders_confirmed: usize,
                  orders_rejected: usize,
                  dust_skipped: usize,
                  mismatches: usize,
                  start_time: std::time::Instant,
                  json_flag: bool,
                  recorder: Option<&Recorder>| {
        let summary = RunSummary {
            run_id: audit.run_id().to_string(),
            outcome: outcome.to_string(),
            duration_ms: start_time.elapsed().as_millis() as u64,
            orders_placed,
            orders_confirmed,
            orders_rejected,
            dust_skipped,
            mismatches,
            audit_write_failed: audit.write_failed,
            db_write_failed: recorder.is_some_and(|r| r.write_failed()),
            audit_path: audit.path().display().to_string(),
        };
        audit.log_run_end(outcome, &summary);

        if let Some(rec) = recorder {
            rec.record_run_end(outcome, summary.duration_ms);
        }

        if json_flag {
            if let Ok(json) = serde_json::to_string_pretty(&summary) {
                println!("{json}");
            }
        } else {
            eprintln!("  {summary}");
        }
    };

    eprintln!("  Authenticating...");
    engine.health_check().await.context("health check failed")?;
    audit.log_auth_ok(engine_name);
    audit.log_health_check_ok();
    eprintln!("  Health check passed");

    // ── Fetch actual positions and reconcile ────────────────────
    let live_positions = engine
        .get_positions()
        .await
        .context("failed to fetch positions")?;
    let actual_signed = reconcile::positions_to_signed(&live_positions);

    eprintln!("  Live positions: {} instrument(s)", actual_signed.len());
    for (sym, qty) in &actual_signed {
        eprintln!("    {}: {:.1}", sym, qty);
    }

    audit.log_positions_fetched(&audit::positions_to_entries(&actual_signed));

    if let Some(rec) = recorder {
        rec.record_actual_positions(&actual_signed);
    }

    // Filter target quantities to only requested instruments
    let target_quantities: HashMap<String, f64> = snapshot
        .target_quantities
        .iter()
        .filter(|(sym, _)| symbols.contains(sym))
        .map(|(k, v)| (k.clone(), *v))
        .collect();

    let reconcile_result = reconcile::compute_deltas(&target_quantities, &actual_signed, ig_config);

    // ── Fail on unknown instruments (missing config = missing leg) ──
    if !reconcile_result.unknown_instruments.is_empty() {
        let msg = format!(
            "unknown instruments with no epic mapping: [{}]",
            reconcile_result.unknown_instruments.join(", ")
        );
        audit.log_error(&msg);
        finish(
            audit, "ERROR", 0, 0, 0, 0, 0, start_time, args.json, recorder,
        );
        bail!("{msg} — fix config or remove from targets");
    }

    let dust_skipped = reconcile_result.skipped_dust.len();

    // ── Report dust deltas ─────────────────────────────────────
    if !reconcile_result.skipped_dust.is_empty() {
        eprintln!(
            "  Dust: {} instrument(s) with sub-minimum deltas (tracked, not traded):",
            reconcile_result.skipped_dust.len()
        );
        for dust in &reconcile_result.skipped_dust {
            eprintln!(
                "    {}: target={:.1}, actual={:.1}, delta={:.4}",
                dust.instrument, dust.target, dust.actual, dust.delta
            );
        }
    }

    let mut delta_orders = reconcile_result.orders;

    eprintln!(
        "  Reconciliation: {} orders, {} dust",
        delta_orders.len(),
        reconcile_result.skipped_dust.len(),
    );

    audit.log_reconcile(
        &audit::order_requests_to_entries(&delta_orders),
        &reconcile_result.skipped_dust,
    );

    // ── Apply safety valves ────────────────────────────────────
    if let Some(max_size) = args.max_size {
        let before = delta_orders.len();
        delta_orders.retain(|o| o.size <= max_size);
        let filtered = before - delta_orders.len();
        if filtered > 0 {
            eprintln!("  Safety: filtered {filtered} orders exceeding --max-size {max_size}");
        }
    }

    if delta_orders.len() > args.max_orders {
        eprintln!(
            "  Safety: truncating {} orders to --max-orders {}",
            delta_orders.len(),
            args.max_orders
        );
        delta_orders.truncate(args.max_orders);
    }

    // ── Circuit breaker check ──────────────────────────────────
    let mut breaker = CircuitBreaker::default();
    if let Some(max_size) = args.max_size {
        breaker = breaker.with_max_order_size(max_size);
    }

    let max_order_size = delta_orders.iter().map(|o| o.size).fold(0.0_f64, f64::max);
    if let Err(reason) = breaker.check_orders(delta_orders.len(), max_order_size) {
        eprintln!("  CIRCUIT BREAKER: {reason}");
        audit.log_breaker_check(false, delta_orders.len(), max_order_size, Some(&reason));

        eprintln!("  Attempting to flatten all positions...");
        if let Err(e) = engine.flatten_all().await {
            eprintln!("  Flatten failed: {e}");
        }
        finish(
            audit, "BREAKER_TRIPPED", 0, 0, 0, dust_skipped, 0, start_time, args.json, recorder,
        );
        bail!("circuit breaker tripped: {reason}");
    }

    audit.log_breaker_check(true, delta_orders.len(), max_order_size, None);

    // ── Print report ───────────────────────────────────────────
    print_live_report(snapshot, nav, &delta_orders);

    if args.dry_run {
        eprintln!("  --dry-run: no orders placed");
        audit.log_execution_skipped("dry_run", delta_orders.len());
        finish(
            audit, "DRY_RUN", 0, 0, 0, dust_skipped, 0, start_time, args.json, recorder,
        );
        return Ok(());
    }

    if delta_orders.is_empty() {
        eprintln!("  No orders to place (already at target)");
        audit.log_execution_skipped("already_at_target", 0);
    } else {
        // ── Execute ────────────────────────────────────────────
        orders_placed = delta_orders.len();
        audit.log_orders_submitted(&audit::order_requests_to_entries(&delta_orders));
        if let Some(rec) = recorder {
            rec.record_orders_submitted(&delta_orders);
        }

        let acks = engine
            .place_orders(delta_orders.clone())
            .await
            .context("failed to place orders")?;

        for ack in &acks {
            eprintln!(
                "  {} {} → {:?}",
                ack.deal_reference, ack.instrument, ack.status
            );
        }

        let ack_entries = audit::order_acks_to_entries(&acks);
        orders_confirmed = ack_entries
            .iter()
            .filter(|a| a.status == "Accepted")
            .count();
        orders_rejected = ack_entries
            .iter()
            .filter(|a| a.status == "Rejected")
            .count();

        audit.log_orders_confirmed(&ack_entries);
        if let Some(rec) = recorder {
            rec.record_orders_confirmed(&acks);
        }
        breaker.record_success();
    }

    // ── Post-trade verification ────────────────────────────────
    let post_positions = engine
        .get_positions()
        .await
        .context("post-trade position fetch failed")?;
    let post_signed = reconcile::positions_to_signed(&post_positions);
    let mismatches = reconcile::verify_positions(&target_quantities, &post_signed, ig_config);

    let mismatch_count = mismatches.len();

    if mismatches.is_empty() {
        eprintln!("  Post-trade verification: all positions match targets");
    } else {
        eprintln!(
            "  Post-trade verification: {} mismatch(es)",
            mismatches.len()
        );
        for m in &mismatches {
            eprintln!(
                "    {}: target={:.1}, actual={:.1}, delta={:.1}",
                m.instrument, m.target, m.actual, m.delta
            );
        }
    }

    audit.log_verify(mismatches.is_empty(), &audit::mismatches_to_entries(&mismatches));

    if let Some(rec) = recorder {
        rec.record_post_trade_positions(&post_signed);
    }

    // ── Save state with actual quantities ──────────────────────
    save_state(
        &args.state_file,
        &PaperTradeState {
            date: snapshot.date,
            nav,
            quantities: post_signed,
        },
    )?;
    eprintln!("  State saved to {}", args.state_file.display());

    // ── Final summary ──────────────────────────────────────────
    let outcome = if orders_rejected > 0 {
        "PARTIAL"
    } else {
        "SUCCESS"
    };
    finish(
        audit,
        outcome,
        orders_placed,
        orders_confirmed,
        orders_rejected,
        dust_skipped,
        mismatch_count,
        start_time,
        args.json,
        recorder,
    );

    Ok(())
}

async fn run_flatten(app_config: &AppConfig) -> Result<()> {
    match app_config.execution.engine {
        EngineType::Paper => {
            eprintln!("  Paper engine: nothing to flatten");
        }
        EngineType::Ig => {
            let ig_config = app_config
                .execution
                .ig
                .as_ref()
                .context("IG config required for engine=ig")?;
            let engine = IgExecutionEngine::new(ig_config)
                .map_err(|e| anyhow::anyhow!("failed to create IG engine: {e}"))?;

            eprintln!("  Authenticating with IG...");
            engine
                .health_check()
                .await
                .context("IG health check failed")?;

            eprintln!("  Flattening all positions...");
            engine.flatten_all().await.context("failed to flatten")?;
            eprintln!("  All positions closed");
        }
    }
    Ok(())
}

async fn run_positions(args: PositionsArgs) -> Result<()> {
    let app_config = AppConfig::load(&args.config)
        .with_context(|| format!("failed to load config from {}", args.config.display()))?;

    let ig_config = match (&app_config.execution.engine, &app_config.execution.ig) {
        (EngineType::Ig, Some(ig)) => ig,
        _ => bail!("positions command requires engine = \"ig\" with IG config"),
    };

    let engine = IgExecutionEngine::new(ig_config)
        .map_err(|e| anyhow::anyhow!("failed to create IG engine: {e}"))?;

    engine
        .health_check()
        .await
        .context("IG health check failed")?;

    let positions = engine
        .get_positions()
        .await
        .context("failed to fetch positions")?;

    if args.json {
        let json =
            serde_json::to_string_pretty(&positions).context("failed to serialize positions")?;
        println!("{json}");
        return Ok(());
    }

    if positions.is_empty() {
        println!("  0 open positions");
        return Ok(());
    }

    println!(
        "  {:<14} {:<6} {:>10} {:>10} {:<30}",
        "Instrument", "Side", "Size", "Level", "Epic"
    );
    println!("  {}", "─".repeat(74));

    for pos in &positions {
        let side_str = match pos.direction {
            OrderSide::Buy => "BUY",
            OrderSide::Sell => "SELL",
        };
        println!(
            "  {:<14} {:<6} {:>10.1} {:>10.4} {:<30}",
            pos.instrument, side_str, pos.size, pos.open_level, pos.epic,
        );
    }

    println!("  {}", "─".repeat(74));
    println!("  {} open position(s)", positions.len());

    Ok(())
}

fn print_live_report(snap: &TargetSnapshot, nav: f64, orders: &[OrderRequest]) {
    println!("==================================================");
    println!("  LIVE TRADE TARGETS — {}", snap.date);
    println!("  NAV: ${}", format_number(nav));
    println!("==================================================");
    println!();

    // Signals table
    println!("  SIGNALS");
    println!(
        "  {:<14} {:<10} {:>8} {:>10} {:>8}",
        "Instrument", "Direction", "Strength", "Confidence", "Weight"
    );

    let mut sig_syms: Vec<&String> = snap.signals.keys().collect();
    sig_syms.sort();
    for sym in sig_syms {
        let sig = &snap.signals[sym];
        let dir_str = match sig.direction {
            SignalDirection::Long => "Long",
            SignalDirection::Short => "Short",
            SignalDirection::Flat => "Flat",
        };
        let weight = snap.target_weights.get(sym).copied().unwrap_or(0.0);
        println!(
            "  {:<14} {:<10} {:>+8.2} {:>10.2} {:>+8.2}",
            sym, dir_str, sig.strength, sig.confidence, weight,
        );
    }
    println!();

    if orders.is_empty() {
        println!("  NO ORDERS (already at target)");
    } else {
        println!("  ORDERS TO EXECUTE ({} orders)", orders.len());
        println!(
            "  {:<14} {:<6} {:>10} {:<30}",
            "Instrument", "Side", "Size", "Epic"
        );
        for order in orders {
            let side_str = match order.direction {
                OrderSide::Buy => "BUY",
                OrderSide::Sell => "SELL",
            };
            println!(
                "  {:<14} {:<6} {:>10.1} {:<30}",
                order.instrument, side_str, order.size, order.epic,
            );
        }
    }

    println!("==================================================");
}

fn print_paper_trade_report(snap: &TargetSnapshot, nav: f64, prior: Option<&PaperTradeState>) {
    println!("==================================================");
    println!("  PAPER TRADE REPORT — {}", snap.date);
    println!("  NAV: ${}", format_number(nav));
    match prior {
        Some(state) => println!(
            "  Previous state: {} ({} positions)",
            state.date,
            state.quantities.len()
        ),
        None => println!("  Previous state: flat (no state file)"),
    }
    println!("==================================================");
    println!();

    // Signals table
    println!("  SIGNALS");
    println!(
        "  {:<14} {:<10} {:>8} {:>10} {:>8} {:>8}",
        "Instrument", "Direction", "Strength", "Confidence", "AnnVol", "Weight"
    );

    let mut sig_syms: Vec<&String> = snap.signals.keys().collect();
    sig_syms.sort();
    for sym in sig_syms {
        let sig = &snap.signals[sym];
        let dir_str = match sig.direction {
            SignalDirection::Long => "Long",
            SignalDirection::Short => "Short",
            SignalDirection::Flat => "Flat",
        };
        let ann_vol = sig.metadata.get("ann_vol").copied().unwrap_or(0.0);
        let weight = snap.target_weights.get(sym).copied().unwrap_or(0.0);
        println!(
            "  {:<14} {:<10} {:>+8.2} {:>10.2} {:>7.1}% {:>+8.2}",
            sym,
            dir_str,
            sig.strength,
            sig.confidence,
            ann_vol * 100.0,
            weight,
        );
    }

    println!();

    // Orders table
    if snap.orders.is_empty() {
        println!("  NO ORDERS (already at target)");
    } else {
        match prior {
            Some(state) => println!("  ORDERS (rebalance from {})", state.date),
            None => println!("  ORDERS (from flat)"),
        }
        println!(
            "  {:<14} {:<6} {:>10} {:>10} {:>12} {:>10} {:>10}",
            "Instrument", "Side", "Qty", "Price", "Notional", "Margin", "Spread"
        );

        for order in &snap.orders {
            print_order_row(order);
        }

        println!("  {}", "─".repeat(74));
        let margin_pct = if nav > 0.0 {
            snap.total_margin / nav * 100.0
        } else {
            0.0
        };
        println!(
            "  Total Margin: ${} ({:.1}% of NAV)",
            format_number(snap.total_margin),
            margin_pct
        );
    }

    println!("==================================================");
}

fn run_history(args: HistoryArgs) -> Result<()> {
    let db = Db::open(&args.db)
        .map_err(|e| anyhow::anyhow!("failed to open database {}: {e}", args.db.display()))?;

    if let Some(run_id) = &args.run {
        // Show details for a specific run
        let orders = db
            .orders_for_run_filtered(run_id, args.status.as_deref())
            .map_err(|e| anyhow::anyhow!("query failed: {e}"))?;
        let signals = db
            .signals_for_run(run_id)
            .map_err(|e| anyhow::anyhow!("query failed: {e}"))?;

        if args.json {
            let output = serde_json::json!({
                "run_id": run_id,
                "signals": signals,
                "orders": orders,
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
        } else {
            if !signals.is_empty() {
                eprintln!("  SIGNALS for {run_id}");
                eprintln!(
                    "  {:<15} {:>10} {:>10} {:>10} {:>10} {:>10}",
                    "Instrument", "Agent", "Direction", "Strength", "Confidence", "Weight"
                );
                for s in &signals {
                    eprintln!(
                        "  {:<15} {:>10} {:>10} {:>+10.2} {:>10.2} {:>+10.2}",
                        s.instrument, s.agent_name, s.direction, s.strength, s.confidence, s.weight
                    );
                }
            }
            if !orders.is_empty() {
                eprintln!();
                eprintln!("  ORDERS for {run_id}");
                eprintln!(
                    "  {:<15} {:>10} {:>10} {:>15} {:>10}",
                    "Instrument", "Direction", "Size", "Deal Ref", "Status"
                );
                for o in &orders {
                    eprintln!(
                        "  {:<15} {:>10} {:>10.1} {:>15} {:>10}",
                        o.instrument,
                        o.direction,
                        o.size,
                        o.deal_reference.as_deref().unwrap_or("-"),
                        o.status.as_deref().unwrap_or("-"),
                    );
                }
            }
            if signals.is_empty() && orders.is_empty() {
                eprintln!("  No data found for run {run_id}");
            }
        }
    } else if let Some(instrument) = &args.instrument {
        // Show orders for a specific instrument
        let orders = db
            .orders_for_instrument(instrument, args.last)
            .map_err(|e| anyhow::anyhow!("query failed: {e}"))?;

        if args.json {
            println!("{}", serde_json::to_string_pretty(&orders)?);
        } else {
            if orders.is_empty() {
                eprintln!("  No orders found for {instrument}");
            } else {
                eprintln!("  ORDERS for {instrument} (last {})", args.last);
                eprintln!(
                    "  {:>10} {:>10} {:>15} {:>10} {:>24}",
                    "Direction", "Size", "Deal Ref", "Status", "Time"
                );
                for o in &orders {
                    eprintln!(
                        "  {:>10} {:>10.1} {:>15} {:>10} {:>24}",
                        o.direction,
                        o.size,
                        o.deal_reference.as_deref().unwrap_or("-"),
                        o.status.as_deref().unwrap_or("-"),
                        o.ts,
                    );
                }
            }
        }
    } else {
        // List recent runs, optionally filtered by date
        let runs = if let Some(date) = &args.date {
            db.list_runs_by_date(date, args.last)
                .map_err(|e| anyhow::anyhow!("query failed: {e}"))?
        } else {
            db.list_runs(args.last)
                .map_err(|e| anyhow::anyhow!("query failed: {e}"))?
        };

        if args.json {
            println!("{}", serde_json::to_string_pretty(&runs)?);
        } else {
            if runs.is_empty() {
                eprintln!("  No runs found in {}", args.db.display());
            } else {
                eprintln!("  RECENT RUNS (last {})", args.last);
                eprintln!(
                    "  {:<22} {:>12} {:>12} {:>10}",
                    "Run ID", "NAV", "Outcome", "Duration"
                );
                for r in &runs {
                    let duration = r
                        .duration_ms
                        .map(|ms| format!("{:.1}s", ms as f64 / 1000.0))
                        .unwrap_or_else(|| "-".into());
                    eprintln!(
                        "  {:<22} {:>12} {:>12} {:>10}",
                        r.run_id,
                        format!("${}", format_number(r.nav_usd)),
                        r.outcome.as_deref().unwrap_or("IN_PROGRESS"),
                        duration,
                    );
                }
            }
        }
    }

    Ok(())
}

/// Format a number with thousands separators (e.g., 1234567 → "1,234,567").
fn format_number(n: f64) -> String {
    let rounded = n.round() as i64;
    let negative = rounded < 0;
    let s = rounded.unsigned_abs().to_string();
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    let formatted: String = result.chars().rev().collect();
    if negative {
        format!("-{formatted}")
    } else {
        formatted
    }
}

fn print_order_row(order: &SizedOrder) {
    let side_str = match order.side {
        OrderSide::Buy => "BUY",
        OrderSide::Sell => "SELL",
    };
    println!(
        "  {:<14} {:<6} {:>10.1} {:>10.2} {:>12} {:>10} {:>10.2}",
        order.instrument,
        side_str,
        order.quantity,
        order.reference_price,
        format_number(order.notional),
        format_number(order.margin_required),
        order.spread_cost,
    );
}

async fn run_data(args: DataArgs) -> Result<()> {
    let symbols: Vec<String> = if let Some(syms) = args.instruments {
        syms
    } else if args.tradeable_only {
        TRADEABLE_UNIVERSE
            .iter()
            .map(|i| i.symbol.clone())
            .collect()
    } else {
        // Discover from existing CSVs, fall back to tradeable universe
        let updater = DataUpdater::new(&args.data_dir);
        let discovered = updater.discover_symbols().unwrap_or_default();
        if discovered.is_empty() {
            TRADEABLE_UNIVERSE
                .iter()
                .map(|i| i.symbol.clone())
                .collect()
        } else {
            discovered
        }
    };

    let today = chrono::Utc::now().date_naive();
    eprintln!(
        "  Updating {} symbol(s) to {}...",
        symbols.len(),
        today
    );

    let updater = DataUpdater::new(&args.data_dir);
    let mut client = YahooClient::new();
    let results = updater.update_all(&mut client, &symbols, today).await;

    let mut any_error = false;
    if args.json {
        let json_results: Vec<serde_json::Value> = results
            .iter()
            .map(|r| {
                serde_json::json!({
                    "symbol": r.symbol,
                    "bars_fetched": r.bars_fetched,
                    "bars_appended": r.bars_appended,
                    "total_bars": r.total_bars,
                    "error": r.error,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&json_results)?);
    } else {
        eprintln!(
            "  {:<15} {:>10} {:>10} {:>10} Status",
            "Symbol", "Fetched", "Appended", "Total"
        );
        for r in &results {
            if let Some(err) = &r.error {
                eprintln!(
                    "  {:<15} {:>10} {:>10} {:>10} ERROR: {}",
                    r.symbol, r.bars_fetched, r.bars_appended, r.total_bars, err
                );
                any_error = true;
            } else {
                eprintln!(
                    "  {:<15} {:>10} {:>10} {:>10} OK",
                    r.symbol, r.bars_fetched, r.bars_appended, r.total_bars
                );
            }
        }
    }

    if any_error {
        bail!("Some symbols failed to update — see errors above");
    }

    Ok(())
}

#[cfg(all(test, feature = "track-b"))]
mod attribution_tests {
    use super::*;
    use chrono::{NaiveDate, Utc};
    use quantbot::backtest::engine::Snapshot;
    use quantbot::core::portfolio::{Fill, Order, OrderSide};
    use quantbot::core::signal::{Signal, SignalDirection, SignalType};

    fn make_signal(instrument: &str, indicator_used: Option<f64>) -> Signal {
        let mut metadata = HashMap::new();
        if let Some(v) = indicator_used {
            metadata.insert("indicator_used".to_string(), v);
        }
        Signal {
            instrument: instrument.to_string(),
            direction: SignalDirection::Long,
            strength: 0.5,
            confidence: 0.8,
            agent_name: "combined".to_string(),
            signal_type: SignalType::Combined,
            horizon_days: 21,
            timestamp: Utc::now(),
            metadata,
        }
    }

    fn make_fill(instrument: &str) -> Fill {
        Fill {
            order: Order::new(instrument.to_string(), OrderSide::Buy, 1.0),
            fill_price: 100.0,
            fill_quantity: 1.0,
            timestamp: Utc::now(),
            slippage_bps: 0.0,
        }
    }

    fn make_snapshot(
        day: u32,
        notionals: &[(&str, f64)],
        signals: &[(&str, Option<f64>)],
        fills: &[&str],
    ) -> Snapshot {
        Snapshot {
            date: NaiveDate::from_ymd_opt(2025, 1, day).unwrap(),
            nav: 1_000_000.0,
            cash: 500_000.0,
            gross_exposure: 0.0,
            net_exposure: 0.0,
            positions: notionals
                .iter()
                .map(|(s, _)| (s.to_string(), 1.0))
                .collect(),
            position_notionals: notionals
                .iter()
                .map(|(s, v)| (s.to_string(), *v))
                .collect(),
            signals: signals
                .iter()
                .map(|(s, ind)| (s.to_string(), make_signal(s, *ind)))
                .collect(),
            fills: fills.iter().map(|s| make_fill(s)).collect(),
        }
    }

    #[test]
    fn test_pnl_from_notional_changes() {
        let snapshots = vec![
            make_snapshot(1, &[("SPY", 100_000.0)], &[("SPY", None)], &["SPY"]),
            make_snapshot(2, &[("SPY", 105_000.0)], &[("SPY", None)], &[]),
            make_snapshot(3, &[("SPY", 102_000.0)], &[("SPY", None)], &[]),
        ];

        let attr = compute_attribution(&snapshots);
        let spy = &attr["SPY"];

        // PnL = (105k - 100k) + (102k - 105k) = 5000 - 3000 = 2000
        assert!((spy.pnl_usd - 2_000.0).abs() < 1.0);
        assert_eq!(spy.trade_count, 1);
        assert_eq!(spy.total_days, 3);
    }

    #[test]
    fn test_indicator_used_tracking() {
        let snapshots = vec![
            make_snapshot(1, &[("GLD", 50_000.0)], &[("GLD", Some(1.0))], &[]),
            make_snapshot(2, &[("GLD", 52_000.0)], &[("GLD", Some(0.0))], &[]),
            make_snapshot(3, &[("GLD", 54_000.0)], &[("GLD", Some(1.0))], &[]),
        ];

        let attr = compute_attribution(&snapshots);
        let gld = &attr["GLD"];

        assert_eq!(gld.indicator_used_days, 2);
        assert_eq!(gld.total_days, 3);
    }

    #[test]
    fn test_multiple_instruments() {
        let snapshots = vec![
            make_snapshot(
                1,
                &[("SPY", 100_000.0), ("GLD", 50_000.0)],
                &[("SPY", None), ("GLD", Some(1.0))],
                &["SPY", "GLD"],
            ),
            make_snapshot(
                2,
                &[("SPY", 110_000.0), ("GLD", 48_000.0)],
                &[("SPY", None), ("GLD", Some(1.0))],
                &[],
            ),
        ];

        let attr = compute_attribution(&snapshots);

        assert!((attr["SPY"].pnl_usd - 10_000.0).abs() < 1.0);
        assert!((attr["GLD"].pnl_usd - (-2_000.0)).abs() < 1.0);
        assert_eq!(attr["SPY"].trade_count, 1);
        assert_eq!(attr["GLD"].trade_count, 1);
        assert_eq!(attr["GLD"].indicator_used_days, 2);
    }

    #[test]
    fn test_instrument_appears_only_in_signals() {
        // Instrument only in signals, not in positions
        let snapshots = vec![
            make_snapshot(1, &[], &[("USDJPY=X", Some(0.0))], &[]),
        ];

        let attr = compute_attribution(&snapshots);
        let jpy = &attr["USDJPY=X"];

        assert!((jpy.pnl_usd).abs() < 1e-10);
        assert_eq!(jpy.total_days, 1);
        assert_eq!(jpy.indicator_used_days, 0);
    }

    #[test]
    fn test_empty_snapshots() {
        let attr = compute_attribution(&[]);
        assert!(attr.is_empty());
    }
}

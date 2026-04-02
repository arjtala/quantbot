use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process;

use anyhow::{bail, Context, Result};
use chrono::NaiveDate;
use clap::{Parser, Subcommand};

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
use quantbot::data::loader::CsvLoader;
use quantbot::db::Db;
use quantbot::execution::circuit_breaker::CircuitBreaker;
use quantbot::execution::ig::engine::IgExecutionEngine;
use quantbot::execution::paper::PaperExecutionEngine;
use quantbot::execution::reconcile;
use quantbot::execution::router::SizedOrder;
use quantbot::execution::traits::{ExecutionEngine, OrderRequest};
use quantbot::recording::Recorder;

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

    /// Number of recent runs to show (default: 10)
    #[arg(long, default_value_t = 10)]
    last: usize,

    /// Output as JSON
    #[arg(long)]
    json: bool,
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
}

#[derive(Parser)]
struct PaperTradeArgs {
    /// Comma-separated instrument symbols (default: tradeable universe)
    #[arg(long, value_delimiter = ',')]
    instruments: Option<Vec<String>>,

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

    // ── Generate targets ───────────────────────────────────────
    // Use prior state for NAV, but actual positions come from IG
    let nav = match load_state(&args.state_file)? {
        Some(state) => {
            eprintln!("  Loaded NAV from state: ${}", format_number(state.nav));
            state.nav
        }
        None => args.initial_cash,
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

    // For target generation, use empty quantities (we'll reconcile against live positions)
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
    let recorder = match Db::open(&db_path) {
        Ok(db) => {
            let config_json = serde_json::json!({
                "engine": engine_name,
                "dry_run": args.dry_run,
                "instruments": &symbols,
                "config_path": args.config.display().to_string(),
            })
            .to_string();
            let rec = Recorder::new(db, audit.run_id(), &config_json, nav);

            // Record signals from snapshot
            let signal_map: HashMap<String, (SignalDirection, f64, f64)> = snapshot
                .signals
                .iter()
                .map(|(sym, sig)| (sym.clone(), (sig.direction, sig.strength, sig.confidence)))
                .collect();
            rec.record_signals(&target_entries, &signal_map);
            rec.record_target_positions(&target_entries);

            Some(rec)
        }
        Err(e) => {
            eprintln!("  WARN: failed to open SQLite database {}: {e}", db_path.display());
            None
        }
    };

    // ── Construct engine and run rebalance ───────────────────
    let result = match app_config.execution.engine {
        EngineType::Paper => {
            let engine = PaperExecutionEngine::new();
            run_rebalance(&engine, &snapshot, &symbols, ig_config, &args, nav, engine_name, &mut audit, recorder.as_ref()).await
        }
        EngineType::Ig => {
            let engine = IgExecutionEngine::new(ig_config)
                .map_err(|e| anyhow::anyhow!("failed to create IG engine: {e}"))?;
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
            .orders_for_run(run_id)
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
                    "  {:<15} {:>10} {:>10} {:>10} {:>10}",
                    "Instrument", "Direction", "Strength", "Confidence", "Weight"
                );
                for s in &signals {
                    eprintln!(
                        "  {:<15} {:>10} {:>+10.2} {:>10.2} {:>+10.2}",
                        s.instrument, s.direction, s.strength, s.confidence, s.weight
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
        // List recent runs
        let runs = db
            .list_runs(args.last)
            .map_err(|e| anyhow::anyhow!("query failed: {e}"))?;

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

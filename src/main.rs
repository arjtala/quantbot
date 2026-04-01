use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process;

use anyhow::{bail, Context, Result};
use chrono::NaiveDate;
use clap::{Parser, Subcommand};

use quantbot::agents::tsmom::TSMOMAgent;
use quantbot::backtest::engine::{BacktestConfig, BacktestEngine, TargetSnapshot};
use quantbot::backtest::metrics::BacktestResult;
use quantbot::core::signal::SignalDirection;
use quantbot::core::universe::TRADEABLE_UNIVERSE;
use quantbot::data::loader::CsvLoader;
use quantbot::execution::router::SizedOrder;

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
    /// Live trade with IG execution (not yet implemented)
    Live(StubArgs),
    /// Show current positions (not yet implemented)
    Positions,
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
struct StubArgs {
    /// Path to configuration file
    #[arg(long)]
    config: Option<PathBuf>,

    /// Dry run (no actual trades)
    #[arg(long)]
    dry_run: bool,
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
    let contents =
        std::fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
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

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Command::Backtest(args) => run_backtest(args),
        Command::PaperTrade(args) => run_paper_trade(args),
        Command::Live(_) => {
            eprintln!("live: not yet implemented");
            process::exit(1);
        }
        Command::Positions => {
            eprintln!("positions: not yet implemented");
            process::exit(1);
        }
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
        None => TRADEABLE_UNIVERSE.iter().map(|i| i.symbol.clone()).collect(),
    };

    // Validate data directory
    if !args.data_dir.is_dir() {
        bail!(
            "Data directory does not exist: {}",
            args.data_dir.display()
        );
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
        None => TRADEABLE_UNIVERSE.iter().map(|i| i.symbol.clone()).collect(),
    };

    if !args.data_dir.is_dir() {
        bail!(
            "Data directory does not exist: {}",
            args.data_dir.display()
        );
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

    let snapshot = engine.generate_targets(
        &agent,
        &bars,
        &current_quantities,
        nav,
        args.min_history,
    );

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
        let json = serde_json::to_string_pretty(&snapshot)
            .context("Failed to serialize targets")?;
        println!("{json}");
    } else {
        print_paper_trade_report(&snapshot, nav, prior_state.as_ref());
    }

    Ok(())
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
            format_number(snap.total_margin), margin_pct
        );
    }

    println!("==================================================");
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
        quantbot::core::portfolio::OrderSide::Buy => "BUY",
        quantbot::core::portfolio::OrderSide::Sell => "SELL",
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
use std::collections::HashMap;
use std::path::PathBuf;
use std::process;

use anyhow::{bail, Context, Result};
use chrono::NaiveDate;
use clap::{Parser, Subcommand};

use quantbot::agents::tsmom::TSMOMAgent;
use quantbot::backtest::engine::{BacktestConfig, BacktestEngine};
use quantbot::backtest::metrics::BacktestResult;
use quantbot::core::universe::TRADEABLE_UNIVERSE;
use quantbot::data::loader::CsvLoader;

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
    /// Paper trade against live data (not yet implemented)
    PaperTrade(StubArgs),
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

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Command::Backtest(args) => run_backtest(args),
        Command::PaperTrade(_) => {
            eprintln!("paper-trade: not yet implemented");
            process::exit(1);
        }
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

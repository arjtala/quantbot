use std::collections::HashMap;
use std::env;
use std::path::PathBuf;

use chrono::NaiveDate;

use quantbot::agents::tsmom::TSMOMAgent;
use quantbot::backtest::engine::{BacktestConfig, BacktestEngine, Snapshot};
use quantbot::backtest::metrics::BacktestResult;
use quantbot::data::loader::CsvLoader;

fn data_dir() -> PathBuf {
    if let Ok(dir) = env::var("QUANTBOT_DATA") {
        return PathBuf::from(dir);
    }
    let candidates = ["data", "../data", "../../data"];
    for c in candidates {
        let p = PathBuf::from(c);
        if p.is_dir() {
            return p;
        }
    }
    panic!("Cannot find data/ directory. Set QUANTBOT_DATA env var.");
}

fn default_config() -> BacktestConfig {
    BacktestConfig {
        initial_cash: 1_000_000.0,
        vol_target: 0.40,
        max_gross_leverage: 2.0,
        max_position_pct: 0.20,
    }
}

fn load_instruments(
    loader: &CsvLoader,
    symbols: &[&str],
    start: NaiveDate,
    end: NaiveDate,
) -> HashMap<String, quantbot::core::bar::BarSeries> {
    let mut map = HashMap::new();
    for &sym in symbols {
        match loader.load_bars(sym, Some(start), Some(end)) {
            Ok(series) => {
                println!("  {sym}: {} bars", series.len());
                map.insert(sym.to_string(), series);
            }
            Err(e) => {
                eprintln!("  {sym}: FAILED — {e}");
            }
        }
    }
    map
}

fn print_eval(label: &str, result: &BacktestResult, python_sharpe: f64) {
    println!("\n{label}");
    println!("{}", result.summary());
    println!("  Rust Sharpe:   {:.4}", result.sharpe_ratio);
    println!("  Python Sharpe: {:.4}", python_sharpe);
    let delta = result.sharpe_ratio - python_sharpe;
    println!(
        "  Delta:         {delta:+.4} ({:+.1}%)",
        delta / python_sharpe.abs().max(0.01) * 100.0
    );
}

/// Print per-instrument exposure from the last snapshot for diagnostics.
fn print_exposure(snapshots: &[Snapshot]) {
    if let Some(last) = snapshots.last() {
        let nav = last.nav;
        println!("\n  Per-instrument exposure (last day {}):", last.date);
        let mut notionals: Vec<_> = last.position_notionals.iter().collect();
        notionals.sort_by(|a, b| b.1.abs().partial_cmp(&a.1.abs()).unwrap());
        for (sym, notional) in &notionals {
            let pct = if nav > 0.0 { *notional / nav * 100.0 } else { 0.0 };
            println!("    {sym:>12}: notional=${notional:>12.0}  ({pct:>+6.1}% of NAV)");
        }
        println!(
            "  Gross exposure: ${:.0}, Net exposure: ${:.0}, NAV: ${:.0}",
            last.gross_exposure, last.net_exposure, nav
        );
        if nav > 0.0 {
            println!(
                "  Gross leverage: {:.2}x",
                last.gross_exposure / nav
            );
        }
    }
}

/// Estimate per-instrument PnL contribution across the eval period.
/// Computes notional change on all days where a position was held in both
/// the previous and current snapshot, regardless of quantity changes.
fn print_pnl_attribution(snapshots: &[Snapshot]) {
    if snapshots.len() < 2 {
        return;
    }
    let first_nav = snapshots[0].nav;
    let last_nav = snapshots.last().unwrap().nav;
    let total_pnl = last_nav - first_nav;

    let mut inst_pnl: HashMap<String, f64> = HashMap::new();
    for window in snapshots.windows(2) {
        let prev = &window[0];
        let curr = &window[1];

        for sym in curr.position_notionals.keys() {
            let prev_notional = prev.position_notionals.get(sym).copied().unwrap_or(0.0);
            let curr_notional = curr.position_notionals.get(sym).copied().unwrap_or(0.0);

            // Only attribute when position existed on both days and notional changed
            // (notional unchanged = no price data that day, skip)
            if prev_notional.abs() > 1e-10 && (curr_notional - prev_notional).abs() > 1e-6 {
                *inst_pnl.entry(sym.to_string()).or_default() += curr_notional - prev_notional;
            }
        }
    }

    let attributed: f64 = inst_pnl.values().sum();

    println!("\n  Per-instrument PnL attribution (notional change, includes rebalance noise):");
    let mut sorted: Vec<_> = inst_pnl.iter().collect();
    sorted.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap());
    for (sym, pnl) in &sorted {
        let pct_of_nav = *pnl / first_nav * 100.0;
        println!(
            "    {sym:>12}: ${pnl:>+12.0}  ({pct_of_nav:>+6.2}% of starting NAV)"
        );
    }
    println!(
        "  Attributed: ${attributed:>+.0} of ${total_pnl:>+.0} total PnL ({:.0}%)",
        if total_pnl.abs() > 1.0 { attributed / total_pnl * 100.0 } else { 0.0 }
    );
}

// ── Validation 1: 60-day, Oct-Dec 2024, 4 instruments ─────────────────
// Python Round 1: TSMOM Sharpe ~1.37

#[test]
fn validate_60day_4_instruments() {
    let loader = CsvLoader::new(data_dir());
    let symbols = ["SPY", "BTC-USD", "ES=F", "GC=F"];

    // Load enough history for 252-bar lookback before Oct 2024
    let data_start = NaiveDate::from_ymd_opt(2021, 1, 1).unwrap();
    let data_end = NaiveDate::from_ymd_opt(2024, 12, 31).unwrap();
    // Eval window: Oct-Dec 2024 only
    let eval_start = NaiveDate::from_ymd_opt(2024, 10, 1).unwrap();

    println!("\n=== Validation 1: 60-day TSMOM (4 instruments, Oct-Dec 2024) ===");
    let instruments = load_instruments(&loader, &symbols, data_start, data_end);
    assert_eq!(instruments.len(), 4);

    let engine = BacktestEngine::new(default_config());
    let agent = TSMOMAgent::new();
    let snapshots = engine.run(&agent, &instruments, 252, Some(eval_start));

    let result = BacktestResult::from_snapshots(&snapshots).unwrap();
    print_eval("60-day eval window (Oct-Dec 2024)", &result, 1.37);
    print_exposure(&snapshots);

    assert!(result.sharpe_ratio.is_finite());
    assert!(result.total_trades > 0);
}

// ── Validation 2: 252-day, Mar 2024-Mar 2025, 6 focused instruments ──
// Python Round 3/4: TSMOM Sharpe ~0.882

#[test]
fn validate_252day_tradeable_universe() {
    let loader = CsvLoader::new(data_dir());
    let symbols = ["GLD", "GC=F", "SPY", "GBPUSD=X", "USDCHF=X", "USDJPY=X"];

    let data_start = NaiveDate::from_ymd_opt(2021, 1, 1).unwrap();
    let data_end = NaiveDate::from_ymd_opt(2025, 3, 31).unwrap();
    let eval_start = NaiveDate::from_ymd_opt(2024, 3, 1).unwrap();

    println!("\n=== Validation 2: Tradeable universe TSMOM (6 instruments, Mar 2024 → Mar 2025) ===");
    let instruments = load_instruments(&loader, &symbols, data_start, data_end);
    assert_eq!(instruments.len(), 6);

    let engine = BacktestEngine::new(default_config());
    let agent = TSMOMAgent::new();
    let snapshots = engine.run(&agent, &instruments, 252, Some(eval_start));

    let result = BacktestResult::from_snapshots(&snapshots).unwrap();
    print_eval("252-day eval window (Mar 2024 → Mar 2025)", &result, 0.882);
    print_exposure(&snapshots);

    assert!(result.sharpe_ratio.is_finite());
    assert!(result.total_trades > 0);
}

// ── Validation 3: Full 21-instrument universe, 252-day ────────────────
// Python Round 3: TSMOM Sharpe ~0.34 (on 15 tradeable)

#[test]
fn validate_full_universe() {
    let loader = CsvLoader::new(data_dir());
    let symbols = [
        "BTC-USD", "ETH-USD", "SOL-USD", "BNB-USD",
        "SPY", "QQQ", "IWM", "EFA", "EEM", "TLT", "GLD",
        "ES=F", "NQ=F", "GC=F", "CL=F", "ZB=F",
        "EURUSD=X", "GBPUSD=X", "USDJPY=X", "AUDUSD=X", "USDCHF=X",
    ];

    let data_start = NaiveDate::from_ymd_opt(2021, 1, 1).unwrap();
    let data_end = NaiveDate::from_ymd_opt(2025, 3, 31).unwrap();
    let eval_start = NaiveDate::from_ymd_opt(2024, 3, 1).unwrap();

    println!("\n=== Validation 3: Full universe TSMOM (21 instruments, Mar 2024 → Mar 2025) ===");
    let instruments = load_instruments(&loader, &symbols, data_start, data_end);

    let engine = BacktestEngine::new(default_config());
    let agent = TSMOMAgent::new();
    let snapshots = engine.run(&agent, &instruments, 252, Some(eval_start));

    let result = BacktestResult::from_snapshots(&snapshots).unwrap();
    print_eval("Full universe 252-day eval window", &result, 0.34);
    print_exposure(&snapshots);
    print_pnl_attribution(&snapshots);

    assert!(result.sharpe_ratio.is_finite());
    assert!(result.total_trades > 0);
}

// ── Speed benchmark ────────────────────────────────────────────────────

#[test]
fn benchmark_backtest_speed() {
    let loader = CsvLoader::new(data_dir());
    let symbols = [
        "BTC-USD", "ETH-USD", "SOL-USD", "BNB-USD",
        "SPY", "QQQ", "IWM", "EFA", "EEM", "TLT", "GLD",
        "ES=F", "NQ=F", "GC=F", "CL=F", "ZB=F",
        "EURUSD=X", "GBPUSD=X", "USDJPY=X", "AUDUSD=X", "USDCHF=X",
    ];

    let start = NaiveDate::from_ymd_opt(2021, 1, 1).unwrap();
    let end = NaiveDate::from_ymd_opt(2025, 3, 31).unwrap();

    let instruments = load_instruments(&loader, &symbols, start, end);
    let engine = BacktestEngine::new(default_config());
    let agent = TSMOMAgent::new();

    // Warm up
    let _ = engine.run(&agent, &instruments, 252, None);

    let iterations = 10;
    let t = std::time::Instant::now();
    for _ in 0..iterations {
        let _ = engine.run(&agent, &instruments, 252, None);
    }
    let elapsed = t.elapsed();
    let per_run = elapsed / iterations;

    println!("\n=== Benchmark: 21-instrument backtest ===");
    println!("  {iterations} iterations in {elapsed:?}");
    println!("  Per run: {per_run:?}");
}

#!/usr/bin/env python3
"""CLI to run TSMOM backtest.

Usage:
    python scripts/run_backtest.py --instruments BTC-USD,SPY,ES=F,GC=F --start 2015-01-01 --end 2025-01-01
"""

from __future__ import annotations

import argparse
import sys
from datetime import date

import pandas as pd

from quantbot.agents.tsmom.agent import TSMOMAgent
from quantbot.backtest.engine import BacktestConfig, BacktestEngine
from quantbot.backtest.metrics import BacktestResult
from quantbot.data.yahoo import YahooProvider


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Run TSMOM backtest")
    parser.add_argument(
        "--instruments",
        type=str,
        default="BTC-USD,SPY,ES=F,GC=F",
        help="Comma-separated instrument symbols",
    )
    parser.add_argument(
        "--start",
        type=str,
        default="2015-01-01",
        help="Start date (YYYY-MM-DD)",
    )
    parser.add_argument(
        "--end",
        type=str,
        default="2025-01-01",
        help="End date (YYYY-MM-DD)",
    )
    parser.add_argument(
        "--cash",
        type=float,
        default=1_000_000.0,
        help="Initial cash",
    )
    parser.add_argument(
        "--vol-target",
        type=float,
        default=0.40,
        help="Annualized volatility target",
    )
    parser.add_argument(
        "--slippage-bps",
        type=float,
        default=5.0,
        help="Slippage in basis points",
    )
    parser.add_argument(
        "--save-plot",
        type=str,
        default=None,
        help="Path to save equity curve plot (e.g., backtest.png)",
    )
    parser.add_argument(
        "--min-history",
        type=int,
        default=252,
        help="Minimum bars of history before first signal",
    )
    return parser.parse_args()


def main() -> None:
    args = parse_args()

    instruments = [s.strip() for s in args.instruments.split(",")]
    start = date.fromisoformat(args.start)
    end = date.fromisoformat(args.end)

    print(f"Fetching data for {instruments} from {start} to {end}...")
    provider = YahooProvider()
    bars_by_instrument: dict[str, pd.DataFrame] = {}

    for sym in instruments:
        try:
            bars = provider.fetch_bars(sym, start, end)
            bars_by_instrument[sym] = bars
            print(f"  {sym}: {len(bars)} bars ({bars.index[0].date()} → {bars.index[-1].date()})")
        except Exception as e:
            print(f"  {sym}: FAILED — {e}", file=sys.stderr)

    if not bars_by_instrument:
        print("No data fetched. Exiting.", file=sys.stderr)
        sys.exit(1)

    # Configure and run
    config = BacktestConfig(
        initial_cash=args.cash,
        slippage_bps=args.slippage_bps,
        vol_target=args.vol_target,
    )
    agent = TSMOMAgent(vol_target=args.vol_target)
    engine = BacktestEngine(config)

    print(f"\nRunning backtest with {len(bars_by_instrument)} instruments...")
    snapshots = engine.run(agent, bars_by_instrument, min_history=args.min_history)

    if len(snapshots) < 2:
        print("Not enough snapshots to compute results.", file=sys.stderr)
        sys.exit(1)

    result = BacktestResult.from_snapshots(snapshots)
    print(result.summary())

    print("\nMonthly Returns:")
    print(result.monthly_returns.to_string(float_format=lambda x: f"{x:.1%}"))

    result.plot(save_path=args.save_plot)


if __name__ == "__main__":
    main()

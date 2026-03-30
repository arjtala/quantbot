#!/usr/bin/env python3
"""Round 1 Evaluation: Go/No-Go for Phase 3 Rust rewrite.

Tests:
1. TSMOM-only vs TSMOM+LLM Sharpe comparison
2. Per-agent directional accuracy
3. Multi-instrument validation (SPY, BTC-USD, ES=F, GC=F)

Usage:
    python scripts/eval_round1.py [--days 60] [--instruments SPY,BTC-USD,ES=F,GC=F]
"""

from __future__ import annotations

import argparse
import json
import logging
import sys
import time
from dataclasses import dataclass, field
from datetime import date, timedelta
from pathlib import Path

import numpy as np
import pandas as pd

from quantbot.agents.decision.combiner import SignalCombiner
from quantbot.agents.indicator.agent import make_indicator_node
from quantbot.agents.indicator.tools import compute_all_indicators
from quantbot.agents.shared.llm import parse_signal_response, get_llm_client
from quantbot.agents.tsmom.agent import TSMOMAgent
from quantbot.config import load_prompt, settings
from quantbot.core.signal import Signal, SignalDirection, SignalType
from quantbot.data.yahoo import YahooProvider

logging.basicConfig(level=logging.WARNING)
logger = logging.getLogger(__name__)

RESULTS_DIR = Path("eval_results")


@dataclass
class DayResult:
    date: str
    instrument: str
    actual_return: float
    tsmom_direction: str
    tsmom_strength: float
    tsmom_confidence: float
    indicator_direction: str = "FLAT"
    indicator_strength: float = 0.0
    indicator_confidence: float = 0.0
    indicator_reasoning: str = ""
    combined_direction: str = "FLAT"
    combined_strength: float = 0.0
    tsmom_correct: bool = False
    indicator_correct: bool = False
    combined_correct: bool = False


def run_indicator_agent_raw(bars: pd.DataFrame, instrument: str) -> Signal:
    """Run the indicator agent directly (without LangGraph state overhead)."""
    indicators = compute_all_indicators(bars)

    system_prompt = load_prompt("indicator_system")
    user_content = f"""## Instrument: {instrument}

## Current Technical Indicators
```json
{json.dumps(indicators, indent=2)}
```

## How to Read These Indicators
- **trend.trend_regime**: The current trend direction based on SMA slopes and price position
- **Momentum (MACD histogram, ROC)**: Positive = bullish momentum, negative = bearish. These CONFIRM trends.
- **Oscillators (RSI, Stochastic, Williams %R)**: Show overbought/oversold. In a trend, these confirm strength — only signal exhaustion when they DIVERGE from price.

Assess whether the indicators confirm or contradict the current trend regime, then produce your signal."""

    from langchain_core.messages import HumanMessage, SystemMessage

    llm = get_llm_client(settings.indicator_model)
    messages = [
        SystemMessage(content=system_prompt),
        HumanMessage(content=user_content),
    ]

    response = llm.invoke(messages)
    return parse_signal_response(response.content, instrument, "Indicator")


def evaluate_instrument(
    instrument: str,
    bars: pd.DataFrame,
    eval_days: int,
    min_history: int = 252,
) -> list[DayResult]:
    """Run day-by-day evaluation on one instrument."""
    results = []
    tsmom = TSMOMAgent()
    combiner = SignalCombiner()

    total_bars = len(bars)
    start_idx = max(min_history, total_bars - eval_days - 1)

    print(f"\n  Evaluating {instrument}: {total_bars - start_idx - 1} days")
    print(f"  {'Day':>4} {'Date':>12} {'Actual':>8} {'TSMOM':>8} {'Indicator':>10} {'Combined':>10}")
    print(f"  {'-'*4} {'-'*12} {'-'*8} {'-'*8} {'-'*10} {'-'*10}")

    for i in range(start_idx, total_bars - 1):
        today = bars.index[i]
        history = bars.iloc[: i + 1]
        actual_return = float(bars["Close"].iloc[i + 1] / bars["Close"].iloc[i] - 1)
        actual_dir = "LONG" if actual_return > 0 else "SHORT"

        day_num = i - start_idx + 1

        # TSMOM signal (fast, no LLM)
        tsmom_sig = tsmom.generate_signal(history, instrument)

        # Indicator agent (LLM call)
        try:
            ind_sig = run_indicator_agent_raw(history, instrument)
        except Exception as e:
            logger.warning("Indicator agent failed on %s %s: %s", instrument, today.date(), e)
            ind_sig = Signal(
                instrument=instrument,
                direction=SignalDirection.FLAT,
                strength=0.0,
                confidence=0.0,
                agent_name="Indicator",
                signal_type=SignalType.LLM,
                metadata={"error": str(e)},
            )

        # Combined signal
        combined = combiner.combine([tsmom_sig, ind_sig])

        # Check correctness (FLAT counts as neither correct nor incorrect)
        tsmom_correct = tsmom_sig.direction.value == actual_dir if tsmom_sig.direction != SignalDirection.FLAT else False
        ind_correct = ind_sig.direction.value == actual_dir if ind_sig.direction != SignalDirection.FLAT else False
        comb_correct = combined.direction.value == actual_dir if combined.direction != SignalDirection.FLAT else False

        result = DayResult(
            date=str(today.date()),
            instrument=instrument,
            actual_return=actual_return,
            tsmom_direction=tsmom_sig.direction.value,
            tsmom_strength=tsmom_sig.strength,
            tsmom_confidence=tsmom_sig.confidence,
            indicator_direction=ind_sig.direction.value,
            indicator_strength=ind_sig.strength,
            indicator_confidence=ind_sig.confidence,
            indicator_reasoning=ind_sig.metadata.get("llm_reasoning", ind_sig.metadata.get("reasoning", "")),
            combined_direction=combined.direction.value,
            combined_strength=combined.strength,
            tsmom_correct=tsmom_correct,
            indicator_correct=ind_correct,
            combined_correct=comb_correct,
        )
        results.append(result)

        # Progress
        t_mark = "✓" if tsmom_correct else ("·" if tsmom_sig.direction == SignalDirection.FLAT else "✗")
        i_mark = "✓" if ind_correct else ("·" if ind_sig.direction == SignalDirection.FLAT else "✗")
        c_mark = "✓" if comb_correct else ("·" if combined.direction == SignalDirection.FLAT else "✗")
        print(
            f"  {day_num:>4} {today.date()} {actual_return:>+7.2%} "
            f"{tsmom_sig.direction.value:>6}{t_mark} "
            f"{ind_sig.direction.value:>8}{i_mark} "
            f"{combined.direction.value:>8}{c_mark}"
        )

    return results


def compute_accuracy(results: list[DayResult], field_dir: str, field_correct: str) -> dict:
    """Compute accuracy stats for a given agent."""
    non_flat = [r for r in results if getattr(r, field_dir) != "FLAT"]
    if not non_flat:
        return {"total": 0, "non_flat": 0, "correct": 0, "accuracy": 0.0, "flat_pct": 1.0}

    correct = sum(1 for r in non_flat if getattr(r, field_correct))
    flat_count = len(results) - len(non_flat)

    return {
        "total": len(results),
        "non_flat": len(non_flat),
        "correct": correct,
        "accuracy": correct / len(non_flat) if non_flat else 0.0,
        "flat_pct": flat_count / len(results) if results else 0.0,
    }


def compute_pnl(results: list[DayResult], direction_field: str) -> dict:
    """Compute simple PnL assuming unit position sized by strength."""
    daily_returns = []
    for r in results:
        d = getattr(r, direction_field)
        if d == "LONG":
            daily_returns.append(r.actual_return)
        elif d == "SHORT":
            daily_returns.append(-r.actual_return)
        else:
            daily_returns.append(0.0)

    rets = np.array(daily_returns)
    ann_return = float(np.mean(rets) * 252)
    ann_vol = float(np.std(rets) * np.sqrt(252))
    sharpe = ann_return / ann_vol if ann_vol > 1e-8 else 0.0
    cumulative = float(np.prod(1 + rets) - 1)
    max_dd = float(np.min(np.minimum.accumulate(np.cumprod(1 + rets)) / np.maximum.accumulate(np.cumprod(1 + rets)) - 1))

    return {
        "ann_return": ann_return,
        "ann_vol": ann_vol,
        "sharpe": sharpe,
        "cumulative": cumulative,
        "max_drawdown": max_dd,
    }


def main():
    parser = argparse.ArgumentParser(description="Round 1 Evaluation")
    parser.add_argument("--days", type=int, default=60, help="Number of eval days")
    parser.add_argument(
        "--instruments", type=str,
        default="BTC-USD,ETH-USD,SOL-USD,BNB-USD,SPY,QQQ,IWM,EFA,EEM,TLT,GLD,ES=F,NQ=F,GC=F,CL=F,ZB=F,EURUSD=X,GBPUSD=X,USDJPY=X,AUDUSD=X,USDCHF=X",
        help="Comma-separated instruments (default: full 21-instrument universe)",
    )
    parser.add_argument("--start", type=str, default="2022-01-01", help="Data start date")
    parser.add_argument("--end", type=str, default="2025-01-01", help="Data end date")
    parser.add_argument("--data-dir", type=str, default=None, help="Load bars from CSV files in this directory instead of Yahoo Finance")
    parser.add_argument("--run-name", type=str, default=None, help="Name for this eval run (e.g. model name). Results saved to eval_results/<run-name>/")
    args = parser.parse_args()

    instruments = [s.strip() for s in args.instruments.split(",")]
    start = date.fromisoformat(args.start)
    end = date.fromisoformat(args.end)

    # Determine results directory
    if args.run_name:
        results_dir = RESULTS_DIR / args.run_name
    else:
        # Auto-name from indicator model setting
        model_name = settings.indicator_model.split(":")[-1].replace("/", "_")
        results_dir = RESULTS_DIR / model_name
    results_dir.mkdir(parents=True, exist_ok=True)

    print("=" * 70)
    print("  ROUND 1 EVALUATION — Go/No-Go for Phase 3")
    print("=" * 70)
    print(f"  Instruments: {instruments}")
    print(f"  Eval days:   {args.days}")
    print(f"  Data range:  {start} → {end}")
    print(f"  Model:       {settings.indicator_model}")
    print(f"  Results dir: {results_dir}")

    # Fetch data
    print("\nFetching data...")
    all_bars: dict[str, pd.DataFrame] = {}
    if not args.data_dir:
        provider = YahooProvider()
    for sym in instruments:
        try:
            if args.data_dir:
                csv_path = Path(args.data_dir) / f"{sym}.csv"
                bars = pd.read_csv(csv_path, index_col=0, parse_dates=True)
                # Ensure expected column names
                for col in ["Open", "High", "Low", "Close", "Volume"]:
                    if col not in bars.columns:
                        raise ValueError(f"Missing column {col} in {csv_path}")
            else:
                bars = provider.fetch_bars(sym, start, end)
            all_bars[sym] = bars
            print(f"  {sym}: {len(bars)} bars")
        except Exception as e:
            print(f"  {sym}: FAILED — {e}", file=sys.stderr)

    # Run evaluation
    all_results: dict[str, list[DayResult]] = {}
    start_time = time.time()

    for sym, bars in all_bars.items():
        results = evaluate_instrument(sym, bars, args.days)
        all_results[sym] = results

    elapsed = time.time() - start_time

    # Save raw results
    results_dir.mkdir(parents=True, exist_ok=True)
    for sym, results in all_results.items():
        rows = [vars(r) for r in results]
        df = pd.DataFrame(rows)
        df.to_csv(results_dir / f"round1_{sym}.csv", index=False)

    # Summary
    print("\n" + "=" * 70)
    print("  RESULTS SUMMARY")
    print("=" * 70)

    for sym, results in all_results.items():
        tsmom_acc = compute_accuracy(results, "tsmom_direction", "tsmom_correct")
        ind_acc = compute_accuracy(results, "indicator_direction", "indicator_correct")
        comb_acc = compute_accuracy(results, "combined_direction", "combined_correct")

        tsmom_pnl = compute_pnl(results, "tsmom_direction")
        ind_pnl = compute_pnl(results, "indicator_direction")
        comb_pnl = compute_pnl(results, "combined_direction")

        print(f"\n  {sym}")
        print(f"  {'':15} {'Accuracy':>10} {'Sharpe':>8} {'Ann.Ret':>10} {'MaxDD':>10} {'Flat%':>8}")
        print(f"  {'-'*15} {'-'*10} {'-'*8} {'-'*10} {'-'*10} {'-'*8}")
        print(
            f"  {'TSMOM-only':15} {tsmom_acc['accuracy']:>9.1%} {tsmom_pnl['sharpe']:>8.2f} "
            f"{tsmom_pnl['ann_return']:>9.1%} {tsmom_pnl['max_drawdown']:>9.1%} {tsmom_acc['flat_pct']:>7.1%}"
        )
        print(
            f"  {'Indicator-only':15} {ind_acc['accuracy']:>9.1%} {ind_pnl['sharpe']:>8.2f} "
            f"{ind_pnl['ann_return']:>9.1%} {ind_pnl['max_drawdown']:>9.1%} {ind_acc['flat_pct']:>7.1%}"
        )
        print(
            f"  {'TSMOM+Indicator':15} {comb_acc['accuracy']:>9.1%} {comb_pnl['sharpe']:>8.2f} "
            f"{comb_pnl['ann_return']:>9.1%} {comb_pnl['max_drawdown']:>9.1%} {comb_acc['flat_pct']:>7.1%}"
        )

    # Aggregate across instruments
    all_flat = [r for results in all_results.values() for r in results]
    if all_flat:
        print(f"\n  AGGREGATE (all instruments)")
        for label, dir_f, cor_f in [
            ("TSMOM-only", "tsmom_direction", "tsmom_correct"),
            ("Indicator-only", "indicator_direction", "indicator_correct"),
            ("TSMOM+Indicator", "combined_direction", "combined_correct"),
        ]:
            acc = compute_accuracy(all_flat, dir_f, cor_f)
            pnl = compute_pnl(all_flat, dir_f)
            print(
                f"  {label:15} Acc={acc['accuracy']:.1%}  Sharpe={pnl['sharpe']:.2f}  "
                f"Ann.Ret={pnl['ann_return']:.1%}  MaxDD={pnl['max_drawdown']:.1%}"
            )

    print(f"\n  Elapsed: {elapsed:.0f}s ({elapsed/60:.1f} min)")
    print(f"  Results saved to {results_dir}/")

    # Go/No-Go verdict
    agg_tsmom = compute_pnl(all_flat, "tsmom_direction")
    agg_combined = compute_pnl(all_flat, "combined_direction")
    sharpe_delta = agg_combined["sharpe"] - agg_tsmom["sharpe"]

    print(f"\n  GO/NO-GO: Sharpe delta (TSMOM+LLM vs TSMOM-only) = {sharpe_delta:+.3f}")
    if sharpe_delta > 0.1:
        print("  VERDICT: ✓ GO — LLM agents add meaningful alpha")
    elif sharpe_delta > 0:
        print("  VERDICT: ~ MARGINAL — LLM agents add some value, consider simplifying")
    else:
        print("  VERDICT: ✗ NO-GO — LLM agents don't improve over TSMOM. Rethink before Rust port.")

    print("=" * 70)


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""Simulate different combiner strategies on existing Round 1b CSV results.

Zero LLM calls — just replays the indicator + TSMOM signals with different
weighting schemes and computes what Sharpe *would have been*.

Experiments:
1. Fixed combiner (current: 0.50/0.20 TSMOM/Indicator)
2. Instrument-type dynamic combiner
3. FLAT-aware position sizing
4. Indicator-only (baseline)
5. TSMOM-only (baseline)
6. Drop EEM/EFA from universe

Also includes transaction cost sensitivity analysis for IG spread betting.

Usage:
    python scripts/simulate_combiner.py [--results-dir eval_results/finr1]
"""

from __future__ import annotations

import argparse
from dataclasses import dataclass
from pathlib import Path

import numpy as np
import pandas as pd

# ---------------------------------------------------------------------------
# Instrument classification
# ---------------------------------------------------------------------------

INSTRUMENT_TYPE = {
    # Crypto
    "BTC-USD": "crypto", "ETH-USD": "crypto", "SOL-USD": "crypto", "BNB-USD": "crypto",
    # Equity ETFs
    "SPY": "equity", "QQQ": "equity", "IWM": "equity", "EFA": "equity",
    "EEM": "equity", "TLT": "bonds",
    # Gold (split from commodity — different strategy)
    "GLD": "gold", "GC=F": "gold",
    # Futures
    "ES=F": "equity", "NQ=F": "equity",
    "CL=F": "commodity", "ZB=F": "bonds",
    # FX
    "EURUSD=X": "forex", "GBPUSD=X": "forex", "USDJPY=X": "forex",
    "AUDUSD=X": "forex", "USDCHF=X": "forex",
}

# Round 1b dynamic combiner weights (60-day eval)
DYNAMIC_WEIGHTS_60D = {
    "forex":     {"tsmom": 0.20, "indicator": 0.80},
    "crypto":    {"tsmom": 0.30, "indicator": 0.70},
    "equity":    {"tsmom": 0.80, "indicator": 0.20},
    "bonds":     {"tsmom": 0.30, "indicator": 0.70},
    "commodity": {"tsmom": 0.50, "indicator": 0.50},
    "gold":      {"tsmom": 0.50, "indicator": 0.50},
}

# Revised weights based on 252-day eval (Round 3)
DYNAMIC_WEIGHTS_252D = {
    "gold":      {"tsmom": 0.50, "indicator": 0.50},
    "equity":    {"tsmom": 1.00, "indicator": 0.00},
    "forex":     {"tsmom": 0.10, "indicator": 0.90},
    "crypto":    {"tsmom": 0.50, "indicator": 0.50},
    "bonds":     {"tsmom": 0.50, "indicator": 0.50},
    "commodity": {"tsmom": 0.50, "indicator": 0.50},
}

# IG spread costs in basis points (round-trip: open + close)
IG_SPREAD_BPS = {
    "forex": 3.0,       # ~0.6-1 pip on EURUSD ≈ 3 bps round-trip
    "crypto": 80.0,     # ~50-100 bps spread on BTC
    "equity": 5.0,      # ~1-2 points on SPY/ES ≈ 5 bps
    "bonds": 10.0,      # ~2-3 ticks
    "commodity": 15.0,  # ~3-6 cents on CL
    "gold": 10.0,       # ~0.3-0.6 points on gold ≈ 10 bps
}

# Instruments to drop (negative Sharpe across all strategies in 252-day eval)
DROP_INSTRUMENTS = {"EEM", "EFA"}

# Focused 6-instrument universe (252-day eval winners)
FOCUSED_UNIVERSE = {"GLD", "GC=F", "SPY", "GBPUSD=X", "USDCHF=X", "USDJPY=X"}


# ---------------------------------------------------------------------------
# Combiner strategies
# ---------------------------------------------------------------------------

def strategy_tsmom_only(row: pd.Series) -> tuple[str, float]:
    """Pure TSMOM direction."""
    return row["tsmom_direction"], 1.0


def strategy_indicator_only(row: pd.Series) -> tuple[str, float]:
    """Pure indicator direction."""
    return row["indicator_direction"], 1.0


def strategy_fixed_combiner(row: pd.Series) -> tuple[str, float]:
    """Current fixed combiner: 0.50 TSMOM / 0.20 Indicator."""
    return row["combined_direction"], 1.0


def _make_dynamic_combiner(weights: dict) -> callable:
    """Create a dynamic combiner strategy with given weights."""
    def strategy(row: pd.Series) -> tuple[str, float]:
        inst_type = INSTRUMENT_TYPE.get(row["instrument"], "equity")
        w = weights.get(inst_type, {"tsmom": 0.50, "indicator": 0.50})

        def dir_to_sign(d: str) -> float:
            if d == "LONG": return 1.0
            if d == "SHORT": return -1.0
            return 0.0

        t_sign = dir_to_sign(row["tsmom_direction"])
        i_sign = dir_to_sign(row["indicator_direction"])

        w_t = w["tsmom"]
        w_i = w["indicator"]

        numerator = w_t * t_sign * abs(row["tsmom_strength"]) * row["tsmom_confidence"] + \
                    w_i * i_sign * abs(row["indicator_strength"]) * row["indicator_confidence"]
        denominator = w_t * row["tsmom_confidence"] + w_i * row["indicator_confidence"]

        if denominator < 1e-8:
            return "FLAT", 0.0

        combined = numerator / denominator

        if abs(combined) < 0.10:
            return "FLAT", 0.0
        elif combined > 0:
            return "LONG", abs(combined)
        else:
            return "SHORT", abs(combined)
    return strategy


def strategy_dynamic_combiner(row: pd.Series) -> tuple[str, float]:
    """Instrument-type dynamic weights (60-day weights)."""
    return _make_dynamic_combiner(DYNAMIC_WEIGHTS_60D)(row)


def strategy_dynamic_252d(row: pd.Series) -> tuple[str, float]:
    """Instrument-type dynamic weights (252-day revised weights)."""
    return _make_dynamic_combiner(DYNAMIC_WEIGHTS_252D)(row)


def strategy_dynamic_flat_aware(row: pd.Series) -> tuple[str, float]:
    """Dynamic combiner (60-day weights) + FLAT-aware sizing.

    When indicator is FLAT, reduce position to 50% of TSMOM-only size
    instead of overriding with full TSMOM weight.
    """
    ind_dir = row["indicator_direction"]

    if ind_dir == "FLAT":
        # Indicator uncertain → half-size TSMOM position
        tsmom_dir = row["tsmom_direction"]
        return tsmom_dir, 0.5

    # Otherwise, use dynamic combiner at full size
    direction, _ = strategy_dynamic_combiner(row)
    return direction, 1.0


# ---------------------------------------------------------------------------
# Evaluation
# ---------------------------------------------------------------------------

def compute_returns(df: pd.DataFrame, strategy_fn, apply_costs: bool = False) -> pd.Series:
    """Compute daily returns for a strategy.

    Transaction costs are only charged when the position changes
    (open, close, or flip direction) — not on hold days.
    """
    returns = []
    # Track previous direction per instrument to detect position changes
    prev_direction: dict[str, str] = {}

    for _, row in df.iterrows():
        direction, size = strategy_fn(row)
        actual = row["actual_return"]
        instrument = row["instrument"]

        if direction == "LONG":
            ret = actual * size
        elif direction == "SHORT":
            ret = -actual * size
        else:
            ret = 0.0

        # Only charge spread cost when position changes
        if apply_costs:
            prev = prev_direction.get(instrument, "FLAT")
            position_changed = (direction != prev)

            if position_changed:
                inst_type = INSTRUMENT_TYPE.get(instrument, "equity")
                cost_bps = IG_SPREAD_BPS[inst_type]

                if prev != "FLAT" and direction != "FLAT":
                    # Flip (close old + open new) → double spread
                    ret -= 2 * cost_bps / 10_000 * size
                elif prev == "FLAT" and direction != "FLAT":
                    # New entry → single spread
                    ret -= cost_bps / 10_000 * size
                elif prev != "FLAT" and direction == "FLAT":
                    # Close → single spread (applied to previous day's return)
                    ret -= cost_bps / 10_000  # close cost at unit size

            prev_direction[instrument] = direction

        returns.append(ret)

    return pd.Series(returns, index=df.index)


def compute_metrics(returns: pd.Series) -> dict:
    """Compute Sharpe, ann return, max DD from daily returns."""
    ann_ret = float(np.mean(returns) * 252)
    ann_vol = float(np.std(returns) * np.sqrt(252))
    sharpe = ann_ret / ann_vol if ann_vol > 1e-8 else 0.0
    cumulative = float(np.prod(1 + returns) - 1)

    cum_product = np.cumprod(1 + returns)
    running_max = np.maximum.accumulate(cum_product)
    drawdown = cum_product / running_max - 1
    max_dd = float(np.min(drawdown))

    n_traded = int((returns != 0).sum())
    n_total = len(returns)
    flat_pct = 1.0 - n_traded / n_total if n_total > 0 else 0.0

    return {
        "sharpe": sharpe,
        "ann_return": ann_ret,
        "ann_vol": ann_vol,
        "max_dd": max_dd,
        "cumulative": cumulative,
        "flat_pct": flat_pct,
        "n_days": n_total,
    }


def main():
    parser = argparse.ArgumentParser(description="Simulate combiner strategies on Round 1b results")
    parser.add_argument("--results-dir", type=str, default="eval_results/finr1")
    args = parser.parse_args()

    results_dir = Path(args.results_dir)
    if not results_dir.exists():
        print(f"ERROR: Results directory not found: {results_dir}")
        return

    # Load all CSVs
    all_data = []
    for csv_file in sorted(results_dir.glob("round1_*.csv")):
        sym = csv_file.stem.replace("round1_", "")
        df = pd.read_csv(csv_file)
        all_data.append(df)

    if not all_data:
        print("ERROR: No CSV files found")
        return

    full_df = pd.concat(all_data, ignore_index=True)
    clean_df = full_df[~full_df["instrument"].isin(DROP_INSTRUMENTS)]

    print("=" * 85)
    print("  COMBINER SIMULATION — Replay Round 1b with Different Strategies")
    print("=" * 85)
    print(f"  Source: {results_dir}")
    print(f"  Full universe: {full_df['instrument'].nunique()} instruments, {len(full_df)} data points")
    print(f"  Clean universe: {clean_df['instrument'].nunique()} instruments (dropped {', '.join(DROP_INSTRUMENTS)})")

    strategies = {
        "TSMOM-only": strategy_tsmom_only,
        "Indicator-only": strategy_indicator_only,
        "Fixed combiner (0.50/0.20)": strategy_fixed_combiner,
        "Dynamic combiner (60d wts)": strategy_dynamic_combiner,
        "Dynamic combiner (252d wts)": strategy_dynamic_252d,
        "Dynamic + FLAT-aware": strategy_dynamic_flat_aware,
    }

    # --- Full universe results ---
    print(f"\n{'─' * 85}")
    print(f"  FULL UNIVERSE ({full_df['instrument'].nunique()} instruments)")
    print(f"{'─' * 85}")
    print(f"  {'Strategy':<30} {'Sharpe':>8} {'Ann.Ret':>10} {'MaxDD':>10} {'Flat%':>8}")
    print(f"  {'─'*30} {'─'*8} {'─'*10} {'─'*10} {'─'*8}")

    for name, fn in strategies.items():
        rets = compute_returns(full_df, fn)
        m = compute_metrics(rets)
        print(f"  {name:<30} {m['sharpe']:>8.3f} {m['ann_return']:>9.1%} {m['max_dd']:>9.1%} {m['flat_pct']:>7.1%}")

    # --- Clean universe (drop EEM/EFA) ---
    print(f"\n{'─' * 85}")
    print(f"  CLEAN UNIVERSE (dropped EEM, EFA)")
    print(f"{'─' * 85}")
    print(f"  {'Strategy':<30} {'Sharpe':>8} {'Ann.Ret':>10} {'MaxDD':>10} {'Flat%':>8}")
    print(f"  {'─'*30} {'─'*8} {'─'*10} {'─'*10} {'─'*8}")

    for name, fn in strategies.items():
        rets = compute_returns(clean_df, fn)
        m = compute_metrics(rets)
        print(f"  {name:<30} {m['sharpe']:>8.3f} {m['ann_return']:>9.1%} {m['max_dd']:>9.1%} {m['flat_pct']:>7.1%}")

    # --- Transaction cost sensitivity (clean universe) ---
    print(f"\n{'─' * 85}")
    print(f"  TRANSACTION COST SENSITIVITY (clean universe, IG spread costs)")
    print(f"{'─' * 85}")
    print(f"  Spread assumptions: forex={IG_SPREAD_BPS['forex']}bps, crypto={IG_SPREAD_BPS['crypto']}bps, "
          f"equity={IG_SPREAD_BPS['equity']}bps, bonds={IG_SPREAD_BPS['bonds']}bps, commodity={IG_SPREAD_BPS['commodity']}bps")
    print()
    print(f"  {'Strategy':<30} {'Sharpe':>8} {'w/costs':>8} {'Δ Sharpe':>10}")
    print(f"  {'─'*30} {'─'*8} {'─'*8} {'─'*10}")

    for name, fn in strategies.items():
        rets_no_cost = compute_returns(clean_df, fn, apply_costs=False)
        rets_with_cost = compute_returns(clean_df, fn, apply_costs=True)
        m_no = compute_metrics(rets_no_cost)
        m_with = compute_metrics(rets_with_cost)
        delta = m_with["sharpe"] - m_no["sharpe"]
        print(f"  {name:<30} {m_no['sharpe']:>8.3f} {m_with['sharpe']:>8.3f} {delta:>+9.3f}")

    # --- Per instrument-type breakdown (best strategy) ---
    print(f"\n{'─' * 85}")
    print(f"  PER INSTRUMENT-TYPE BREAKDOWN — Dynamic 252d weights (clean universe)")
    print(f"{'─' * 85}")
    print(f"  {'Type':<12} {'Instruments':>5} {'Sharpe':>8} {'Ann.Ret':>10} {'MaxDD':>10} {'Flat%':>8}")
    print(f"  {'─'*12} {'─'*5} {'─'*8} {'─'*10} {'─'*10} {'─'*8}")

    for inst_type in ["gold", "forex", "equity", "crypto", "bonds", "commodity"]:
        type_instruments = {k for k, v in INSTRUMENT_TYPE.items() if v == inst_type} - DROP_INSTRUMENTS
        type_df = clean_df[clean_df["instrument"].isin(type_instruments)]
        if type_df.empty:
            continue
        rets = compute_returns(type_df, strategy_dynamic_252d)
        m = compute_metrics(rets)
        print(f"  {inst_type:<12} {len(type_instruments):>5} {m['sharpe']:>8.3f} {m['ann_return']:>9.1%} {m['max_dd']:>9.1%} {m['flat_pct']:>7.1%}")

    # --- Focused 6-instrument universe ---
    focused_df = full_df[full_df["instrument"].isin(FOCUSED_UNIVERSE)]
    if not focused_df.empty:
        print(f"\n{'─' * 85}")
        print(f"  FOCUSED UNIVERSE — 6 instruments (GLD, GC=F, SPY, GBPUSD, USDCHF, USDJPY)")
        print(f"{'─' * 85}")
        print(f"  {'Strategy':<30} {'Sharpe':>8} {'w/costs':>8} {'Δ':>8} {'Ann.Ret':>10} {'MaxDD':>10}")
        print(f"  {'─'*30} {'─'*8} {'─'*8} {'─'*8} {'─'*10} {'─'*10}")

        for name, fn in strategies.items():
            rets_no = compute_returns(focused_df, fn, apply_costs=False)
            rets_yes = compute_returns(focused_df, fn, apply_costs=True)
            m_no = compute_metrics(rets_no)
            m_yes = compute_metrics(rets_yes)
            delta = m_yes["sharpe"] - m_no["sharpe"]
            print(f"  {name:<30} {m_no['sharpe']:>8.3f} {m_yes['sharpe']:>8.3f} {delta:>+7.3f} {m_no['ann_return']:>9.1%} {m_no['max_dd']:>9.1%}")

        # Per-instrument detail for focused universe
        print(f"\n  Per-instrument (Dynamic 252d weights):")
        for sym in sorted(FOCUSED_UNIVERSE):
            sym_df = focused_df[focused_df["instrument"] == sym]
            if sym_df.empty:
                continue
            rets = compute_returns(sym_df, strategy_dynamic_252d)
            m = compute_metrics(rets)
            inst_type = INSTRUMENT_TYPE.get(sym, "?")
            print(f"    {sym:<12} ({inst_type:<6})  Sharpe={m['sharpe']:>+.3f}  Ann.Ret={m['ann_return']:>+.1%}  MaxDD={m['max_dd']:>.1%}")

    # --- Per-instrument detail (all, sorted by Sharpe) ---
    print(f"\n{'─' * 85}")
    print(f"  PER-INSTRUMENT SHARPE — Dynamic 252d weights (clean universe)")
    print(f"{'─' * 85}")

    inst_sharpes = []
    for sym in clean_df["instrument"].unique():
        sym_df = clean_df[clean_df["instrument"] == sym]
        rets = compute_returns(sym_df, strategy_dynamic_252d)
        m = compute_metrics(rets)
        inst_sharpes.append((sym, m["sharpe"], m["ann_return"], m["max_dd"]))

    inst_sharpes.sort(key=lambda x: x[1], reverse=True)
    print(f"  {'Instrument':<12} {'Type':<12} {'Sharpe':>8} {'Ann.Ret':>10} {'MaxDD':>10}")
    print(f"  {'─'*12} {'─'*12} {'─'*8} {'─'*10} {'─'*10}")
    for sym, sharpe, ann_ret, max_dd in inst_sharpes:
        inst_type = INSTRUMENT_TYPE.get(sym, "?")
        print(f"  {sym:<12} {inst_type:<12} {sharpe:>8.3f} {ann_ret:>9.1%} {max_dd:>9.1%}")

    print("\n" + "=" * 85)


if __name__ == "__main__":
    main()

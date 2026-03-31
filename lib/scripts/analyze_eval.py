#!/usr/bin/env python3
"""Comprehensive evaluation analysis across 3 models: deepseek, finr1, qwen3.
Uses only stdlib (csv, math, os, glob) — no pandas/numpy required."""

import os
import csv
import math
import glob
from collections import defaultdict

BASE_DIR = "/home/arjangt/src/quantbot/eval_results"
MODELS = ["deepseek", "finr1", "qwen3"]
STRATEGIES = ["tsmom", "indicator", "combined"]
ANNUALIZE = math.sqrt(252)


def load_all_data():
    """Load all CSVs for all models into a dict of list-of-dicts."""
    data = {}
    for model in MODELS:
        model_dir = os.path.join(BASE_DIR, model)
        rows = []
        files = sorted(glob.glob(os.path.join(model_dir, "round1_*.csv")))
        for f in files:
            with open(f, "r") as fh:
                reader = csv.DictReader(fh)
                for row in reader:
                    # Convert numeric fields
                    row["actual_return"] = float(row["actual_return"])
                    row["tsmom_strength"] = float(row["tsmom_strength"])
                    row["tsmom_confidence"] = float(row["tsmom_confidence"])
                    row["indicator_strength"] = float(row["indicator_strength"])
                    row["indicator_confidence"] = float(row["indicator_confidence"])
                    row["combined_strength"] = float(row["combined_strength"])
                    row["tsmom_correct"] = row["tsmom_correct"].strip() == "True"
                    row["indicator_correct"] = row["indicator_correct"].strip() == "True"
                    row["combined_correct"] = row["combined_correct"].strip() == "True"
                    rows.append(row)
        data[model] = rows
        print(f"Loaded {model}: {len(files)} files, {len(rows)} rows")
    return data


def get_direction(row, strategy):
    if strategy == "tsmom":
        return row["tsmom_direction"]
    elif strategy == "indicator":
        return row["indicator_direction"]
    else:
        return row["combined_direction"]


def strategy_return(row, strategy):
    d = get_direction(row, strategy)
    r = row["actual_return"]
    if d == "LONG":
        return r
    elif d == "SHORT":
        return -r
    else:
        return 0.0


def mean(vals):
    return sum(vals) / len(vals) if vals else 0.0


def std(vals):
    if len(vals) < 2:
        return 0.0
    m = mean(vals)
    return math.sqrt(sum((v - m) ** 2 for v in vals) / (len(vals) - 1))


def accuracy_incl_flat(rows, strategy):
    """Accuracy using the _correct column (FLAT counted as wrong)."""
    col = f"{strategy}_correct"
    if not rows:
        return 0.0
    return sum(1 for r in rows if r[col]) / len(rows)


def accuracy_ex_flat(rows, strategy):
    """Accuracy excluding FLAT signals from denominator."""
    non_flat = [r for r in rows if get_direction(r, strategy) != "FLAT"]
    if not non_flat:
        return 0.0
    col = f"{strategy}_correct"
    return sum(1 for r in non_flat if r[col]) / len(non_flat)


def flat_rate(rows, strategy):
    if not rows:
        return 0.0
    return sum(1 for r in rows if get_direction(r, strategy) == "FLAT") / len(rows)


def perf_metrics(rows, strategy):
    """Returns (sharpe, ann_return, max_drawdown)."""
    rets = [strategy_return(r, strategy) for r in rows]
    if not rets:
        return 0.0, 0.0, 0.0
    m = mean(rets)
    s = std(rets)
    sharpe = (m / s * ANNUALIZE) if s > 0 else 0.0
    ann_ret = m * 252
    # Max drawdown
    cum = 1.0
    peak = 1.0
    max_dd = 0.0
    for r in rets:
        cum *= (1 + r)
        if cum > peak:
            peak = cum
        dd = (cum - peak) / peak if peak > 0 else 0.0
        if dd < max_dd:
            max_dd = dd
    return sharpe, ann_ret, max_dd


def pct(v):
    return f"{v * 100:.1f}%"


def print_separator(char="=", width=130):
    print(char * width)


def main():
    data = load_all_data()
    print()

    # =========================================================================
    # 1. OVERALL SUMMARY PER MODEL
    # =========================================================================
    print_separator()
    print("## OVERALL SUMMARY PER MODEL")
    print_separator()

    header = (
        f"{'Model':<12} | {'Strategy':<12} | {'Acc(exFLAT)':>11} | {'Acc(incl)':>9} | "
        f"{'FLAT%':>7} | {'Sharpe':>8} | {'Ann Ret':>9} | {'Max DD':>9}"
    )
    print(header)
    print("-" * len(header))

    results = {}

    for model in MODELS:
        rows = data[model]
        results[model] = {}
        for strat in STRATEGIES:
            acc_ex = accuracy_ex_flat(rows, strat)
            acc_in = accuracy_incl_flat(rows, strat)
            fl = flat_rate(rows, strat)
            sharpe, ann_ret, max_dd = perf_metrics(rows, strat)
            results[model][strat] = {
                "acc_ex_flat": acc_ex,
                "acc_incl": acc_in,
                "flat_pct": fl,
                "sharpe": sharpe,
                "ann_ret": ann_ret,
                "max_dd": max_dd,
            }
            print(
                f"{model:<12} | {strat:<12} | {pct(acc_ex):>11} | {pct(acc_in):>9} | "
                f"{pct(fl):>7} | {sharpe:>8.3f} | {pct(ann_ret):>9} | {pct(max_dd):>9}"
            )
        print("-" * len(header))

    # =========================================================================
    # 2. FLAT SIGNAL ANALYSIS
    # =========================================================================
    print()
    print_separator()
    print("## FLAT SIGNAL ANALYSIS")
    print_separator()

    header2 = f"{'Model':<12} | {'TSMOM FLAT%':>12} | {'Indicator FLAT%':>16} | {'Combined FLAT%':>15} | {'Total Rows':>10}"
    print(header2)
    print("-" * len(header2))
    for model in MODELS:
        rows = data[model]
        t_flat = flat_rate(rows, "tsmom")
        i_flat = flat_rate(rows, "indicator")
        c_flat = flat_rate(rows, "combined")
        print(
            f"{model:<12} | {pct(t_flat):>12} | {pct(i_flat):>16} | {pct(c_flat):>15} | {len(rows):>10}"
        )

    # =========================================================================
    # 3. FLAT SIGNAL BREAKDOWN BY INSTRUMENT
    # =========================================================================
    print()
    print_separator()
    print("## INDICATOR FLAT% BY INSTRUMENT (per model)")
    print_separator()

    instruments = sorted(set(r["instrument"] for r in data[MODELS[0]]))
    header3 = f"{'Instrument':<16} | " + " | ".join(f"{m:>12}" for m in MODELS)
    print(header3)
    print("-" * len(header3))
    for inst in instruments:
        vals = []
        for model in MODELS:
            inst_rows = [r for r in data[model] if r["instrument"] == inst]
            fl = flat_rate(inst_rows, "indicator")
            vals.append(f"{pct(fl):>12}")
        print(f"{inst:<16} | " + " | ".join(vals))

    # =========================================================================
    # 4. PER-INSTRUMENT BREAKDOWN (Combined strategy)
    # =========================================================================
    print()
    print_separator()
    print("## PER-INSTRUMENT BREAKDOWN (Combined strategy)")
    print_separator()

    for model in MODELS:
        print(f"\n### {model.upper()}")
        header4 = (
            f"{'Instrument':<16} | {'N':>4} | {'Acc':>6} | {'FLAT%':>6} | "
            f"{'Sharpe':>8} | {'Ann Ret':>9} | {'Max DD':>9}"
        )
        print(header4)
        print("-" * len(header4))

        for inst in instruments:
            inst_rows = [r for r in data[model] if r["instrument"] == inst]
            n = len(inst_rows)
            acc = accuracy_incl_flat(inst_rows, "combined")
            fl = flat_rate(inst_rows, "combined")
            sharpe, ann_ret, max_dd = perf_metrics(inst_rows, "combined")
            print(
                f"{inst:<16} | {n:>4} | {pct(acc):>6} | {pct(fl):>6} | "
                f"{sharpe:>8.3f} | {pct(ann_ret):>9} | {pct(max_dd):>9}"
            )

    # =========================================================================
    # 5. INDICATOR-ONLY PER-INSTRUMENT BREAKDOWN
    # =========================================================================
    print()
    print_separator()
    print("## PER-INSTRUMENT BREAKDOWN (Indicator-only strategy)")
    print_separator()

    for model in MODELS:
        print(f"\n### {model.upper()}")
        header5 = (
            f"{'Instrument':<16} | {'N':>4} | {'Acc(exFLAT)':>11} | {'Acc(incl)':>9} | {'FLAT%':>6} | "
            f"{'Sharpe':>8} | {'Ann Ret':>9} | {'Max DD':>9}"
        )
        print(header5)
        print("-" * len(header5))

        for inst in instruments:
            inst_rows = [r for r in data[model] if r["instrument"] == inst]
            n = len(inst_rows)
            acc_ex = accuracy_ex_flat(inst_rows, "indicator")
            acc_in = accuracy_incl_flat(inst_rows, "indicator")
            fl = flat_rate(inst_rows, "indicator")
            sharpe, ann_ret, max_dd = perf_metrics(inst_rows, "indicator")
            print(
                f"{inst:<16} | {n:>4} | {pct(acc_ex):>11} | {pct(acc_in):>9} | {pct(fl):>6} | "
                f"{sharpe:>8.3f} | {pct(ann_ret):>9} | {pct(max_dd):>9}"
            )

    # =========================================================================
    # 6. MODEL COMPARISON TABLE
    # =========================================================================
    print()
    print_separator()
    print("## MODEL COMPARISON (side-by-side)")
    print_separator()

    metrics_to_compare = [
        ("TSMOM Accuracy", lambda r: r["tsmom"]["acc_incl"], "pct"),
        ("Indicator Acc (incl FLAT)", lambda r: r["indicator"]["acc_incl"], "pct"),
        ("Indicator Acc (ex FLAT)", lambda r: r["indicator"]["acc_ex_flat"], "pct"),
        ("Combined Accuracy", lambda r: r["combined"]["acc_incl"], "pct"),
        ("Indicator FLAT%", lambda r: r["indicator"]["flat_pct"], "pct"),
        ("Combined FLAT%", lambda r: r["combined"]["flat_pct"], "pct"),
        ("TSMOM Sharpe", lambda r: r["tsmom"]["sharpe"], "num"),
        ("Indicator Sharpe", lambda r: r["indicator"]["sharpe"], "num"),
        ("Combined Sharpe", lambda r: r["combined"]["sharpe"], "num"),
        ("TSMOM Ann Return", lambda r: r["tsmom"]["ann_ret"], "pct"),
        ("Indicator Ann Return", lambda r: r["indicator"]["ann_ret"], "pct"),
        ("Combined Ann Return", lambda r: r["combined"]["ann_ret"], "pct"),
        ("TSMOM Max DD", lambda r: r["tsmom"]["max_dd"], "pct"),
        ("Indicator Max DD", lambda r: r["indicator"]["max_dd"], "pct"),
        ("Combined Max DD", lambda r: r["combined"]["max_dd"], "pct"),
    ]

    header6 = f"{'Metric':<35} | " + " | ".join(f"{m:>14}" for m in MODELS)
    print(header6)
    print("-" * len(header6))

    for name, fn, fmt in metrics_to_compare:
        vals = []
        for model in MODELS:
            v = fn(results[model])
            if fmt == "num":
                vals.append(f"{v:>14.3f}")
            else:
                vals.append(f"{pct(v):>14}")
        print(f"{name:<35} | " + " | ".join(vals))

    # =========================================================================
    # 7. SHARPE DELTA ANALYSIS
    # =========================================================================
    print()
    print_separator()
    print("## SHARPE DELTA ANALYSIS (vs TSMOM baseline)")
    print_separator()

    header7 = f"{'Model':<12} | {'TSMOM Sharpe':>13} | {'Ind Sharpe':>11} | {'Comb Sharpe':>12} | {'Ind Delta':>10} | {'Comb Delta':>11}"
    print(header7)
    print("-" * len(header7))

    best_model = None
    best_acc = 0
    best_sharpe_delta = -999

    for model in MODELS:
        ts = results[model]["tsmom"]["sharpe"]
        ind = results[model]["indicator"]["sharpe"]
        comb = results[model]["combined"]["sharpe"]
        d_ind = ind - ts
        d_comb = comb - ts
        print(
            f"{model:<12} | {ts:>13.3f} | {ind:>11.3f} | {comb:>12.3f} | {d_ind:>+10.3f} | {d_comb:>+11.3f}"
        )
        comb_acc = results[model]["combined"]["acc_incl"]
        if comb_acc > best_acc or (comb_acc == best_acc and d_comb > best_sharpe_delta):
            best_model = model
            best_acc = comb_acc
            best_sharpe_delta = d_comb

    # Find best standalone indicator model
    best_standalone_model = None
    best_standalone_acc = 0
    best_standalone_sharpe_delta = -999

    for model in MODELS:
        ind_acc_ex = results[model]["indicator"]["acc_ex_flat"]
        ind_sharpe = results[model]["indicator"]["sharpe"]
        ts_sharpe = results[model]["tsmom"]["sharpe"]
        delta = ind_sharpe - ts_sharpe
        if ind_acc_ex > best_standalone_acc or (ind_acc_ex == best_standalone_acc and delta > best_standalone_sharpe_delta):
            best_standalone_model = model
            best_standalone_acc = ind_acc_ex
            best_standalone_sharpe_delta = delta

    # =========================================================================
    # 8. GO / NO-GO VERDICT
    # =========================================================================
    print()
    print_separator("=")
    print("## GO / NO-GO VERDICT")
    print_separator("=")

    print(f"\nBest model (combined strategy): {best_model}")
    print(f"  Combined accuracy (incl FLAT): {pct(best_acc)}")
    print(f"  Sharpe delta (combined - TSMOM): {best_sharpe_delta:+.3f}")

    print(f"\nBest model (indicator standalone): {best_standalone_model}")
    print(f"  Indicator accuracy (ex FLAT): {pct(best_standalone_acc)}")
    print(f"  Sharpe delta (indicator - TSMOM): {best_standalone_sharpe_delta:+.3f}")

    go_acc = best_standalone_acc > 0.55
    go_sharpe = best_standalone_sharpe_delta > 0.15

    print(f"\nCriteria check:")
    print(f"  >55% standalone accuracy:  {pct(best_standalone_acc):>8} {'PASS' if go_acc else 'FAIL'}")
    print(f"  >+0.15 Sharpe delta:       {best_standalone_sharpe_delta:>+8.3f} {'PASS' if go_sharpe else 'FAIL'}")

    if go_acc and go_sharpe:
        print(f"\n  >>> VERDICT: GO <<<")
    else:
        print(f"\n  >>> VERDICT: NO-GO <<<")

    print(f"\n--- Additional Context ---")
    print(f"Round 1 FLAT signal issue check (was 30-57% in Round 1):")
    for model in MODELS:
        ind_flat = results[model]["indicator"]["flat_pct"]
        comb_flat = results[model]["combined"]["flat_pct"]
        print(f"  {model}: Indicator FLAT={pct(ind_flat)}, Combined FLAT={pct(comb_flat)}")

    print()


if __name__ == "__main__":
    main()

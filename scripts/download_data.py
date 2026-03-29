#!/usr/bin/env python3
"""Download OHLCV data for eval_round1.py and save as CSV.

Run this on a machine with internet access, then copy data/ to the cluster.

Usage:
    python scripts/download_data.py
    scp -r data/ cluster:~/src/quantbot/data/
"""

from datetime import date
from pathlib import Path

import yfinance as yf

INSTRUMENTS = ["SPY", "BTC-USD", "ES=F", "GC=F"]
START = "2022-01-01"
END = "2025-01-01"
OUT_DIR = Path("data")


def main():
    OUT_DIR.mkdir(exist_ok=True)
    for sym in INSTRUMENTS:
        print(f"Downloading {sym}...", end=" ")
        df = yf.download(sym, start=START, end=END, auto_adjust=False)
        if df.empty:
            print("FAILED (empty)")
            continue
        # Flatten multi-level columns if present
        if hasattr(df.columns, "levels") and len(df.columns.levels) > 1:
            df.columns = df.columns.get_level_values(0)
        out_path = OUT_DIR / f"{sym}.csv"
        df.to_csv(out_path)
        print(f"{len(df)} bars → {out_path}")

    print(f"\nDone. Copy to cluster:\n  scp -r data/ cluster:~/src/quantbot/data/")


if __name__ == "__main__":
    main()

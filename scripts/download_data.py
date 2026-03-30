#!/usr/bin/env python3
"""Download OHLCV data for eval_round1.py and save as CSV.

Run this on a machine with internet access, then copy data/ to the cluster.

Usage:
    python scripts/download_data.py
    python scripts/download_data.py --instruments SPY,BTC-USD  # subset
    scp -r data/ cluster:~/src/quantbot/data/
"""

import argparse
from pathlib import Path

import yfinance as yf

# Full universe: 21 instruments across 5 asset classes
ALL_INSTRUMENTS = [
    # Crypto (4)
    "BTC-USD", "ETH-USD", "SOL-USD", "BNB-USD",
    # Equity ETFs (7)
    "SPY", "QQQ", "IWM", "EFA", "EEM", "TLT", "GLD",
    # Futures (5)
    "ES=F", "NQ=F", "GC=F", "CL=F", "ZB=F",
    # FX (5)
    "EURUSD=X", "GBPUSD=X", "USDJPY=X", "AUDUSD=X", "USDCHF=X",
]

DEFAULT_START = "2022-01-01"
DEFAULT_END = "2025-01-01"
OUT_DIR = Path("data")


def main():
    parser = argparse.ArgumentParser(description="Download OHLCV data for quantbot eval")
    parser.add_argument(
        "--instruments", type=str, default=",".join(ALL_INSTRUMENTS),
        help="Comma-separated instrument symbols (default: full 21-instrument universe)",
    )
    parser.add_argument("--start", type=str, default=DEFAULT_START)
    parser.add_argument("--end", type=str, default=DEFAULT_END)
    parser.add_argument("--out-dir", type=str, default=str(OUT_DIR))
    args = parser.parse_args()

    instruments = [s.strip() for s in args.instruments.split(",")]
    out_dir = Path(args.out_dir)
    out_dir.mkdir(exist_ok=True)

    print(f"Downloading {len(instruments)} instruments ({args.start} → {args.end})")
    print(f"Output: {out_dir}/\n")

    success = 0
    failed = []
    for sym in instruments:
        print(f"  {sym:15s}", end=" ")
        try:
            df = yf.download(sym, start=args.start, end=args.end, auto_adjust=True, progress=False)
            if df.empty:
                print("FAILED (empty)")
                failed.append(sym)
                continue
            # Flatten multi-level columns if present
            if hasattr(df.columns, "levels") and len(df.columns.levels) > 1:
                df = df.droplevel(level=1, axis=1)
            out_path = out_dir / f"{sym}.csv"
            df.to_csv(out_path)
            print(f"{len(df):>5} bars → {out_path}")
            success += 1
        except Exception as e:
            print(f"FAILED ({e})")
            failed.append(sym)

    print(f"\nDone: {success}/{len(instruments)} succeeded")
    if failed:
        print(f"Failed: {', '.join(failed)}")
    print(f"\nCopy to cluster:\n  scp -r {out_dir}/ cluster:~/src/quantbot/{out_dir}/")


if __name__ == "__main__":
    main()

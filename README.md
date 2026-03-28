# Quantbot

Hybrid multi-agent trading system combining **quantitative rules-based strategies** with **LLM-powered agents**. Built for paper trading against live multi-asset data (crypto, equities, futures/forex) with a backtesting framework for strategy validation.

## Architecture

All agents — deterministic quant and non-deterministic LLM — produce a unified **`Signal`** (direction, strength, confidence). A confidence-weighted ensemble combines signals into portfolio decisions. LangGraph orchestrates agents in parallel (fan-out), merging at a Decision node (fan-in).

```
┌──────────────┐  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐
│  TSMOM Agent │  │  Indicator   │  │   Pattern    │  │    Trend     │
│  (Quant)     │  │  Agent (LLM) │  │  Agent (LLM) │  │  Agent (LLM) │
└──────┬───────┘  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘
       │                 │                 │                 │
       └────────┬────────┴────────┬────────┘                 │
                │     Signal      │                          │
                ▼    Combiner     ▼                          │
          ┌─────────────────────────────────────────────────────┐
          │              Decision Agent                         │
          │   weighted ensemble → vol-targeted sizing → risk    │
          └─────────────────────┬───────────────────────────────┘
                                │
                                ▼
                      ┌──────────────────┐
                      │  Paper / Live    │
                      │  Execution       │
                      └──────────────────┘
```

## Quick Start

```bash
# Create environment
conda create -n quantbot python=3.12 -y
conda activate quantbot

# Install
pip install -e ".[dev]"

# Run backtest
python scripts/run_backtest.py \
  --instruments BTC-USD,SPY,ES=F,GC=F \
  --start 2015-01-01 \
  --end 2025-01-01 \
  --save-plot backtest.html
```

## Strategy: Time-Series Momentum (TSMOM)

Based on [Moskowitz, Ooi, Pedersen (JFE 2012)](https://doi.org/10.1016/j.jfineco.2011.11.003):

- Compute trailing return over multiple lookbacks (1, 3, 6, 12 months)
- Signal = average sign across lookbacks
- Position sized via EWMA volatility targeting (40% annualized)
- Confidence = fraction of lookbacks in agreement

The paper shows Sharpe > 1.0 across 58 instruments. With a 4-instrument test universe:

| Metric | Value |
|--------|-------|
| Ann. Return | 11.94% |
| Ann. Volatility | 16.52% |
| Sharpe Ratio | 0.72 |
| Sortino Ratio | 0.92 |
| Max Drawdown | -32.16% |
| Total Return | 358.1% |

## Project Structure

```
quantbot/
├── quantbot/
│   ├── core/           # Signal, Portfolio, Position types
│   ├── data/           # Data providers (Yahoo), instrument universes
│   ├── agents/
│   │   └── tsmom/      # Time-series momentum agent + EWMA volatility
│   ├── backtest/       # Engine (event-driven, next-open execution) + metrics
│   └── execution/      # Paper trading with configurable slippage
├── scripts/
│   └── run_backtest.py # CLI for running backtests
├── tests/              # Unit tests (pytest)
├── JOURNAL.md          # Research notes and paper reviews
└── PROGRESS.md         # Implementation progress tracker
```

## Roadmap

See [PROGRESS.md](PROGRESS.md) for detailed status.

| Phase | Description | Status |
|-------|-------------|--------|
| 1 | Quant Core — TSMOM agent + backtest engine | Done |
| 2 | LangGraph + LLM agents (indicator, pattern, trend, decision) | Planned |
| 3 | Paper trading loop + FastAPI web dashboard | Planned |
| 4 | Rust port of performance-critical paths via PyO3 (hybrid approach — [research notes](PROGRESS.md#rust-ecosystem-research)) | Planned |
| 5 | Extensions — new strategies, CCXT, alerting | Planned |

## Testing

```bash
pytest tests/ -v
```

## License

Private — not yet licensed for distribution.

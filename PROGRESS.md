# Implementation Progress

## Phase 1: Quant Core — TSMOM + Backtest Infrastructure
> **Status: Complete** | 22/22 tests passing

### Core Types
- [x] `quantbot/core/signal.py` — `Signal` dataclass, `SignalDirection`/`SignalType` enums
- [x] `quantbot/core/portfolio.py` — `Position`, `Order`, `Fill`, `PortfolioState`
- [x] `quantbot/data/bar.py` — `BarDataFrame` type alias, `validate_bars()`

### Data Layer
- [x] `quantbot/data/provider.py` — Abstract `DataProvider` base class
- [x] `quantbot/data/universe.py` — `Instrument` dataclass, predefined universes (crypto, equity, futures)
- [x] `quantbot/data/yahoo.py` — `YahooProvider` wrapping yfinance

### TSMOM Agent
- [x] `quantbot/agents/base.py` — Abstract `QuantAgent` base class
- [x] `quantbot/agents/tsmom/volatility.py` — `ewma_volatility()` (EWMA, com=60, annualized)
- [x] `quantbot/agents/tsmom/agent.py` — Multi-lookback momentum (1/3/6/12 mo), vol-targeted sizing (40% target)

### Backtest
- [x] `quantbot/backtest/engine.py` — Event-driven engine, next-open execution, look-ahead prevention
- [x] `quantbot/backtest/metrics.py` — `BacktestResult`: Sharpe, Sortino, Calmar, max DD, equity curve plot

### Execution
- [x] `quantbot/execution/paper.py` — `PaperTradingEngine` with configurable slippage

### Scripts & Tests
- [x] `scripts/run_backtest.py` — CLI with `--instruments`, `--start`, `--end`, `--save-plot`
- [x] `tests/test_tsmom.py` — 7 tests (signal direction, vol scaling, edge cases)
- [x] `tests/test_backtest.py` — 12 tests (engine mechanics, metrics computation)
- [x] `pyproject.toml` — Package config, all dependencies

### Changes
- **Plotting switched from matplotlib to Plotly**: Interactive equity curve + drawdown with hover tooltips, log-scale NAV, unified crosshair. Saves to `.html` (interactive) or static image.

### Bugs Fixed
- **Mark-to-market double-counting**: `_mark_to_market` was adding PnL to cash AND updating entry price, inflating NAV. Fixed to only update entry price; cash tracks cumulative trade costs.
- **KeyError on multi-asset weekends**: `gross_exposure`/`net_exposure` used `prices[sym]` which fails when an instrument doesn't trade that day (e.g. ES=F on a BTC weekend date). Fixed to `prices.get(sym, pos.avg_entry_price)`.

### Validation
- [x] Run backtest on BTC-USD, SPY, ES=F, GC=F (2015-01-01 → 2025-01-01)
  - **Sharpe 0.72** | Ann. Return 11.94% | Ann. Vol 16.52% | Max DD -32.16%
  - Sortino 0.92 | Calmar 0.37 | Total Return 358.1% | 8,887 trades
  - Below paper's 1.0+ Sharpe — expected with 4 instruments vs paper's 58. Diversification benefit scales with universe size.

---

## Phase 2: LangGraph + LLM Agents
> **Status: Not started**

- [ ] `quantbot/graph/state.py` — `TradingGraphState` with signal accumulation reducer
- [ ] `quantbot/graph/builder.py` — Fan-out/fan-in graph builder with configurable agents
- [ ] `quantbot/agents/indicator/agent.py` + `tools.py` — RSI, MACD, Stoch via TA-Lib → LLM interpretation
- [ ] `quantbot/agents/pattern/agent.py` + `charts.py` — Candlestick chart → vision LLM pattern recognition
- [ ] `quantbot/agents/trend/agent.py` + `trendlines.py` — Support/resistance fitting → annotated chart
- [ ] `quantbot/agents/decision/combiner.py` — Confidence-weighted signal ensemble
- [ ] `quantbot/agents/decision/agent.py` — Combined decision + risk checks
- [ ] `quantbot/config.py` — Pydantic Settings from `.env`

---

## Phase 3: Paper Trading + Web Dashboard
> **Status: Not started**

- [ ] `quantbot/core/clock.py` — `LiveClock` vs `BacktestClock`
- [ ] `quantbot/execution/risk.py` — Position/exposure limits
- [ ] `quantbot/data/cache.py` — Local parquet cache at `~/.quantbot/data/`
- [ ] `scripts/run_paper_trading.py` — Async live trading loop
- [ ] `quantbot/web/app.py` — FastAPI dashboard

---

## Phase 4: Rust Port
> **Status: Not started**

- [ ] `quantbot-core/` Rust crate via PyO3/maturin
- [ ] Candidates: BacktestEngine loop, TSMOM signals, EWMA vol, SignalCombiner, portfolio accounting

---

## Phase 5: Extensions
> **Status: Not started**

- [ ] Cross-sectional momentum, mean reversion, carry agents
- [ ] Correlation-aware sizing, drawdown deleveraging
- [ ] CCXT crypto provider
- [ ] Alerting (Slack/email)

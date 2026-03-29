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

### Core Infrastructure
- [ ] `quantbot/graph/state.py` — `TradingGraphState` with signal accumulation reducer
- [ ] `quantbot/graph/builder.py` — Fan-out/fan-in graph builder with configurable agents
- [ ] `quantbot/config.py` — Pydantic Settings from `.env`

### LLM Agents
- [ ] `quantbot/agents/indicator/agent.py` + `tools.py` — RSI, MACD, Stoch via TA-Lib → LLM interpretation
- [ ] `quantbot/agents/pattern/agent.py` + `charts.py` — Candlestick chart → vision LLM pattern recognition
- [ ] `quantbot/agents/trend/agent.py` + `trendlines.py` — Support/resistance fitting → annotated chart

### Decision Layer
- [ ] `quantbot/agents/decision/combiner.py` — Confidence-weighted signal ensemble
- [ ] `quantbot/agents/decision/agent.py` — Combined decision + risk checks

### Research-Informed Enhancements

**Bull/Bear Debate Pattern** *(from TradingAgents, JOURNAL.md §H)*
- [ ] Add bull and bear advocate agents to the graph that argue opposing positions before the Decision Agent adjudicates
- [ ] Decision Agent receives structured arguments (not just numeric signals) for better conflict resolution
- [ ] Configurable: can disable debate and fall back to pure numeric `SignalCombiner`

**Chain-of-Thought Structured Prompts** *(from MarketSenseAI, JOURNAL.md §N)*
- [ ] All LLM agent prompts must enforce step-by-step reasoning: identify signal → assess strength → consider contradicting evidence → state confidence → conclude with direction
- [ ] Use structured output parsing (Pydantic models) to extract `Signal` from CoT reasoning
- [ ] Target: 70%+ directional accuracy on held-out data (MarketSenseAI achieved 72.3%)

**Agent Decision Memory via SQLite** *(from FinMem, JOURNAL.md §G)*
- [ ] `quantbot/memory/store.py` — SQLite-backed memory store at `~/.quantbot/memory.db`
- [ ] Tables: `signal_log` (every signal produced), `decision_log` (combiner outputs + actual P&L outcomes), `agent_memory` (condensed lessons injected into LLM prompts)
- [ ] On each LLM agent invocation, inject recent decision history + win/loss record into the system prompt
- [ ] SQLite chosen over JSON/parquet: ACID transactions for concurrent writes during live trading, SQL queries for analysis ("all BTC-USD signals where confidence > 0.8 that were wrong"), zero config, stdlib `sqlite3`, clean migration path to Postgres, and Rust has excellent support (`rusqlite`/`sqlx`) for Phase 4

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
> **Status: Not started** | Approach updated: **full Rust rewrite** (not hybrid PyO3)

### Updated Recommendation: Full Rust Over Hybrid

After deeper analysis, the Rust ecosystem has matured enough that maintaining a PyO3 FFI boundary is more overhead than benefit. A pure Rust codebase is cleaner to maintain, test, and deploy.

**Approach:**
1. Port Phase 1 (backtest engine + TSMOM) first — pure numerical computation, massive speedups (~1-2 weeks)
2. Build Phase 2 (LLM agents) in Rust using `rig-core` + `tokio` — skip graph frameworks, hand-roll with `tokio::join!` (~1 week)
3. Web dashboard with `axum` — single binary serving API + trading engine

### Portability Assessment

No hard Python-only blockers. LangGraph's fan-out/fan-in is trivially `tokio::join!`:
```rust
let (tsmom, indicator, pattern, trend) = tokio::join!(
    tsmom_agent.generate_signal(&bars),
    indicator_agent.generate_signal(&bars),
    pattern_agent.generate_signal(&bars),
    trend_agent.generate_signal(&bars),
);
let decision = combiner.combine(vec![tsmom, indicator, pattern, trend]);
```

### Crate Mapping

| Python | Rust Crate | Maturity | Notes |
|--------|-----------|----------|-------|
| `pandas` | `polars` | Production | Faster, columnar, excellent for OHLCV |
| `numpy` | `ndarray` / native iterators | Production | EWMA is ~20 lines of Rust |
| `yfinance` | `yahoo_finance_api` | OK | Less polished but functional |
| `plotly` | `plotly-rs` / export JSON | OK | Or serve from web frontend |
| `pydantic` | `serde` + Rust structs | Production | Type system already strict |
| `langgraph` | `tokio::join!` + hand-rolled | N/A | Our graph is simple enough |
| `langchain-*` | `rig-core` | Good | OpenAI, Anthropic, structured output, tools |
| `TA-Lib` | `ta` crate / FFI to C lib | OK | RSI, MACD, Stochastic, Bollinger |
| `fastapi` | `axum` / `actix-web` | Production | Both excellent for REST APIs |
| `ccxt` | `reqwest` + exchange APIs | Manual | No Rust ccxt port exists |
| `scipy` | `statrs` | OK | Only used minimally |
| `argparse` | `clap` | Production | Best-in-class CLI parsing |

### Effort Estimate

| Component | Effort | Expected Speedup |
|-----------|--------|------------------|
| Core types (signal, portfolio) | ~1 day | Negligible (already fast) |
| Data layer (polars + yahoo) | ~2 days | 5-10x on data processing |
| TSMOM agent + volatility | ~2 days | 10-50x on computation |
| Backtest engine | ~3-4 days | **50-100x** (hot loop) |
| LLM agents (rig-core + tokio) | ~3-4 days | Network-bound (similar speed) |
| Metrics + plotting | ~1-2 days | Moderate |
| **Total** | **~2-3 weeks** | |

### Trade-off Analysis

| Factor | Full Rust | Hybrid (PyO3) | Stay Python |
|--------|-----------|---------------|-------------|
| Backtest performance | 10-100x faster | 5-50x on hot paths | Baseline |
| Development speed | Slower initially | Medium | Fastest |
| LLM agent iteration | Load prompts from files* | Python for prompts | Fastest |
| Deployment | Single binary | Python + Rust ext | Python env needed |
| Maintenance | One language | FFI boundary overhead | One language |

*Prompt changes don't need recompile if loaded from `.md`/`.txt` at runtime.

### Rust Graph Framework Research

| Crate | What it is | Verdict |
|-------|-----------|---------|
| [langchain-rust](https://github.com/Abraxas-365/langchain-rust) | Rust port of LangChain. OpenAI, Anthropic, Ollama. 532 commits. | No graph orchestration. LLM API calls only. |
| [rs-graph-llm](https://github.com/a-agmon/rs-graph-llm) | Graph execution engine inspired by LangGraph. 278 stars. Stateful sessions, conditional routing. | Most promising framework. Gap: no parallel fan-out built-in. |
| [rrag-graph](https://docs.rs/rrag-graph/latest/rrag_graph/) | Graph workflow for AI agents. Async, conditional routing. | v0.1.0-alpha. Not production-ready. |
| [langgraph-api](https://crates.io/crates/langgraph-api) | HTTP client SDK for hosted LangGraph Cloud. | Not a graph engine — just an API client. |

**Verdict:** Skip graph frameworks. `tokio::join!` + pattern matching covers our needs. `rig-core` for LLM calls.

### Implementation Plan
- [ ] Scaffold `quantbot-rs/` with `Cargo.toml` (polars, ndarray, rig-core, tokio, clap, axum, ta, plotly, chrono, serde)
- [ ] Port core types: Signal, Portfolio, Position, Order
- [ ] Port data layer: polars BarDataFrame, yahoo_finance_api provider, Instrument universe
- [ ] Port TSMOM agent + EWMA volatility
- [ ] Port backtest engine (main speedup target: 50-100x)
- [ ] Port metrics + plotly-rs charting
- [ ] Port paper trading engine
- [ ] Build LLM agents with rig-core (indicator, pattern, trend)
- [ ] Build signal combiner + decision agent with tokio::join! fan-out/fan-in
- [ ] CLI via clap, web dashboard via axum

---

## Phase 5: Extensions
> **Status: Not started**

### New Agents & Strategies
- [ ] Cross-sectional momentum, mean reversion, carry agents
- [ ] **Chronos Agent** *(JOURNAL.md §O)* — Zero-shot time series forecasting via Amazon's Chronos foundation model. Tokenizes price series into probabilistic forecasts. Especially valuable for new instruments lacking TSMOM lookback history.
- [ ] **AlphaGen Agent** *(JOURNAL.md §F, §J)* — Self-improving alpha discovery loop. LLM generates mathematical factor code, runs it through `BacktestEngine`, evaluates Sharpe, iteratively refines. Promotes winners to production.

### Risk & Sizing
- [ ] Correlation-aware sizing, drawdown deleveraging
- [ ] **RL-based dynamic agent weighting** *(JOURNAL.md §Q)* — Replace fixed `SignalCombiner` weights with a bandit/PPO agent that learns optimal agent weights per market regime. RL survey shows hybrid methods outperform by 15-20%.

### Infrastructure
- [ ] CCXT crypto provider
- [ ] Alerting (Slack/email)
- [ ] **Local Fin-R1 model via Ollama** *(JOURNAL.md §K)* — 7B model matching GPT-4 on financial reasoning. Replace API calls with local inference for near-zero cost paper trading. Supported by `rig-core` (Rust) and `langchain` (Python) via Ollama.

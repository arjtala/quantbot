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
> **Status: Code complete** | Requires API keys for end-to-end testing | 22/22 Phase 1 tests still passing

### Core Infrastructure
- [x] `quantbot/config.py` — Pydantic Settings from `.env`, per-agent model selection, risk limits
- [x] `.env.example` — Template with all configuration options
- [x] `quantbot/graph/state.py` — `TradingGraphState` with signal accumulation reducer
- [x] `quantbot/graph/builder.py` — Fan-out/fan-in graph builder with configurable agents

### Prompt Templates (runtime-loaded, CoT-structured)
- [x] `quantbot/prompts/indicator_system.md` — 5-step CoT: identify → assess → contradict → confidence → conclude
- [x] `quantbot/prompts/pattern_system.md` — Visual pattern recognition with CoT reasoning
- [x] `quantbot/prompts/trend_system.md` — Support/resistance analysis with CoT reasoning
- [x] `quantbot/prompts/bull_advocate.md` — Structured bullish argument template
- [x] `quantbot/prompts/bear_advocate.md` — Structured bearish argument template
- [x] `quantbot/prompts/decision_system.md` — Multi-signal synthesis with decision rules

### Shared Utilities
- [x] `quantbot/agents/shared/llm.py` — LLM client routing (OpenAI/Anthropic), JSON signal parsing with fallback
- [x] `quantbot/agents/shared/chart_renderer.py` — Candlestick chart rendering to BytesIO (fix: save before `plt.close()`)

### LLM Agents
- [x] `quantbot/agents/tsmom/graph_adapter.py` — Phase 1 TSMOM wrapped as graph node (no LLM, deterministic anchor)
- [x] `quantbot/agents/indicator/tools.py` — Pure numpy/pandas: RSI, MACD, Stochastic, ROC, Williams %R
- [x] `quantbot/agents/indicator/agent.py` — TA indicators → LLM interpretation via CoT
- [x] `quantbot/agents/pattern/charts.py` — Candlestick chart rendering with SMA overlays
- [x] `quantbot/agents/pattern/agent.py` — Vision LLM chart pattern recognition
- [x] `quantbot/agents/trend/trendlines.py` — Support/resistance detection + linear trendline fitting
- [x] `quantbot/agents/trend/agent.py` — Annotated chart → vision LLM trend analysis

### Decision Layer
- [x] `quantbot/agents/decision/combiner.py` — Confidence-weighted signal ensemble: `Σ(w×s×c) / Σ(w×c)`
- [x] `quantbot/agents/decision/agent.py` — LLM-based decision synthesis with debate + memory context
- [x] `quantbot/agents/risk/agent.py` — Risk manager with veto authority (leverage, confidence floor)

### Bull/Bear Debate *(from TradingAgents, JOURNAL.md §H)*
- [x] `quantbot/agents/debate/bull.py` — Bull advocate with structured argument output
- [x] `quantbot/agents/debate/bear.py` — Bear advocate with structured argument output
- [x] `quantbot/agents/debate/moderator.py` — Runs debate, structures arguments for Decision Agent
- [x] Configurable via `DEBATE_ENABLED` — can disable and fall back to pure numeric combiner

### Agent Decision Memory *(from FinMem, JOURNAL.md §G)*
- [x] `quantbot/memory/store.py` — SQLite-backed memory at `~/.quantbot/memory.db`
- [x] Tables: `signal_log`, `decision_log` (with outcome tracking), `agent_memory` (lessons)
- [x] `build_memory_context()` — generates text summary for LLM prompt injection
- [x] Win/loss analytics via `get_win_rate()`

### Evaluation Harness
- [x] `quantbot/eval/backtest_llm.py` — Run agents on historical data, measure directional accuracy
- [x] `quantbot/eval/agent_ablation.py` — Compare agent combinations, measure marginal contribution

### Validation
- [ ] Run full graph on SPY for one day — all agents produce signals, combiner output is reasonable
- [ ] Compare TSMOM-only vs TSMOM+LLM Sharpe on held-out data
- [ ] Target: 70%+ directional accuracy for LLM agents (MarketSenseAI achieved 72.3%)

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

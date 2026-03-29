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
> **Status: Code complete + validated** | Running on local Ollama (no API keys) | 22/22 Phase 1 tests passing

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

### Validation — Functional (Complete)
- [x] Run full graph on SPY — all agents produce signals, combiner output is reasonable
  - TSMOM: LONG (s=+0.50, c=0.75) | Indicator: LONG (s=+0.40, c=0.60) | Pattern: SHORT (s=-0.80, c=0.80)
  - Combined: LONG (s=+0.23, c=0.72) — correctly handled conflicting signals via weighted ensemble
  - All running on local Ollama models (qwen3:14b text, qwen3-vl vision) — no API keys needed
  - SQLite memory logged all signals + decision with outcome tracking

### Validation — Alpha (Pre-Phase 3 Gate)

Round 1 experiments must pass before committing to Rust rewrite. If LLM agents don't meaningfully improve Sharpe over plain TSMOM, rethink the architecture.

**Round 1 — Go/No-Go for Phase 3**
- [ ] **TSMOM-only vs TSMOM+LLM Sharpe comparison** — THE critical experiment. Run both on held-out data (2023-2025), compare Sharpe, return, drawdown. If LLM adds <0.1 Sharpe, architecture needs rethinking.
- [ ] **Agent ablation study** — Run `eval/agent_ablation.py`. Which agents contribute? If Pattern agent adds nothing, don't port it to Rust. Measure marginal Sharpe contribution of each.
- [ ] **Multi-instrument validation** — Run full graph on BTC-USD, ES=F, GC=F (not just SPY). Crypto and commodities behave very differently from equities.
- [ ] **Directional accuracy** — Run `eval/backtest_llm.py` on each LLM agent. Target: 70%+ (MarketSenseAI achieved 72.3%).

**Round 2 — Optimization (if Round 1 passes)**
- [ ] **Debate on/off comparison** — Does bull/bear debate improve decisions vs pure numeric combiner? Justify the 2 extra LLM calls per decision.
- [ ] **Memory effectiveness** — Run 50+ sequential decisions on one instrument. Does SQLite memory injection improve win rate vs memoryless? If not, simplify before porting.
- [ ] **Model quality comparison** — Same eval with GPT-4o / Claude Sonnet vs local Ollama. Quantify the quality gap to know if local backtest results reflect production.
- [ ] **Prompt sensitivity analysis** — Tweak prompts (reorder CoT steps, change wording), re-run eval. If results swing wildly, prompts are fragile and need hardening.
- [ ] **Latency profiling** — End-to-end time for one full graph execution. Matters for IBKR live trading — if 30+ seconds, can't react to fast markets.
- [ ] **Cost per decision** — Actual token usage × pricing for one full cycle. Project monthly bill at scale (multiple instruments, daily).

---

## Phase 3: Rust Rewrite + IBKR Execution
> **Status: Not started** | Full Rust implementation with live IBKR trading

Merges the previous Phase 3 (paper trading), Phase 4 (Rust port), and adds IBKR execution. Single Rust binary: `tokio` for async, `rig-core` for LLM, `ibapi` for execution, `polars` for data, `rusqlite` for memory.

### Crate Ecosystem

| Dependency | Crate | Purpose |
|---|---|---|
| Async runtime | `tokio` | Fan-out parallelism, IBKR streaming |
| DataFrames | `polars` | OHLCV data, feature computation |
| Numeric | `ndarray` | Volatility, rolling stats |
| LLM client | `rig-core` | OpenAI, Anthropic, Ollama |
| IBKR API | `ibapi` | TWS connection, orders, market data |
| Technical analysis | `ta` | RSI, MACD, Bollinger, EMA |
| Database | `rusqlite` | Agent memory, decision logs, order logs |
| Charting | `plotters` | Candlestick charts for vision agents |
| CLI | `clap` | CLI interface |
| Config | `config` | Layered config from TOML + env |
| HTTP | `reqwest` | IBKR Client Portal REST fallback |
| Logging | `tracing` | Structured logging |
| Errors | `anyhow` + `thiserror` | Application + library errors |
| Serialization | `serde` + `serde_json` | Config, signals, prompts |

### Phase 3a: Core Infrastructure (Week 1-2)
- [ ] Scaffold `Cargo.toml` with all dependencies
- [ ] `src/core/signal.rs` — Signal struct + Direction enum
- [ ] `src/core/portfolio.rs` — Position, Order, Fill, AccountSummary
- [ ] `src/core/bar.rs` — OHLCV DataFrame wrapper (polars)
- [ ] `src/config.rs` — Layered config from TOML + env vars
- [ ] `src/memory/store.rs` — SQLite schema (signal_log, decision_log, agent_memory, order_log)
- [ ] `src/agents/traits.rs` — `SignalAgent` trait + `PromptLoader`
- [ ] `src/data/yahoo.rs` — Yahoo Finance fetcher
- [ ] `src/data/universe.rs` — Instrument definitions

### Phase 3b: Signal Agents (Week 2-3)
- [ ] `src/agents/tsmom/` — TSMOM agent + EWMA volatility (pure Rust, no LLM)
- [ ] `src/agents/indicator/` — TA computations via `ta` crate → LLM interpretation
- [ ] `src/agents/pattern/` — Chart rendering via `plotters` → vision LLM
- [ ] `src/agents/trend/` — Trendline fitting + S/R detection → LLM
- [ ] `prompts/` — All prompt templates (ported from Python)

### Phase 3c: Decision Layer (Week 3-4)
- [ ] `src/agents/debate/` — Bull/bear advocates + moderator
- [ ] `src/agents/decision/` — Signal combiner + decision agent
- [ ] `src/agents/risk/` — Position sizing, drawdown limits, veto authority
- [ ] `src/graph/` — Fan-out/fan-in via `tokio::join!` (no framework needed)

### Phase 3d: Execution Layer (Week 4-5)
- [ ] `src/execution/traits.rs` — `ExecutionEngine` trait (submit_order, positions, account_summary, fill_stream)
- [ ] `src/execution/paper.rs` — Local paper simulation
- [ ] `src/execution/ibkr.rs` — IBKR via `ibapi` (paper port 4002, live port 4001)
- [ ] `src/execution/recording.rs` — Wraps any engine, logs all orders/fills to SQLite
- [ ] `src/data/ibkr_feed.rs` — IBKR real-time + historical market data

### Phase 3e: Evaluation + CLI (Week 5-6)
- [ ] `src/eval/backtest_llm.rs` — Historical agent evaluation
- [ ] `src/eval/agent_ablation.rs` — Per-agent contribution analysis
- [ ] `src/eval/metrics.rs` — Sharpe, Sortino, Calmar, directional accuracy
- [ ] `src/main.rs` — CLI via `clap` (backtest, paper-trade, live-trade modes)
- [ ] Integration tests: paper trading round-trip, full graph signal → order flow

### Key Design Decisions

**Graph orchestration:** `tokio::join!` for parallel fan-out, no framework needed:
```rust
let signals: Vec<Signal> = {
    let futures: Vec<_> = agents.iter().map(|a| a.generate_signal(bars, memory)).collect();
    futures::future::join_all(futures).await.into_iter().filter_map(|r| r.ok()).collect()
};
```

**Execution engine trait:** Same interface for paper/IBKR/recording — switch via config:
```toml
[execution]
engine = "paper"   # or "ibkr" or "recording"
[execution.ibkr]
port = 4002        # 4002=paper, 4001=live — one config change
```

**Recording engine:** Decorator pattern wrapping any engine + logging to SQLite. All paper/live trades get full audit trail.

**Cost control:** Prompts loaded from `.md` files at runtime (no recompile). Per-agent model selection in config. Use Haiku/mini for backtesting, Opus for live.

### Rust Graph Framework Research (Previous)

| Crate | Verdict |
|-------|---------|
| [langchain-rust](https://github.com/Abraxas-365/langchain-rust) | No graph orchestration. LLM API calls only. |
| [rs-graph-llm](https://github.com/a-agmon/rs-graph-llm) | Most promising but no parallel fan-out. |
| [rrag-graph](https://docs.rs/rrag-graph/latest/rrag_graph/) | v0.1.0-alpha. Not production-ready. |
| [langgraph-api](https://crates.io/crates/langgraph-api) | Just an API client, not a graph engine. |

**Verdict:** Skip graph frameworks. `tokio::join!` + pattern matching covers our needs.

---

## Phase 4: Extensions
> **Status: Not started**

### New Agents & Strategies
- [ ] Cross-sectional momentum, mean reversion, carry agents
- [ ] **Chronos Agent** *(JOURNAL.md §O)* — Zero-shot time series forecasting via Amazon's Chronos foundation model. Tokenizes price series into probabilistic forecasts. Especially valuable for new instruments lacking TSMOM lookback history.
- [ ] **AlphaGen Agent** *(JOURNAL.md §F, §J)* — Self-improving alpha discovery loop. LLM generates mathematical factor code, runs it through `BacktestEngine`, evaluates Sharpe, iteratively refines. Promotes winners to production.
- [ ] **Clifford TSMOM** *(JOURNAL.md §R)* — Geometric algebra-enhanced momentum. Replace scalar momentum (`price_now/price_lookback - 1`) with the full geometric product between multi-feature vectors (price, volume, volatility). Inner product captures trend continuation; wedge product (bivector) captures regime changes via structural variation. Anti-symmetric wedge means trend reversals produce sign flips — a natural reversal detector. Sparse rolling shifts at {1, 5, 21, 63} days map to daily/weekly/monthly/quarterly timescales. Research direction — potentially publishable if it outperforms standard TSMOM on regime change detection.

### Risk & Sizing
- [ ] Correlation-aware sizing, drawdown deleveraging
- [ ] **RL-based dynamic agent weighting** *(JOURNAL.md §Q)* — Replace fixed `SignalCombiner` weights with a bandit/PPO agent that learns optimal agent weights per market regime. RL survey shows hybrid methods outperform by 15-20%.

### Infrastructure
- [ ] CCXT crypto provider
- [ ] Alerting (Slack/email)
- [ ] **Local Fin-R1 model via Ollama** *(JOURNAL.md §K)* — 7B model matching GPT-4 on financial reasoning. Replace API calls with local inference for near-zero cost paper trading. Supported by `rig-core` (Rust) and `langchain` (Python) via Ollama.

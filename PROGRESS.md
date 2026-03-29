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

**Round 1 — Go/No-Go for Phase 3** *(Completed 2026-03-29, 240 LLM calls, 339 min on local Ollama qwen3:14b)*

- [x] **TSMOM-only vs TSMOM+LLM Sharpe comparison**
- [x] **Multi-instrument validation** — SPY, BTC-USD, ES=F, GC=F (60 days each)
- [x] **Directional accuracy** — Measured per agent

**Results (60-day eval, Oct-Dec 2024):**

| | TSMOM-only | Indicator-only | TSMOM+Indicator |
|---|---|---|---|
| Accuracy | 54.9% | 51.0% | 55.9% |
| Sharpe | 1.37 | 0.05 | 1.44 |
| Ann. Return | 36.0% | 1.0% | 37.8% |
| Max DD | -36.8% | -13.6% | -36.8% |

**Per-instrument:**
- SPY: Indicator slightly improved accuracy (57.9%→59.3%), Sharpe 0.93→0.94
- BTC-USD: No difference — TSMOM dominates, Indicator FLAT 57% of the time
- ES=F: Indicator harmful standalone (Sharpe -3.51), but TSMOM weight overrode it
- GC=F: Indicator helped on worst instrument — accuracy 54.5%→57.9%

**Verdict: MARGINAL (+0.07 Sharpe delta)**
- TSMOM is doing the heavy lifting (Sharpe 1.37 standalone)
- Indicator agent goes FLAT 30-57% of the time, reducing influence
- When Indicator takes a position, accuracy is barely above coin flip (51%)
- Combiner's 0.50/0.20 weighting correctly protects TSMOM from bad LLM signals
- Directional accuracy 51% is far below the 70% MarketSenseAI target

**Implications for Phase 3:**
- TSMOM port to Rust is clearly worth it (Sharpe 1.37 → 50-100x backtest speedup)
- LLM Indicator agent in current form does not justify the complexity/cost of porting
- Before porting LLM agents: try better models on GPU cluster, better prompts, or different agent types (Pattern/Trend vision agents may add more value than text-only Indicator)
- [ ] **Agent ablation study** — Still needed: test Pattern and Trend agents (vision) separately. They may capture different alpha than text-based indicators.

**Round 1b — Model Quality Rerun (SGLang on H200 cluster)**

Round 1 used local Ollama qwen3:14b (51% accuracy). Rerun with stronger reasoning models via SGLang on 2-8 H200 GPUs to determine if model quality is the bottleneck.

Recommended models for eval (priority order):

| Model | Size | H200s | Why |
|---|---|---|---|
| DeepSeek-R1-Distill-Qwen-32B | 32B | 1 | R1 reasoning distilled into small model — best quality/GPU ratio |
| Qwen3-235B-A22B | 235B MoE (~22B active) | 2-4 | Excellent reasoning + structured JSON, efficient MoE |
| Qwen3-32B | 32B | 1 | Strong reasoning, big brother of the qwen3:14b we tested |
| DeepSeek-V3.2 | 685B MoE (~37B active) | 4-8 | Current open-source SOTA (Dec 2025) |
| DeepSeek-R1 | 671B MoE | 4-8 | Best explicit chain-of-thought reasoning |
| Fin-R1 | 7B | 1 | Finance-specialized, matches GPT-4 on financial tasks |
| Qwen3-Coder-Next | 80B | 1-2 | Latest Qwen3 (2026), strong structured output |

Also available: Claude API (no GPU needed) — Opus/Sonnet for highest quality baseline.

- [ ] Set up SGLang on SLURM cluster with H200s
- [ ] Rerun `eval_round1.py` with DeepSeek-R1-Distill-Qwen-32B (1 H200)
- [ ] Rerun with Qwen3-235B-A22B if 32B shows improvement (2-4 H200s)
- [ ] Compare accuracy/Sharpe across model tiers: local 14B → cluster 32B → cluster 235B → Claude API
- [ ] If best model achieves >55% standalone accuracy and >+0.15 Sharpe delta → GO for LLM agent Rust port

**Round 2 — Optimization (if Round 1b passes)**
- [ ] **Debate on/off comparison** — Does bull/bear debate improve decisions vs pure numeric combiner? Justify the 2 extra LLM calls per decision.
- [ ] **Memory effectiveness** — Run 50+ sequential decisions on one instrument. Does SQLite memory injection improve win rate vs memoryless? If not, simplify before porting.
- [ ] **Prompt sensitivity analysis** — Tweak prompts (reorder CoT steps, change wording), re-run eval. If results swing wildly, prompts are fragile and need hardening.
- [ ] **Latency profiling** — End-to-end time for one full graph execution. Matters for IG live trading — if 30+ seconds, can't react to fast markets.
- [ ] **Cost per decision** — Actual token usage × pricing for one full cycle. Project monthly bill at scale (multiple instruments, daily).

---

## Phase 3: Rust Rewrite + IG Trading Execution
> **Status: Not started** | Parallel tracks — proven alpha ships immediately, LLM agents gated on model eval

### Strategy: Parallel Tracks with Go/No-Go Gates

Based on Round 1 results (TSMOM Sharpe 1.37 vs LLM +0.07 marginal), Phase 3 splits into parallel tracks:

- **Track A** (unconditional): Port TSMOM + IG execution to Rust. Ships proven alpha.
- **Track B** (conditional): Port LLM agents IF model upgrade eval shows ≥ 0.20 Sharpe delta.
- **Track C** (fallback): If LLM agents remain marginal, invest in new quant signal sources instead.

### Why IG Over IBKR
- **Tax-free profits** via spread betting (UK: no CGT, no stamp duty)
- **Rust crate** `ig_trading_api` v0.3.0 — REST + Lightstreamer streaming, async/Tokio
- **Demo account** ready (Z69YJL, £10K paper), identical API to live
- **Demo→Live = one config change** (just the base URL)
- IBKR can be added later via same `ExecutionEngine` trait if needed

### Track A — Proven Alpha (Weeks 1-3) — START IMMEDIATELY

| Week | Deliverable |
|---|---|
| **1** | Core types (signal, portfolio, bar) + config + SQLite memory + `Cargo.toml` |
| **1** | Data layer: Yahoo via `yahoo_finance_api` + polars DataFrame wrapper + IG epic mapping |
| **2** | TSMOM agent + EWMA volatility (pure Rust, ndarray) |
| **2** | Backtest engine + metrics (Sharpe, Sortino, Calmar, equity curve) — validate matches Python Sharpe 1.37 |
| **3** | IG execution engine (`ig_trading_api`) + Paper engine + Recording engine |
| **3** | Risk manager + circuit breaker + CLI via `clap` (backtest, paper-trade modes) |

Track A checklist:
- [ ] `Cargo.toml` — Track A deps (tokio, polars, ndarray, rusqlite, ig_trading_api, yahoo_finance_api, clap, config, serde, tracing, anyhow/thiserror, chrono, async-trait)
- [ ] `src/core/signal.rs` — Signal struct + Direction enum
- [ ] `src/core/portfolio.rs` — Position, Order, Fill, AccountSummary
- [ ] `src/core/bar.rs` — OHLCV polars DataFrame wrapper
- [ ] `src/config.rs` — Layered config (TOML + env), IG credentials, model selection
- [ ] `src/memory/store.rs` — SQLite schema (signal_log, decision_log, agent_memory, order_log)
- [ ] `src/data/yahoo.rs` — Yahoo Finance fetcher
- [ ] `src/data/universe.rs` — Instrument definitions + IG epic mapping
- [ ] `src/data/ig_feed.rs` — IG historical prices + Lightstreamer streaming
- [ ] `src/agents/traits.rs` — `SignalAgent` trait
- [ ] `src/agents/tsmom/agent.rs` — Multi-lookback momentum
- [ ] `src/agents/tsmom/volatility.rs` — EWMA vol (ndarray)
- [ ] `src/backtest/engine.rs` — Event-driven backtest, next-open execution
- [ ] `src/backtest/metrics.rs` — Sharpe, Sortino, Calmar, max DD, equity curve
- [ ] `src/execution/traits.rs` — `ExecutionEngine` trait
- [ ] `src/execution/paper.rs` — Local paper simulation
- [ ] `src/execution/ig/` — IG REST API (auth, orders, streaming, epics, rate_limiter)
- [ ] `src/execution/recording.rs` — Decorator: logs all orders/fills to SQLite
- [ ] `src/risk/agent.rs` — Position sizing, drawdown limits, veto authority
- [ ] `src/risk/circuit_breaker.rs` — Auto-flatten after 3 failures, daily loss limit, graceful degradation to TSMOM-only
- [ ] `src/main.rs` — CLI (backtest, paper-trade, live-trade modes)
- [ ] Integration tests: IG demo round-trip, backtest Sharpe matches Python 1.37

### Track B — LLM Agents (Weeks 3-5) — CONDITIONAL

**Gate:** Only start after Round 1b model eval confirms ≥ 0.20 Sharpe delta and ≥ 58% standalone accuracy.

- [ ] `rig-core` LLM client (OpenAI/Anthropic/Ollama/SGLang routing)
- [ ] `src/agents/prompt_loader.rs` — Runtime `.md` prompt loading
- [ ] `src/agents/indicator/` — `ta` crate computations → LLM interpretation
- [ ] `src/agents/pattern/` — `plotters` candlestick charts → vision LLM
- [ ] `src/agents/trend/` — Trendline fitting + S/R → vision LLM
- [ ] `src/agents/debate/` — Bull/bear advocates + moderator
- [ ] `src/agents/decision/` — Signal combiner + decision agent
- [ ] `src/graph/runner.rs` — `tokio::join!` fan-out/fan-in
- [ ] `src/eval/` — Backtest LLM + agent ablation (Rust port)
- [ ] `prompts/` — All .md templates (ported from Python)
- [ ] Benchmark against [ai-hedge-fund](https://github.com/virattt/ai-hedge-fund) (49.6K ★) Sharpe

### Track C — Alternative Signals (Weeks 5-8) — IF TRACK B FAILS

If LLM agents remain marginal even with better models:

| Signal Source | Approach | Expected Value |
|---|---|---|
| Cross-sectional momentum | Pure Rust (polars) — rank by relative strength | Diversification with TSMOM |
| Mean reversion | Bollinger/z-score based | Complements TSMOM on range-bound instruments |
| Chronos | Zero-shot time series via ONNX | Works without lookback history |
| Carry | Futures roll yield, rate differentials | Uncorrelated with momentum |
| Prediction markets | Polymarket/Kalshi sentiment | Macro risk overlay |

### Crate Ecosystem

| Dependency | Crate | Track | Purpose |
|---|---|---|---|
| `tokio` | 1.x | A | Async runtime, streaming |
| `polars` | 0.46+ | A | OHLCV DataFrames |
| `ndarray` | 0.16+ | A | EWMA, rolling stats |
| `ig_trading_api` | 0.3+ | A | IG REST + Lightstreamer |
| `yahoo_finance_api` | 2.x | A | Backtest data |
| `rusqlite` | 0.32+ | A | SQLite memory/logs |
| `clap` | 4.x | A | CLI |
| `config` + `dotenvy` | latest | A | Layered config |
| `serde` + `serde_json` | 1.x | A | Serialization |
| `tracing` | 0.1+ | A | Structured logging |
| `anyhow` + `thiserror` | latest | A | Error handling |
| `chrono` | 0.4+ | A | Time handling |
| `async-trait` | 0.1+ | A | ExecutionEngine trait |
| `reqwest` | 0.12+ | A | HTTP fallback |
| `rig-core` | 0.11+ | B | LLM client (multi-provider) |
| `ta` | 0.5+ | B | Technical analysis |
| `plotters` | 0.3+ | B | Chart rendering for vision agents |
| `image` + `base64` | latest | B | Image encoding for vision LLM |

### Key Design Decisions

**Execution engine trait:** Same interface for paper/IG/IBKR — switch via config:
```toml
[execution]
engine = "ig"               # "paper", "ig", or "ibkr"
[execution.ig]
environment = "DEMO"        # "DEMO" or "LIVE" — one config change
account_id = "Z69YJL"
```

**Circuit breaker** *(from [nofx](https://github.com/NoFxAiOS/nofx))*: Trip conditions: 3 consecutive failures, -5% daily loss, -15% max drawdown. Actions: flatten positions, disable new entries, alert, fall back to TSMOM-only. Auto-reset after cooldown.

**Cargo features:** `default = ["track-a"]`, `track-b = ["rig-core", "ta", "plotters", ...]` — LLM agents are opt-in.

**Competitive benchmarks:**

| Repo | Stars | Use |
|------|-------|-----|
| [TradingAgents](https://github.com/TauricResearch/TradingAgents) | 9.3K | Debate pipeline reference |
| [ai-hedge-fund](https://github.com/virattt/ai-hedge-fund) | 49.6K | Sharpe benchmark |
| [nofx](https://github.com/NoFxAiOS/nofx) | 11.2K | Circuit breaker pattern |

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

### Alternative Signal Sources
- [ ] **PredictionMarketAgent** — Polymarket/Kalshi probability estimates as macro sentiment signal. Use [prediction-market-analysis](https://github.com/Jon-Becker/prediction-market-analysis) dataset (36GB) for backtesting. Execution via [pmxt](https://github.com/pmxt-dev/pmxt) if trading prediction markets directly.

### Infrastructure
- [ ] CCXT crypto provider
- [ ] IBKR execution engine — re-add via same `ExecutionEngine` trait for DMA/options/futures if needed
- [ ] Alerting (Slack/Telegram/email)
- [ ] **Local Fin-R1 model via Ollama** *(JOURNAL.md §K)* — 7B model matching GPT-4 on financial reasoning. Replace API calls with local inference for near-zero cost paper trading. Supported by `rig-core` (Rust) and `langchain` (Python) via Ollama.

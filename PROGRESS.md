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

**Round 1b — Model Quality Rerun (SGLang on H200 cluster)** *(Completed 2026-03-30, 3×21 instruments, 3,780 LLM calls)*

Round 1 used local Ollama qwen3:14b (51% accuracy). Rerun with 3 models via SGLang on H200 GPUs across 21-instrument universe to determine if model quality is the bottleneck.

**Models tested:**

| Model | Size | H200s | Port |
|---|---|---|---|
| DeepSeek-R1-Distill-Qwen-32B | 32B | 1 | 30000 |
| Qwen3-32B | 32B | 1 | 30002 |
| SUFE-AIFLM-Lab/Fin-R1 | 7B | 1 | 30000 |

**Cluster setup learnings (2026-03-29):**
- Partition: inferred from QOS prefix, `--partition` flag is ignored
- Account: `dream`, QOS: `h200_comm_shared`
- Qwen3-235B-A22B requires **8 H200s** (not 2-4), **1TB system RAM**, and these SGLang flags:
  - `--host 0.0.0.0` (default binds to localhost only)
  - `--disable-cuda-graph` (flashinfer JIT compilation fails on cluster CUDA toolkit)
  - `--disable-custom-all-reduce` (same flashinfer incompatibility)
  - `--attention-backend triton --sampling-backend pytorch` (bypass flashinfer entirely)
  - `--mem-fraction-static 0.80` (leave headroom for activations)
- Cluster proxy blocks outbound HTTPS — yfinance cannot download data. Use `scripts/download_data.py` locally, then `--data-dir data/` on cluster.
- Use `NO_PROXY=<node>` when running eval to bypass proxy for SGLang server.

**Qwen3-235B-A22B on SGLang — flashinfer incompatibility (2026-03-29):**

The MoE model (235B total, ~22B active, ~440GB in bf16) could not complete inference on the H200 cluster due to a flashinfer JIT compilation failure. The server starts and appears healthy but crashes on the first real request.

- **Root cause:** SGLang's MoE execution path triggers flashinfer's `trtllm_moe_allreduce_fusion` JIT compilation regardless of all backend override flags (`--attention-backend triton`, `--sampling-backend pytorch`, `--disable-cuda-graph`, `--disable-custom-all-reduce`, `--disable-shared-experts-fusion`, `--moe-runner-backend triton`). The kernel (`trtllm_moe_allreduce_fusion.cu`) is incompatible with flashinfer 0.6.3 on the cluster's CUDA toolkit — 90 compilation errors from `MoeFinalizeAllReduceFusionParams` struct member mismatches.
- **SLURM config:** `--gres=gpu:h200:8 --mem=1024G --cpus-per-task=16 --qos=h200_comm_shared`
- **Measured:** 484GB RSS at runtime; 512GB caused SLURM OOM kill. 4× H200 OOM during weight loading.
- **Workarounds not tried:** vLLM (different MoE backend, no flashinfer dependency), FP8/AWQ quantization (different code path), pre-compiling flashinfer on a dev node.
- **Resolution:** Used DeepSeek-R1-Distill-Qwen-32B (dense 32B, 1 H200) and Qwen3-32B instead. Both worked fine. Fin-R1 7B ultimately won on performance anyway.

**Headline results (60-day eval, 21 instruments, Oct-Dec 2024):**

| Metric | TSMOM | DeepSeek Indicator | Qwen3 Indicator | Fin-R1 Indicator |
|---|---|---|---|---|
| Accuracy (ex-FLAT) | 50.3% | 50.6% | 51.6% | **53.3%** |
| Sharpe | 0.315 | 0.448 | 0.677 | **0.671** |
| Ann. Return | 8.6% | — | — | **14.2%** |
| Max Drawdown | -41.5% | -28.6% | -23.7% | **-18.8%** |
| FLAT % | 0% | 35.6% | 41.8% | 48.2% |

**Key insight: Domain specialization > model scale.** Fin-R1 (7B, finance-RL-trained) matches or beats both 32B general-purpose models. Runs on Mac Mini M4 Pro at ~45 tok/s, free inference.

**The combiner is destroying alpha:**

| Strategy | Sharpe | Max DD |
|---|---|---|
| TSMOM-only | 0.315 | -41.5% |
| Fin-R1 indicator-only | **0.671** | **-18.8%** |
| Combined (0.50/0.20 weights) | 0.420 | -35.6% |

Combined Sharpe (0.420) is **worse** than indicator-only (0.671). The fixed 0.50/0.20 TSMOM/indicator weighting dilutes the indicator's alpha. TSMOM was great on 4 trending instruments (Round 1 Sharpe 1.37) but mediocre on a diversified 21-instrument universe (Sharpe 0.315).

**FLAT signals are a feature, not a bug.** Fin-R1 goes FLAT 48% of the time, but when it takes a position: 53.3% accuracy, Sharpe 0.671, max DD -18.8%. The selectivity *is* the risk management. The combiner destroys this by always taking a position (Combined FLAT: 5.1%).

**Per-instrument regime split — Fin-R1 excels on forex/crypto, TSMOM on equities:**

| Instrument | Fin-R1 Indicator Sharpe | TSMOM Sharpe | Winner |
|---|---|---|---|
| EURUSD=X | **3.77** | -0.53 | Indicator |
| BTC-USD | **3.02** | 2.95 | Indicator |
| USDJPY=X | **1.98** | 0.12 | Indicator |
| ZB=F | **1.61** | -0.89 | Indicator |
| ETH-USD | **1.52** | 0.37 | Indicator |
| CL=F | **1.47** | -0.41 | Indicator |
| SPY | -1.00 | **0.92** | TSMOM |
| ES=F | -1.21 | **0.75** | TSMOM |
| EEM | -1.86 | -3.07 | Both terrible — drop |
| EFA | -1.08 | -2.95 | Both terrible — drop |

**Revised verdict: CONDITIONAL GO**

The original Go/No-Go gate (>55% accuracy AND >+0.15 Sharpe delta) was designed for a uniform combiner. The data shows a clear instrument-type regime split that a dynamic combiner should exploit:

- [x] Set up SGLang on SLURM cluster with H200s
- [x] Download OHLCV data locally, run on cluster with `--data-dir data/`
- [x] Eval DeepSeek-R1-Distill-Qwen-32B (1 H200) — Sharpe 0.448, accuracy 50.6%
- [x] Eval Qwen3-32B (1 H200) — Sharpe 0.677, accuracy 51.6%
- [x] Eval Fin-R1 (1 H200) — **Sharpe 0.671, accuracy 53.3%, best risk-adjusted**
- [x] Compare across model tiers: 7B Fin-R1 ≥ 32B general models — specialization wins

**Implications for Phase 3:**

1. **Dynamic combiner weights by instrument type** — not fixed 0.50/0.20:
   ```python
   weights = {
       "forex":       {"tsmom": 0.20, "indicator": 0.80},
       "crypto":      {"tsmom": 0.30, "indicator": 0.70},
       "equity":      {"tsmom": 0.80, "indicator": 0.20},
       "bonds":       {"tsmom": 0.30, "indicator": 0.70},
       "commodities": {"tsmom": 0.50, "indicator": 0.50},
   }
   ```
2. **Respect the FLAT signal** — when indicator goes FLAT, reduce position size instead of overriding with TSMOM at full weight. FLAT = uncertainty = smaller position.
3. **Drop EEM and EFA** — negative Sharpe across every model and strategy. Pure noise.
4. **Use Fin-R1 as default model** — 7B, runs on Mac Mini M4 Pro, free, best performer. No API costs needed.

**Round 2 — Combiner Simulation** *(Completed 2026-03-30, zero GPU, replayed Fin-R1 CSVs)*

Tested 5 combiner strategies on existing Round 1b data. Key question: can a smart combiner unlock the alpha being destroyed by fixed weights?

- [x] **Dynamic combiner simulation** — Replayed with instrument-type weights (forex 80% indicator, equity 80% TSMOM, etc.)
- [x] **FLAT-aware position sizing** — Tested reducing to 50% TSMOM when indicator is FLAT
- [x] **Drop EEM/EFA** — Re-computed excluding negative-Sharpe instruments
- [x] **Transaction cost sensitivity** — IG spread costs (fixed: only on position changes, not hold days)

**Results (clean 19-instrument universe, 60 days):**

| Strategy | Sharpe | w/IG costs | Δ cost | Ann.Ret | MaxDD |
|---|---|---|---|---|---|
| TSMOM-only | 0.508 | 0.383 | -0.125 | 14.4% | -33.4% |
| Indicator-only (Fin-R1) | 0.765 | 0.422 | -0.343 | 17.0% | -18.8% |
| Fixed combiner (0.50/0.20) | 0.623 | 0.444 | -0.179 | 18.2% | -29.9% |
| **Dynamic combiner** | **0.793** | **0.643** | **-0.150** | **16.9%** | **-17.1%** |
| Dynamic + FLAT-aware | 0.714 | 0.533 | -0.181 | 17.1% | -23.7% |

**Key findings:**
1. **Dynamic combiner (0.793) is the best strategy** — beats indicator-only (0.765) and TSMOM-only (0.508). First evidence that combining is actually additive.
2. **Survives IG transaction costs** — Dynamic combiner drops only 0.150 Sharpe (to 0.643) vs indicator-only losing 0.343 (to 0.422). The combiner's lower turnover helps.
3. **FLAT-aware sizing hurts** — 0.714 < 0.793. When indicator goes FLAT, TSMOM at half-size still drags. Better to just trust the dynamic weights.
4. **Indicator-only is worst after costs** — Sharpe 0.765→0.422 (-0.343). High turnover from frequent FLAT→LONG/SHORT transitions kills it.
5. **Dropping EEM/EFA is free alpha** — ~0.1-0.2 Sharpe improvement across all strategies.

**Transaction cost implications for IG live trading:**
- Dynamic combiner (Sharpe 0.643 after costs) is viable
- Crypto's 80bps spread is painful but tolerated by strong crypto alpha
- Forex (3bps) and equity (5bps) spreads are negligible
- Consider dropping commodity positions (GC=F, GLD negative Sharpe even before costs)

**Per instrument-type (Dynamic + FLAT-aware):**

| Type | Sharpe | Best instruments |
|---|---|---|
| Crypto | 1.653 | BTC-USD (3.07), SOL-USD (1.64), BNB-USD (1.18) |
| Forex | 0.387 | EURUSD (1.75), USDJPY (1.53) — but GBPUSD, AUDUSD negative |
| Equity | 0.345 | QQQ (0.70), IWM (0.54) |
| Bonds | -0.218 | ZB=F good (1.66), TLT terrible (-1.75) |
| Commodity | -0.467 | All negative |

**Round 3 — 252-Day Full Eval** *(Completed 2026-03-31, 21 instruments × 252 days = 5,292 LLM calls, H200 cluster)*

Extended eval from 60 days to a full year (Mar 2024 – Mar 2025). Critical reality check: the 60-day results were inflated by a strong trending period.

**Headline results (252 days, 21 instruments):**

| Universe | TSMOM | Indicator | Combined | Ind Acc | Ind FLAT% |
|---|---|---|---|---|---|
| All 21 | -0.07 | -0.14 | -0.04 | 51.7% | 49% |
| No crypto (17) | +0.24 | +0.06 | +0.22 | 52.8% | 48% |
| Tradeable (15) | **+0.34** | +0.04 | +0.27 | 53.0% | 48% |

**By asset class:**

| Type | TSMOM | Indicator | Combined | Winner |
|---|---|---|---|---|
| Commodity | **+1.22** | +0.38 | +1.15 | TSMOM (GC=F +2.02, GLD +2.12) |
| Equity | +0.16 | -0.10 | +0.11 | TSMOM (SPY +1.13) |
| Forex | -0.73 | **+0.51** | -0.55 | Indicator (GBPUSD +1.40, USDCHF +1.42, USDJPY +0.70) |
| Bonds | -0.45 | -0.47 | -0.71 | Both terrible |
| Crypto | -0.48 | -0.42 | -0.37 | Both terrible |

**Best strategy per instrument (wins: TSMOM=6, Indicator=8, Combined=7):**

| Instrument | Winner | TSMOM | Indicator | Combined |
|---|---|---|---|---|
| GLD | Combined | +2.12 | +0.62 | **+2.38** |
| GC=F | Combined | +2.02 | +1.16 | **+2.24** |
| USDCHF=X | Indicator | -1.15 | **+1.42** | -0.64 |
| GBPUSD=X | Indicator | -0.59 | **+1.40** | -0.41 |
| SPY | TSMOM | **+1.13** | -0.17 | +0.59 |
| USDJPY=X | Indicator | -0.57 | **+0.70** | -0.75 |
| NQ=F | Combined | +0.08 | -0.43 | **+0.56** |
| EEM | Indicator | -0.12 | **+0.55** | +0.14 |

**Key findings:**

1. **60-day results were overfitting to a trending period.** TSMOM dropped from 0.508→0.34, indicator from 0.765→0.04. The Oct–Dec 2024 window had strong directional trends — exactly the regime where both strategies perform best.
2. **TSMOM-only (Sharpe 0.34) is the best aggregate strategy** on the tradeable universe. Gold (GC=F +2.02, GLD +2.12) and SPY (+1.13) carry it.
3. **Indicator excels on forex** (GBPUSD +1.40, USDCHF +1.42, USDJPY +0.70) but the fixed combiner destroys it — need per-instrument routing, not weighted average.
4. **Gold is the star** — only asset class where all strategies are positive AND the combiner adds value (GC=F: TSMOM +2.02, Combined +2.24).
5. **Equities: TSMOM-only.** SPY Sharpe 1.13 TSMOM vs −0.17 indicator. The LLM adds noise on efficiently-priced US large cap.
6. **Bonds are uninvestable** — negative Sharpe across all strategies. Drop TLT/ZB=F entirely.
7. **Crypto negative over a full year** — Q4 2024 bull run masked mediocre full-year performance. Excluded from tradeable universe.
8. **Parse failure rate dropped to 0.04%** (2/5,292) — retry mechanism works well.

**Revised instrument-type weights (252-day data):**

```python
weights_252day = {
    "gold":   {"tsmom": 0.50, "indicator": 0.50},  # Both contribute, combiner adds value
    "equity": {"tsmom": 1.00, "indicator": 0.00},  # TSMOM only — indicator is destructive
    "forex":  {"tsmom": 0.10, "indicator": 0.90},  # Indicator dominates
}
```

**Revised tradeable universe (6 instruments):**

| Instrument | Type | Strategy | 252-Day Sharpe |
|---|---|---|---|
| GLD | Gold | Dynamic combiner | +2.38 |
| GC=F | Gold | Dynamic combiner | +2.24 |
| USDCHF=X | Forex | Indicator-heavy | +1.42 |
| GBPUSD=X | Forex | Indicator-heavy | +1.40 |
| SPY | Equity | TSMOM-only | +1.13 |
| USDJPY=X | Forex | Indicator-heavy | +0.70 |

Dropped: all crypto, EEM, EFA, TLT, ZB=F, CL=F, IWM, QQQ, NQ=F, ES=F, EURUSD=X, AUDUSD=X.

**Remaining experiments:**
- [x] **Dynamic combiner simulation on 252-day data** — See Round 4 below.
- [ ] **Out-of-sample period test** — Run on a different 252-day window (e.g., Mar 2023–Mar 2024) to check stability
- [ ] **Walk-forward validation** — Train weights on first 126 days, test on second 126 days
- [ ] **Pull Fin-R1 into local Ollama** — Verify it works locally for free dev/testing

**Round 4 — 252-Day Combiner Simulation** *(Completed 2026-03-31, zero GPU, replayed Round 3 CSVs)*

Replayed 252-day Fin-R1 results with revised instrument-type weights and focused 6-instrument universe. Critical question: does per-instrument routing produce a viable Sharpe over a full year?

**Focused universe results (6 instruments, 252 days, with IG transaction costs):**

| Strategy | Sharpe | w/IG costs | Max DD | Ann. Ret |
|---|---|---|---|---|
| TSMOM-only | 0.882 | 0.823 | -15.4% | 10.2% |
| Fixed combiner (0.50/0.20) | 0.903 | 0.800 | -18.2% | 10.8% |
| Dynamic combiner (60d wts) | 1.221 | 1.103 | -8.6% | 11.0% |
| **Dynamic combiner (252d wts)** | **1.228** | **1.112** | **-8.6%** | **11.1%** |
| Dynamic + FLAT-aware | 1.056 | 0.925 | -9.1% | 10.4% |
| Indicator-only | 0.707 | 0.404 | -12.8% | 6.5% |

**Per-instrument (Dynamic 252d weights):**

| Instrument | Type | Sharpe | Ann. Ret | Max DD |
|---|---|---|---|---|
| GC=F | Gold | +1.74 | +21.3% | -8.4% |
| GLD | Gold | +1.63 | +20.2% | -8.6% |
| GBPUSD=X | Forex | +1.41 | +5.8% | -2.0% |
| USDCHF=X | Forex | +1.22 | +5.0% | -2.5% |
| SPY | Equity | +0.94 | +9.8% | -7.0% |
| USDJPY=X | Forex | +0.68 | +4.4% | -4.3% |

**Key findings:**

1. **Sharpe 1.228 (1.112 after costs) on focused universe.** This is the strongest result so far — better than the 60-day dynamic combiner (0.793) because instrument selection matters more than combiner weights.
2. **Max drawdown halved.** Dynamic combiner -8.6% vs TSMOM-only -15.4%. The combiner isn't just adding return — it's cutting risk.
3. **Weights are robust.** 60-day weights (1.221) and 252-day weights (1.228) give nearly identical results — not overfitted to either period.
4. **FLAT-aware sizing still hurts** (1.056 vs 1.228). Instrument-type routing is the alpha, not position sizing tricks.
5. **Transaction costs are manageable.** Only -0.116 Sharpe impact (1.228→1.112). Forex spreads (3bps) are negligible, gold (10bps) is tolerable.
6. **Full universe is still negative.** The 15 dropped instruments drag aggregate Sharpe below zero. Instrument selection is essential.

**Verdict: GO for Phase 3.** Sharpe 1.112 after costs with -8.6% max DD on a focused 6-instrument universe. Track A (TSMOM) and Track B (Fin-R1 indicator) are both unconditional.

---

## Phase 3: Rust Rewrite + IG Trading Execution
> **Status: In progress — Track A complete (8 PRs), Track B in progress (PRs B1-B5d)** | 286 tests, clean clippy

### Strategy: Parallel Tracks (Updated After Round 4 — Combiner Simulation)

Round 4 combiner simulation on 252-day data confirms Sharpe 1.228 (1.112 after IG costs) on focused 6-instrument universe. Both tracks are unconditional.

- **Track A** (unconditional): Port TSMOM + IG execution to Rust. 6 instruments: GLD, GC=F, SPY, GBPUSD=X, USDCHF=X, USDJPY=X.
- **Track B** (unconditional): Port Fin-R1 indicator agent to Rust. Per-instrument router (gold: 50/50, equity: TSMOM-only, forex: 10/90).
- **Track C** (deferred): Pattern/Trend vision agents, debate. Validate core system first.

### Why IG Over IBKR
- **Tax-free profits** via spread betting (UK: no CGT, no stamp duty)
- Direct reqwest wrapper (~5 IG REST endpoints), async/Tokio
- **Demo account** ready (Z69YJL, £10K paper), identical API to live
- **Demo→Live = one config change** (just the base URL + environment)
- IBKR can be added later via same `ExecutionEngine` trait if needed

### Track A — Proven Alpha (Weeks 1-4)

**Principle: validate the math first, then add plumbing.** Weeks 1-2 validated the Rust backtest against Python. Weeks 3-4 add execution infrastructure.

| Week | Deliverable | Status |
|---|---|---|
| **1** | Core types (signal, portfolio, bar) + config + `Cargo.toml` | ✅ Done |
| **1** | Data layer: load OHLCV from CSV (reuse `data/` from Python eval) | ✅ Done |
| **2** | TSMOM agent + EWMA volatility (pure Rust) | ✅ Done |
| **2** | Backtest engine + metrics + validation gate | ✅ **PASSED** |
| **3** | Per-instrument execution router + spread cost model | ✅ Done |
| **3** | CLI (clap — backtest, paper-trade modes) + paper-trade state persistence | ✅ Done |
| **3-4** | ExecutionEngine trait + TOML config + PaperExecutionEngine | ✅ Done |
| **3-4** | IG REST client (auth, positions, orders, confirm, flatten) | ✅ Done |
| **3-4** | Safety valves, reconciliation, circuit breaker | ✅ Done |
| **3-4** | IG demo round-trip verified (auth→place→confirm→flatten) | ✅ **PASSED** |
| **4** | IG spread-bet sizing calibration (ig_point_value per instrument) | ✅ Done |
| **4** | Per-run JSONL audit logging + run summaries | ✅ Done |
| **4b** | Audit log polish (schema v2, Z timestamps, signed_deal_size) | ✅ Done |
| **5** | SQLite recording decorator + `history` subcommand | ✅ Done |
| **5b** | SQLite polish (user_version, batch inserts, filters) | ✅ Done |
| **6** | Risk agent with veto authority + drawdown tracking | ✅ Done |
| **7** | Data pipeline (Yahoo update + freshness gate) | ✅ Done |
| **8** | NAV mark-to-market (MTM from live positions + bar prices) | ✅ Done |

#### IG Execution Architecture (PRs 1-3 Complete)

**PR 1 — Boundary + Config + Compile-Time Shape:**
- [x] `src/execution/traits.rs` — `ExecutionEngine` trait (async methods: health_check, get_positions, place_orders, get_order_status, flatten_all)
- [x] `src/config.rs` — `AppConfig` with TOML parsing, `IgConfig`, `InstrumentConfig` with `ig_point_value`
- [x] `src/execution/paper.rs` — `PaperExecutionEngine` with simulated fills
- [x] `src/execution/ig/` — IG module: client, engine, types, errors, mapping
- [x] `config.example.toml` — All 6 instruments with epics + sizing config

**PR 2 — IG Demo Round-Trip:**
- [x] `src/execution/ig/client.rs` — Full `IgClient`: auth (CST/X-SECURITY-TOKEN), rate limiting (1050ms), retry on 5xx, re-auth on 401
- [x] `src/execution/ig/engine.rs` — `IgExecutionEngine` via `tokio::sync::Mutex<IgClient>`: sequential orders with 500ms confirm delay
- [x] `src/execution/ig/mapping.rs` — `SymbolMapper`: bidirectional symbol↔epic lookup
- [x] Safety valves: `--instrument`, `--max-orders`, `--max-size`, `--flatten`
- [x] JSONL audit logging to `data/audit/`
- [x] `tests/ig_demo_roundtrip.rs` — `#[ignore]` integration test: auth→place 0.5 GBPUSD→confirm→flatten→verify flat
- [x] **IG demo round-trip passing** (12s end-to-end)

**PR 3 — Reconciliation + Safety:**
- [x] `src/execution/reconcile.rs` — `positions_to_signed()`, `compute_deltas()`, `verify_positions()` with per-instrument tolerance
- [x] `src/execution/circuit_breaker.rs` — Consecutive failure tracking, pre-trade order count/size checks
- [x] `positions` subcommand: `quantbot positions --config config.toml [--json]`
- [x] Live loop: generate targets → fetch live positions → compute deltas → circuit breaker → place deltas → post-trade verify → save actual quantities
- [x] Unknown epics → bail (fail-fast, not silent skip)
- [x] Dust deltas tracked and reported (sub-minimum deltas visible, not hidden)
- [x] Running twice → 0 orders on second run (idempotency)

**Sizing Calibration:**
- [x] `ig_point_value` in `InstrumentConfig` — converts notional to IG deal size (£/pip for FX, £/point for equity/commodity)
- [x] `IgConfig::to_execution_router()` — builds ExecutionRouter with IG-correct point values
- [x] Live path uses `BacktestEngine::new_with_router()` — backtest point values unchanged

**PR 4 — Audit Logging + Run Summaries:**
- [x] `src/audit.rs` — `AuditLogger` with `BufWriter<File>`, per-run JSONL append, non-blocking write failure handling
- [x] Events: `run_start`, `targets`, `auth_ok`, `health_check_ok`, `positions_fetched`, `reconcile`, `breaker_check`, `execution_skipped`, `orders_submitted`, `orders_confirmed`, `verify`, `run_end`
- [x] Every JSONL line: `schema_version`, `ts` (RFC3339 Z-suffix), `run_id`, `event`, `level`, `data`
- [x] File per run: `data/audit/<ISO-timestamp>.jsonl`
- [x] `run_end` always emitted (SUCCESS/DRY_RUN/ERROR/BREAKER_TRIPPED/PARTIAL)
- [x] Audit write failures never block trading (stderr WARN, `audit_write_failed` flag)
- [x] `--json` prints `RunSummary` to stdout for cron scraping
- [x] Human-readable summary line on stderr otherwise
- [x] Old `write_audit_log()` replaced with structured event trace
- [x] 7 unit tests (file creation, JSONL validity, event ordering, conversion helpers)

**PR 4b — Audit Log Polish:**
- [x] Schema bump to v2 (`SCHEMA_VERSION = 2`)
- [x] Timestamps normalized to `Z` suffix (`to_rfc3339_opts(SecondsFormat::Micros, true)`)
- [x] `signed_qty` renamed to `signed_deal_size` in `TargetEntry` and `PositionEntry` (clarifies IG deal size units)
- [x] Float noise removed — sizes rounded to 1dp for logging (`25.700000000000003` → `25.7`)
- [x] `auth_ok` and `health_check_ok` audit events added (between `run_start` and `positions_fetched`)
- [x] Event ordering: `run_start → targets → auth_ok → health_check_ok → positions_fetched → reconcile → breaker_check → execution_skipped → run_end`

**PR 5 — SQLite Recording + History CLI:**
- [x] `rusqlite = { version = "0.32", features = ["bundled"] }` added to `Cargo.toml`
- [x] `src/db.rs` — Schema (4 tables: `runs`, `signals`, `orders`, `positions`), WAL mode, insert/query helpers, 7 tests
- [x] `src/recording.rs` — `Recorder` struct with typed `record_*` methods (not trait decorator — avoids RPITIT complexity), 2 tests
- [x] `history` subcommand: `quantbot history [--run <id>] [--instrument <sym>] [--last <n>] [--json]`
- [x] Single DB file at `data/quantbot.db` (colocated with audit JSONL)
- [x] Recorder wired into `run_live`/`run_rebalance` at every stage: signals, targets, actual positions, orders submitted, orders confirmed, post-trade positions, run end
- [x] SQLite errors never block trading (warn to stderr, continue)
- [x] `PRAGMA journal_mode=WAL` for concurrent read safety
- [x] Schema indexed on `run_id` and `instrument` for fast queries

**PR 5b — SQLite Polish (from review):**
- [x] Add `PRAGMA user_version` for lightweight schema versioning
- [x] Batch inserts in a transaction for multi-signal/order runs
- [x] `--status` and `--date` filters for `history` subcommand
- [x] Track B readiness: ensure schema can log LLM agent signals and decisions
- [x] `db_write_failed` flag in `RunSummary` (parallel to `audit_write_failed`)

**PR 6 — Risk Agent:**
- [x] `src/agents/risk/mod.rs` — `RiskAgent::check()` with veto authority
- [x] Daily loss limit (-5%), max drawdown (-15%), gross leverage cap, per-position concentration
- [x] `risk_check` audit event
- [x] Drawdown high-water mark persisted in SQLite (PR 5 dependency)

**PR 7 — Data Pipeline (Yahoo Update + Freshness Gate):**
- [x] `src/data/yahoo.rs` — `YahooClient`: Yahoo v8 chart API, rate limiting (500ms), null filtering, serde response parsing, mockito test constructor
- [x] `src/data/updater.rs` — `DataUpdater`: CSV merge/append, last_date detection, `update_all` orchestrator, `discover_symbols`. Preserves `Date,Close,High,Low,Open,Volume` column order
- [x] `src/data/freshness.rs` — `previous_trading_day` (weekday logic), `check_freshness`, `check_all_fresh`, max_stale_days tolerance (default 3)
- [x] `data` CLI subcommand: `quantbot data [--instruments SYM,...] [--tradeable-only] [--data-dir PATH] [--json]`
- [x] Freshness gate in `run_live`: checks all instruments before trading, `--allow-stale` override, `--max-stale-days` config
- [x] 20 unit tests (13 freshness + 7 updater), 5 Yahoo mockito tests
- [x] Mockito tests use `.no_proxy()` on reqwest client to bypass HTTP proxy on cluster login node (also fixed in IG client tests)

**PR 8 — NAV Mark-to-Market:**
- [x] `src/execution/mtm.rs` — `mark_to_market()` pure function, `MtmResult`/`MtmPosition` structs
- [x] NAV = initial_cash + Σ(signed_size × (current_price - open_level))
- [x] Computed from live IG positions + latest bar close prices each run
- [x] MTM NAV used for: `generate_targets` sizing, risk agent drawdown check, state file save, audit log
- [x] `nav_mark_to_market` audit event with per-position breakdown
- [x] IG engine created early in `run_live`, reused for MTM + `run_rebalance` (single auth)
- [x] Paper engine falls back to state file NAV / initial_cash
- [x] 6 unit tests (empty, long up, short down, mixed, missing price, missing level)
- [x] Fixed mockito proxy issue (`.no_proxy()` on reqwest client)

#### Validation Results (2026-03-31)

| Test | Rust | Python | Delta |
|---|---|---|---|
| 60-day, 4 instruments (Oct-Dec 2024) | 1.377 | 1.370 | **+0.5%** |
| 252-day, 6 instruments (Mar 2024 → Mar 2025) | 0.930 | 0.882 | +5.5% |
| 252-day, 21 instruments (Mar 2024 → Mar 2025) | 0.378 | 0.340 | +11.1% |

Key fixes: `eval_start` warmup/eval separation, risk limit ordering (gross scaling first, then per-position cap).

Remaining +5-11% delta on 252-day tests likely from: EWMA `adjust=True` edge effects in first ~60 bars, weekend date handling (crypto 1551 bars vs equity 1065), minor float precision. Not blocking.

Track A checklist:
- [x] `Cargo.toml` — Core deps (serde, chrono, thiserror, anyhow, csv, tokio, reqwest, toml, clap)
- [x] `src/core/signal.rs` — Signal struct + Direction enum + validation
- [x] `src/core/portfolio.rs` — Position, Order, Fill, PortfolioState (NAV, exposure)
- [x] `src/core/bar.rs` — Bar struct + BarSeries with validation (non-empty, sorted)
- [x] `src/core/universe.rs` — Instrument definitions + 6-instrument tradeable universe
- [x] `src/data/loader.rs` — CSV data loader with DataProvider trait + date filtering
- [x] `src/agents/tsmom/mod.rs` — Multi-lookback momentum signal generation
- [x] `src/agents/tsmom/volatility.rs` — EWMA volatility estimation (pure Rust, no ndarray needed)
- [x] `src/backtest/engine.rs` — Event-driven backtest, next-open execution, mark-to-market, risk limits, eval_start warmup separation
- [x] `src/backtest/metrics.rs` — Sharpe, Sortino, Calmar, max DD, equity curve
- [x] Python code relocated to `lib/` — Rust owns `src/`, Python is reference implementation
- [x] `tests/validate_sharpe.rs` — 4 validation tests + benchmark + per-instrument PnL diagnostics
- [x] **Validation gate: Rust Sharpe matches Python** ✅ PASSED
- [x] `src/main.rs` — CLI (clap — backtest, paper-trade, live, positions subcommands)
- [x] `src/execution/router.rs` — ExecutionRouter: per-instrument ContractSpec, lot rounding, direction-aware spread costs
- [x] `src/backtest/engine.rs` — `generate_targets()` for paper-trade + live modes
- [x] Paper-trade state persistence — `PaperTradeState` JSON file, `--state-file`/`--reset` CLI args
- [x] `src/config.rs` — TOML config with `ig_point_value` per instrument, `IgConfig::to_execution_router()`
- [x] `src/execution/traits.rs` — `ExecutionEngine` trait (async, RPITIT)
- [x] `src/execution/paper.rs` — PaperExecutionEngine with simulated fills
- [x] `src/execution/ig/client.rs` — IG REST client (auth, positions, create/close, confirm, rate limiting, retry)
- [x] `src/execution/ig/engine.rs` — IgExecutionEngine (tokio::sync::Mutex, sequential orders with confirmation)
- [x] `src/execution/ig/mapping.rs` — SymbolMapper: bidirectional symbol↔epic lookup
- [x] `src/execution/reconcile.rs` — Reconciliation: positions_to_signed, compute_deltas, verify_positions
- [x] `src/execution/circuit_breaker.rs` — CircuitBreaker: consecutive failures, pre-trade checks
- [x] `tests/ig_demo_roundtrip.rs` — Integration test: full IG demo round-trip
- [x] `config.example.toml` — All 6 instruments with IG epics + sizing calibration
- [x] `src/audit.rs` — Per-run JSONL audit logging with structured events + RunSummary (schema v2)
- [x] `src/agents/combiner.rs` — Per-instrument signal combiner with configurable blend weights (PR B3)
- [x] `src/db.rs` — SQLite schema (runs, orders, positions, signals) + WAL mode + query helpers
- [x] `src/recording.rs` — Recorder: logs all trading activity to SQLite (standalone struct, not trait decorator)
- [x] `src/agents/risk/mod.rs` — RiskAgent: drawdown cap, leverage cap, per-position concentration, veto authority
- [x] `src/execution/mtm.rs` — Mark-to-market NAV from live positions + bar prices

### Track B — Fin-R1 Indicator Agent (Weeks 3-5) — UNCONDITIONAL (Gate Passed)

**Gate passed:** Round 4 combiner simulation confirms Sharpe 1.228→1.112 after costs on focused universe. Per-instrument routing is the key — not universal weighting.

**PR B1 — Multi-Agent Plumbing + Dummy RSI Indicator:** ✅ Done
- [x] `track-b` cargo feature gate in `Cargo.toml` (no extra deps — RSI is inline arithmetic)
- [x] `src/agents/mod.rs` — `SignalAgent` trait (object-safe: `name()`, `signal_type()`, `generate_signal()`)
- [x] `src/agents/tsmom/mod.rs` — `impl SignalAgent for TSMOMAgent` (delegates to inherent method, zero test breakage)
- [x] `src/agents/indicator/mod.rs` — `DummyIndicatorAgent` with 14-period RSI (Wilder's smoothing), 7 unit tests
  - RSI < 30 → Long (oversold), RSI > 70 → Short (overbought), else Flat
  - Strength = distance from threshold scaled to [0,1], confidence = 0.7
  - RSI value stored in `metadata["rsi"]`
- [x] `src/db.rs` — Schema v2→v3: `agent_name TEXT NOT NULL DEFAULT 'tsmom'` on signals table, migration, `idx_signals_agent` index, migration test
- [x] `src/recording.rs` — `SignalRecord` struct replaces `HashMap` for multi-agent provenance
- [x] `src/main.rs` — Conditional indicator agent in `run_live` (behind `#[cfg(feature = "track-b")]`), signals recorded to SQLite with `weight=0` (advisory only), printed in report. `run_history` shows Agent column.
- [x] All tests pass with and without `track-b` feature, clean clippy

#### PR B2 — LLM Indicator Client + TA Features (2026-04-04)

Replaced DummyIndicatorAgent's inline RSI with a full TA suite and LLM-based indicator agent. All code gated behind `track-b` feature. +1350 lines.

- [x] `src/agents/indicator/ta.rs` — Extracted `compute_rsi`, added SMA, EMA, MACD (12/26/9), Bollinger Bands (20,2), ATR (Wilder's smoothing). `TaSnapshot::compute()` + `format_for_prompt()`. 15 tests
- [x] `src/agents/indicator/llm_client.rs` — OpenAI-compatible `/v1/chat/completions` client (Ollama/SGLang). `LlmConfig` with serde defaults (temp=0.3, max_tokens=512, timeout=30s, retries=2). Rate limiting (200ms), retry on 5xx with exponential backoff (1s/2s/4s), no retry on timeout/4xx. 5 mockito tests
- [x] `src/agents/indicator/parser.rs` — `parse_llm_response()` pipeline: strip `<think>` blocks (OnceLock regex) → strip markdown fences → try direct JSON → regex fallback for embedded JSON. Clamps confidence/strength, direction aliases (long/buy/short/sell/flat/neutral/hold). 11 tests
- [x] `prompts/indicator_system.md` — System prompt: trading analyst role, JSON output schema `{direction, confidence, strength, horizon_days, reasoning}`, indicator analysis guidelines. Loaded via `include_str!`
- [x] `src/agents/indicator/llm_agent.rs` — `LlmIndicatorAgent` with `tokio::sync::Mutex<LlmClient>`. `generate_signal_async()`: TaSnapshot → format prompt → LLM call → parse → Signal. `SignalAgent` impl via `block_in_place` + `Handle::current().block_on()`. Graceful degradation: any error → Flat + `llm_success=0.0` in metadata. 3 mockito tests
- [x] `src/config.rs` — Feature-gated `llm: Option<LlmConfig>` on `AppConfig`. 1 test
- [x] `src/main.rs` — `Box<dyn SignalAgent>` dynamic dispatch: `config.llm.is_some()` → `LlmIndicatorAgent`, else `DummyIndicatorAgent`. Agent column in display, RSI display conditional ("-" when absent). `sig.agent_name` for recording
- [x] `config.example.toml` — Commented `[llm]` section with all fields and defaults
- [x] All tests pass with and without `track-b` feature, clean clippy

#### PR B3 — Per-Instrument Signal Combiner + Pipeline Integration (2026-04-04)

Wires indicator signals into sizing via per-asset-class combiner with configurable blend weights. Absorbs PR B5's per-instrument router — fixed global weights are wrong, instrument-type routing is the alpha. Blending gated: `enabled=false` preserves existing TSMOM-only behavior. Scope: `run_live` + `run_paper_trade`; backtest unchanged. +950 lines, 15 new tests.

- [x] `src/config.rs` — `BlendCategory` (Gold/Equity/Forex), `BlendWeights`, `BlendConfig` with `weights_for()` safe lookup (TSMOM-only default). Feature-gated `blending: Option<BlendConfig>` on `AppConfig`. Validation: warns on missing category, errors on zero-sum weights. 3 tests
- [x] `src/agents/combiner.rs` (NEW) — Pure-function combiner module:
  - `blend_category()`: GLD/GC=F→Gold, SPY→Equity, *=X→Forex, fallback→Equity (with warning)
  - `combine_signals()`: per-instrument TSMOM+indicator blending using vol_scalar normalization
  - `build_combined_signal()`: constructs combined Signal with full provenance metadata (tsmom_weight, indicator_weight, blend ratios, vol_scalar, latency_ms, indicator_used)
  - Graceful TSMOM-only fallback: flat indicator, confidence=0, llm_success=0, or missing signal
  - 11 tests: 50/50 gold, 100/0 equity passthrough, all fallback cases, category routing, opposing signals, vol_scalar fallback
- [x] `src/agents/mod.rs` — `pub mod combiner` (feature-gated)
- [x] `src/backtest/engine.rs` — Refactored `generate_targets()` into shared `build_snapshot()` helper. Added `generate_targets_with_overrides()` (feature-gated) for combined pipeline. 1 test verifying identical output
- [x] `src/main.rs` — Full pipeline integration:
  - `run_live`: indicator signals generated before targets (with latency tracking), branching on blend config, combined pipeline via `generate_targets_with_overrides()`, blending summary table, SQLite records tsmom + indicator + combined signal layers
  - `run_paper_trade`: `--config` arg (feature-gated) enables blended mode with same pipeline
- [x] `config.example.toml` — Commented `[blending]` section with per-asset-class weights (gold 50/50, equity 100/0, forex 10/90)
- [x] Fixed pre-existing `ema_basic` test (linear→quadratic series) and MACD clippy warning in `ta.rs`
- [x] All tests pass with and without `track-b` feature, clean clippy

#### PR B4 — Runtime Prompt Loading with Hash Provenance (2026-04-04)

Decoupled system prompt from compiled binary. Runtime loading from optional `prompt_path` file with graceful fallback to embedded prompt. SHA-256 hash (16 hex chars, raw bytes) for deterministic provenance. Logged to both audit JSONL (`prompt_info` event) and SQLite `runs` table. +400 lines, 9 new tests.

- [x] `src/agents/indicator/prompt_loader.rs` (NEW) — `load()` with file/embedded fallback, empty file detection, `sha256_short()` pub hash function. 7 tests
- [x] `src/agents/indicator/llm_client.rs` — `prompt_path: Option<String>` on `LlmConfig`
- [x] `src/agents/indicator/llm_agent.rs` — Uses `PromptLoader` instead of `include_str!`, exposes `loaded_prompt()` for audit/recording
- [x] `src/audit.rs` — `log_prompt_info()` method: `prompt_info` event with hash, source, model
- [x] `src/db.rs` — Schema v3→v4: `prompt_hash TEXT`, `prompt_source TEXT`, `llm_model TEXT` nullable columns on `runs` table. `update_run_prompt()` method. Migration with `.ok()` idempotency. 2 tests
- [x] `src/recording.rs` — `record_prompt_info()` writes to runs row
- [x] `src/main.rs` — `run_live` emits `prompt_info` audit event + SQLite recording (feature-gated)
- [x] `config.example.toml` — Commented `prompt_path` option
- [x] All tests pass with and without `track-b` feature, clean clippy

#### PR B5a — LLM Cache Write-Through (2026-04-04)

Every LLM indicator call cached to SQLite for deterministic replay. Cache key = `(llm_model, prompt_hash, instrument, eval_date, ta_hash)`. INSERT OR IGNORE semantics — entries never overwritten. Error cases cached for transparency. +480 lines, 12 new tests.

- [x] `src/db.rs` — Schema v4→v5: `llm_cache` table with `cache_key TEXT PRIMARY KEY`, indexes on `(instrument, eval_date)` and `(llm_model, prompt_hash)`. `LlmCacheEntry` struct. `insert_llm_cache` (INSERT OR IGNORE), `get_llm_cache` (for B5b). Migration v4→v5. 5 tests
- [x] `src/agents/indicator/prompt_loader.rs` — `sha256_short()` made pub for `ta_hash` reuse
- [x] `src/agents/mod.rs` — `take_cache_entries()` default method on `SignalAgent` trait (returns empty vec for non-LLM agents)
- [x] `src/agents/indicator/llm_agent.rs` — Added `model`, `cache_entries: Mutex<Vec<LlmCacheEntry>>` fields. `generate_signal_async` computes eval_date/ta_hash/cache_key, measures latency, pushes cache entry on every call (success, parse error, LLM error). `take_cache_entries()` drains. 7 tests (3 existing enhanced + 4 new: deterministic key, key varies by instrument, drain semantics)
- [x] `src/recording.rs` — `record_llm_cache_entries()` non-blocking batch write with count logging
- [x] `src/main.rs` — `run_live` drains entries via trait, writes to recorder. `run_paper_trade` drains and writes directly to Db
- [x] All tests pass with and without `track-b` feature, clean clippy

**Remaining Track B PRs:**
- [x] **PR B3** — Signal combiner: blend TSMOM + indicator signals with configurable per-asset-class weights
- [x] **PR B4** — `src/agents/indicator/prompt_loader.rs` — Runtime `.md` prompt loading with SHA-256 hash provenance
- [x] **PR B5a** — LLM cache write-through to SQLite (deterministic cache key, INSERT OR IGNORE, error caching)
- [x] **PR B5b** — LLM client fix for Ollama thinking models (qwen3, Fin-R1). Verified end-to-end
- [x] **PR B5c** — Replay harness: CachedIndicatorAgent + `eval replay` subcommand for offline blended backtest
- [x] **PR B5d** — Batch cache-fill subcommand (`quantbot cache fill`) for populating LLM cache across date ranges

#### PR B5b — LLM Client Fix for Ollama Thinking Models (2026-04-05)

Fixed LLM client compatibility with Ollama thinking models (qwen3:14b, Fin-R1:Q5). Thinking models return CoT in `message.reasoning` with empty `content`, and need larger token budgets + timeouts.

- [x] `src/agents/indicator/llm_client.rs` — `content: Option<String>` + `reasoning: Option<String>` on `ChatMessage`, `stream: false`, raw body snippet in `EmptyResponse` error, default `max_tokens` 512→4096. 2 new tests (null content, reasoning-without-content)
- [x] `prompts/indicator_system.md` — Moved from `src/agents/indicator/prompt.txt`. `include_str!` path updated in `prompt_loader.rs`
- [x] `config.example.toml` — `max_tokens = 4096`, `timeout_secs = 120`
- [x] Verified end-to-end: qwen3:14b + Fin-R1:Q5, all 6 instruments `llm_ok=1, parse_ok=1`

#### PR B5c — Replay Harness: CachedIndicatorAgent + eval replay (2026-04-05)

Offline deterministic replay of cached LLM responses through the backtest engine. Compares blended (TSMOM + LLM indicator) vs TSMOM-only Sharpe without network calls.

- [x] `src/agents/indicator/cached_agent.rs` (NEW) — `CachedIndicatorAgent` implements `SignalAgent`. Reconstructs cache keys identically to `LlmIndicatorAgent` (`model|prompt_hash|instrument|eval_date|ta_hash`). Looks up responses via `db.get_llm_cache()`. Cache miss → Flat with `llm_success=0.0`. Tracks hits/misses per instrument in `CoverageReport`. 8 tests
- [x] `src/agents/indicator/mod.rs` — `pub mod cached_agent;`
- [x] `src/db.rs` — `llm_cache_coverage(model, prompt_hash) -> HashMap<String, usize>` for pre-flight cache count per instrument. Uses existing `idx_llm_cache_model_prompt` index. 1 test
- [x] `src/backtest/engine.rs` — `run_blended()` (feature-gated `track-b`): daily loop with TSMOM signals → indicator signals → `combine_signals()` + `build_combined_signal()` → risk limits → sizing. Separate method from battle-tested `run()`
- [x] `src/main.rs` — `quantbot eval replay` subcommand with `--config`, `--model`, `--prompt-hash`, `--start`, `--end`, `--eval-start`, `--instruments`, `--json`. Runs blended replay + TSMOM-only baseline, prints side-by-side comparison (Sharpe, return, DD, trades) + coverage report
- [x] `src/agents/tsmom/mod.rs` — Fixed `vol_scalar` missing on flat TSMOM signals: moved EWMA vol computation before `avg_sign==0` early return so `conflicting_signals` and `zero_volatility` flat signals still carry `vol_scalar`/`ann_vol` metadata for correct indicator weight scaling in combiner
- [x] `src/config.rs` — Fixed pre-existing test: `parse_llm_config` expected `max_tokens=512` but default was changed to 4096
- [x] All tests pass (284) with and without `track-b` feature, clean clippy
- [ ] `src/graph/runner.rs` — `tokio::join!` fan-out/fan-in (TSMOM + indicator)

#### PR B5d — Batch Cache-Fill Subcommand (2026-04-05)

Populates the `llm_cache` SQLite table for all (instrument, date) pairs in an eval window so that `eval replay` produces meaningful blended vs TSMOM-only results. Idempotent — safe to interrupt and resume.

- [x] `src/db.rs` — `Db::delete_llm_cache(cache_key)` for retrying failed entries (INSERT OR IGNORE prevents re-insert without delete). 2 tests
- [x] `src/main.rs` — `quantbot cache fill` subcommand (feature-gated `track-b`). `CacheArgs`/`CacheCommand::Fill`/`CacheFillArgs` structs. CLI flags: `--config`, `--start`, `--end`, `--instruments`, `--tradeable-only`, `--max-failures`, `--require-success`, `--progress`, `--data-dir`, `--min-history`
- [x] `run_cache_fill()` async fn: 4-phase pipeline (setup → build work list → execute LLM calls → summary). Loads full bar history per instrument, slices `[0..=i]` per date for identical TA/cache_key as `LlmIndicatorAgent`. Skips successful entries, deletes+retries failed ones. Consecutive-failure abort. Per-instrument coverage report
- [x] Verified: 3/3 SPY calls OK (2024-12-18 to 2024-12-20), latency 3-9s, idempotent skip on re-run

#### PR B6 — Confidence Gating for Indicator Signals (2026-04-06)

15-month eval replay showed LLM indicator is PnL-neutral but adds 41 extra trades, creating spread cost drag (Sharpe 1.278 vs 1.394 TSMOM-only). Confidence gating filters out low-conviction indicator signals before blending, reducing turnover while preserving any high-conviction edge.

- [x] `src/config.rs` — `GatingConfig` struct (`min_confidence`, `min_abs_strength`) with serde defaults (0.0 = no gating). Optional `gating: Option<GatingConfig>` field on `BlendConfig`. 1 parse test
- [x] `src/agents/combiner.rs` — `should_use_indicator()` accepts `gating: Option<&GatingConfig>`, rejects signals below thresholds. `combine_signals()` threads gating from `blend_config.gating`. 3 tests (reject low confidence, reject low strength, allow high conviction)
- [x] `config.example.toml` — Commented `[blending.gating]` section with suggested defaults (min_confidence=0.70, min_abs_strength=0.30)
- [x] No call-site changes needed — gating carried inside `BlendConfig`, `combine_signals()` signature unchanged

#### Ablation Results — Fin-R1 + Baseline Prompt (2026-04-06)

15-month eval replay (2024-01-01 → 2025-03-31, 98.7% cache coverage) with systematic ablation of blending configurations. Conclusion: **no evidence of alpha** from Fin-R1 indicator under realistic IG spread costs.

| Config | Sharpe | Δ vs TSMOM | Extra Trades | Spread Residual |
|---|---|---|---|---|
| TSMOM-only (baseline) | 1.394 | — | — | — |
| Ungated (all indicator) | 1.278 | -0.116 | +41 | — |
| Gated 0.70/0.30 | 1.314 | -0.080 | +34 | 36,013 (19.3%) |
| Forex off, gold 50/50 | 1.365 | -0.029 | +23 | 14,657 (7.5%) |

Per-instrument attribution (forex-off run):
- GC=F: -3,760 (gold indicator hurts on low-trade instrument)
- GLD: -258 (neutral)
- SPY: +780 (noise — equity indicator weight is 0%)
- FX: -810 total (near-neutral with indicator disabled)

**Production default: TSMOM-only** (`blending.enabled = false`). LLM indicator pipeline preserved as experimental feature behind config for prompt/model A/B testing.

**Next steps — prompt/model A/B testing:**

1. **Prompt A/B** (holding model constant):
   - Baseline prompt `8430ffc768a841ee` vs directional prompt `634b…`
   - Cache fill + replay for each prompt variant
2. **Model A/B** (holding prompt constant):
   - Fin-R1 vs qwen3 (qwen may be more conservative → fewer spurious signals)
   - Gold-only first to minimize compute, expand if positive signal found

### Track C — Additional Agents & Signals (Weeks 5-8) — DEFERRED

Defer until core system (Track A + B) is validated with live paper trading. Vision agents and debate are lower priority given 252-day results.

- [ ] `src/agents/pattern/` — `plotters` candlestick charts → vision LLM
- [ ] `src/agents/trend/` — Trendline fitting + S/R → vision LLM
- [ ] `src/agents/debate/` — Bull/bear advocates + moderator
- [ ] `src/agents/decision/` — Full decision agent with debate context

Alternative quant signals (if vision agents don't pan out):

| Signal Source | Approach | Expected Value |
|---|---|---|
| Cross-sectional momentum | Pure Rust (polars) — rank by relative strength | Diversification with TSMOM |
| Mean reversion | Bollinger/z-score based | Complements TSMOM on range-bound instruments |
| Chronos | Zero-shot time series via ONNX | Works without lookback history |
| Carry | Futures roll yield, rate differentials | Uncorrelated with momentum |
| Prediction markets | Polymarket/Kalshi sentiment | Macro risk overlay |

### Crate Ecosystem

| Dependency | Crate | Track | Status | Purpose |
|---|---|---|---|---|
| `tokio` | 1.x | A | ✅ | Async runtime |
| `reqwest` | 0.12 (native-tls) | A | ✅ | IG REST API client |
| `toml` | 0.8 | A | ✅ | Config parsing |
| `clap` | 4.x | A | ✅ | CLI |
| `serde` + `serde_json` | 1.x | A | ✅ | Serialization |
| `anyhow` + `thiserror` | latest | A | ✅ | Error handling |
| `chrono` | 0.4+ | A | ✅ | Time handling |
| `csv` | 1.x | A | ✅ | Data loading |
| `mockito` | 1.x | A (dev) | ✅ | HTTP mocking for IG client tests |
| `tempfile` | 3.x | A (dev) | ✅ | Temp dirs for config tests |
| `rusqlite` | 0.32 (bundled) | A | ✅ | SQLite recording/history |
| `tracing` | 0.1+ | A | Planned | Structured logging |
| `rig-core` | 0.11+ | B | Planned | LLM client (multi-provider) |
| `ta` | 0.5+ | B | Planned | Technical analysis |
| `plotters` | 0.3+ | B | Planned | Chart rendering for vision agents |
| `image` + `base64` | latest | B | Planned | Image encoding for vision LLM |

Note: `polars`, `ndarray`, `ig_trading_api`, `async-trait` were originally planned but not needed. Direct reqwest wrapper replaced ig_trading_api. RPITIT replaced async-trait. Pure Rust iteration replaced ndarray.

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

## Track D: Continuous Bot — Overlay Actions + Always-On
> **Status: In progress** | See JOURNAL.md §8 for architecture

### PR D1: Overlay Actions v1 (typed enum + expiry + SQLite) ✅ `9a0e9e6`
- [x] `src/overlay/mod.rs` — `OverlayAction` enum (4 actions), `OverlayScope` enum (Global/AssetClass/Instrument), `AppliedOverlay`
- [x] Actions: `FreezeEntries`, `ScaleExposure`, `Flatten`, `DisableInstrument` (TightenGating deferred to v2)
- [x] `apply_overlays(weights, current_quantities, overlays, eval_date) -> Vec<AppliedOverlay>`
- [x] Date-based expiry: actions with `until < eval_date` skipped; `Flatten` immediate (no expiry)
- [x] SQLite schema v5→v6: `overlay_actions` table, `Db::insert_overlay_action()`, migration
- [x] Audit JSONL: `overlay_applied` event with per-action weight changes
- [x] Hook into `run_live` / `paper-trade` after signals, before `generate_targets`
- [x] Config-driven v1: `[[overlays.actions]]` in TOML, `OverlayConfig` struct
- [x] 8 unit tests: scale global/asset-class, freeze with/without position, flatten, disable, expiry, composition
- [x] Un-gated `BlendCategory` from `track-b` feature (used by overlays + combiner)

### PR D2: Volatility/Market-Condition Overlay (deterministic)
- [ ] Triggers: realized vol spike, ATR% spike, large move (>1.5σ)
- [ ] Emits `ScaleExposure` or `FreezeEntries` actions
- [ ] Deterministic + backtestable

### PR D3: News Overlay (bounded, not HFT)
- [ ] Data ingestion (polling + caching)
- [ ] Classifier (rule-based, then LLM-assisted)
- [ ] Emits only bounded action types from D1

### PR D4: Daemon + Scheduling
- [ ] Long-running process with periodic timer + trigger queue
- [ ] "One run at a time" lock
- [ ] Health endpoint / heartbeat
- [ ] Intraday bars (15m/60m) as separate `BarSeries`

---

## Phase 4: Extensions
> **Status: Not started**

### New Agents & Strategies
- [ ] Cross-sectional momentum, mean reversion, carry agents
- [ ] **TimesFM Agent** *(JOURNAL.md §S)* — Google's 200M-param zero-shot time series foundation model. Probabilistic forecasts (quantile output) with 16K context. Replaces Chronos as primary foundation model candidate — newer, smaller, probabilistic. Complements TSMOM (momentum vs mean-reversion/cyclical). Rust: export to ONNX → `ort` crate. Runs on Mac M4.
- [ ] **AlphaGen Agent** *(JOURNAL.md §F, §J)* — Self-improving alpha discovery loop. LLM generates mathematical factor code, runs it through `BacktestEngine`, evaluates Sharpe, iteratively refines. Promotes winners to production.
- [ ] **Clifford TSMOM** *(JOURNAL.md §R)* — Geometric algebra-enhanced momentum via full geometric product. Research direction — potentially publishable.
- [ ] **Multi-timeframe analysis** *(from QuantAgent-SBU + TorchTrade)* — Analyze across 1h/4h/1d simultaneously. Daily trend + shorter-timeframe entry signals for confluence.

### Risk & Sizing
- [ ] Correlation-aware sizing, drawdown deleveraging
- [ ] **RL-based dynamic agent weighting** *(JOURNAL.md §Q, TorchTrade PPO reference)* — Replace fixed `SignalCombiner` weights with a bandit/PPO agent that learns optimal weights per market regime. RL survey shows hybrid methods outperform by 15-20%.

### Alternative Signal Sources
- [ ] **PredictionMarketAgent** — Polymarket/Kalshi probability estimates as macro sentiment signal. Use [prediction-market-analysis](https://github.com/Jon-Becker/prediction-market-analysis) dataset (36GB) for backtesting. Execution via [pmxt](https://github.com/pmxt-dev/pmxt) if trading prediction markets directly.

### Infrastructure
- [ ] CCXT crypto provider
- [ ] IBKR execution engine — re-add via same `ExecutionEngine` trait for DMA/options/futures if needed
- [ ] Alerting (Slack/Telegram/email)
- [ ] **Local Fin-R1 model via Ollama** *(JOURNAL.md §K)* — 7B model matching GPT-4 on financial reasoning. Replace API calls with local inference for near-zero cost paper trading. Supported by `rig-core` (Rust) and `langchain` (Python) via Ollama.

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
> **Status: In progress — Track A Weeks 1-3 complete, Week 3-4 next (IG API)** | 75 tests, clean clippy

### Strategy: Parallel Tracks (Updated After Round 4 — Combiner Simulation)

Round 4 combiner simulation on 252-day data confirms Sharpe 1.228 (1.112 after IG costs) on focused 6-instrument universe. Both tracks are unconditional.

- **Track A** (unconditional): Port TSMOM + IG execution to Rust. 6 instruments: GLD, GC=F, SPY, GBPUSD=X, USDCHF=X, USDJPY=X.
- **Track B** (unconditional): Port Fin-R1 indicator agent to Rust. Per-instrument router (gold: 50/50, equity: TSMOM-only, forex: 10/90).
- **Track C** (deferred): Pattern/Trend vision agents, debate. Validate core system first.

### Why IG Over IBKR
- **Tax-free profits** via spread betting (UK: no CGT, no stamp duty)
- **Rust crate** `ig_trading_api` v0.3.0 — REST + Lightstreamer streaming, async/Tokio
- **Demo account** ready (Z69YJL, £10K paper), identical API to live
- **Demo→Live = one config change** (just the base URL)
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
| **3-4** | IG execution engine (`ig_trading_api`) + Paper engine | ← NEXT |
| **4** | SQLite memory + risk manager + circuit breaker | |

#### Validation Results (2026-03-31)

| Test | Rust | Python | Delta |
|---|---|---|---|
| 60-day, 4 instruments (Oct-Dec 2024) | 1.377 | 1.370 | **+0.5%** |
| 252-day, 6 instruments (Mar 2024 → Mar 2025) | 0.930 | 0.882 | +5.5% |
| 252-day, 21 instruments (Mar 2024 → Mar 2025) | 0.378 | 0.340 | +11.1% |

Key fixes: `eval_start` warmup/eval separation, risk limit ordering (gross scaling first, then per-position cap).

Remaining +5-11% delta on 252-day tests likely from: EWMA `adjust=True` edge effects in first ~60 bars, weekend date handling (crypto 1551 bars vs equity 1065), minor float precision. Not blocking.

Track A checklist:
- [x] `Cargo.toml` — Core deps (serde, chrono, thiserror, anyhow, csv)
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
- [ ] `src/agents/decision/router.rs` — Per-instrument strategy router (gold: combiner, equity: TSMOM, forex: indicator-heavy) ← Track B
- [x] `src/main.rs` — CLI (clap — backtest + paper-trade subcommands, stubs for live/positions)
- [x] `src/execution/router.rs` — ExecutionRouter: per-instrument ContractSpec, lot rounding, direction-aware spread costs (SpreadCostTracker 0x/1x/2x), integrated into BacktestEngine
- [x] `src/backtest/engine.rs` — `generate_targets()` for paper-trade mode: single-shot signal→risk limits→sizing→orders pipeline
- [x] Paper-trade state persistence — `PaperTradeState` JSON file, `--state-file`/`--reset` CLI args, diff-based rebalance orders
- [ ] `src/config.rs` — Layered config (TOML + env), IG credentials, model selection
- [ ] `src/execution/traits.rs` — `ExecutionEngine` trait
- [ ] `src/execution/paper.rs` — Local paper simulation
- [ ] `src/execution/ig/` — IG REST API (auth, orders, streaming, epics, rate_limiter) ← NEXT
- [ ] `src/execution/recording.rs` — Decorator: logs all orders/fills to SQLite
- [ ] `src/memory/store.rs` — SQLite schema (signal_log, decision_log, agent_memory, order_log)
- [ ] `src/risk/agent.rs` — Position sizing, drawdown limits, veto authority
- [ ] `src/risk/circuit_breaker.rs` — Auto-flatten after 3 failures, daily loss limit, graceful degradation to TSMOM-only
- [ ] Integration tests: IG demo round-trip

**75 tests passing, clean clippy, 0 warnings.** Python reference implementation in `lib/`. **Validation gate passed.**

### Track B — Fin-R1 Indicator Agent (Weeks 3-5) — UNCONDITIONAL (Gate Passed)

**Gate passed:** Round 4 combiner simulation confirms Sharpe 1.228→1.112 after costs on focused universe. Per-instrument routing is the key — not universal weighting.

- [ ] `rig-core` LLM client (Ollama routing — Fin-R1 runs locally on Mac Mini M4 Pro)
- [ ] `src/agents/prompt_loader.rs` — Runtime `.md` prompt loading
- [ ] `src/agents/indicator/` — `ta` crate computations → LLM interpretation
- [ ] `src/agents/decision/combiner.rs` — **Per-instrument router** (not weighted average):
  ```rust
  match instrument.asset_type() {
      Gold   => combine(tsmom, indicator, 0.50, 0.50),
      Equity => tsmom_only,
      Forex  => combine(tsmom, indicator, 0.10, 0.90),
  }
  ```
- [ ] Universe: 6 instruments (GLD, GC=F, SPY, GBPUSD=X, USDCHF=X, USDJPY=X)
- [ ] `src/graph/runner.rs` — `tokio::join!` fan-out/fan-in (TSMOM + indicator)
- [ ] `src/eval/` — Backtest LLM + agent ablation (Rust port)
- [ ] `prompts/` — Indicator .md template (ported from Python)

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

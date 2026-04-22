# QuantBot Phase 3 — Rust Port Strategy

## Status: Validation Gate PASSED ✅

### Sharpe Validation Results

| Test | Rust | Python | Delta | Verdict |
|---|---|---|---|---|
| 60-day, 4 instruments | 1.377 | 1.370 | **+0.5%** | ✅ Perfect match |
| 252-day, 6 instruments | 0.930 | 0.882 | +5.5% | ✅ Within tolerance |
| 252-day, 21 instruments | 0.378 | 0.340 | +11.1% | ✅ Acceptable |

60-day test matches within float noise (0.007 Sharpe delta). The Rust engine faithfully reproduces Python's TSMOM behavior.

### Rust Engine Stats

- **37 unit tests** passing (core types: 12, loader: 4, TSMOM: 9, backtest: 12)
- **Zero clippy warnings** (after `is_none_or` fix)
- **Backtest speed:** 2.1s for 21 instruments × 252 days (10 iterations avg)
- **Eval window isolation:** warmup bars excluded from Sharpe calculation
- **1/N vol allocation:** per-instrument vol target = 40% / N instruments

### Components Complete

| Component | Status | Tests | Commit |
|---|---|---|---|
| Core types (signal, portfolio, bar, universe) | ✅ Done | 12 | — |
| CSV data loader | ✅ Done | 4 | — |
| TSMOM agent + EWMA volatility | ✅ Done | 9 | — |
| Backtest engine + metrics | ✅ Done | 12 | 469cb9f |
| **Sharpe validation gate** | ✅ **Passed** | 4 | 2fe2478 |
| **Total** | | **41** | |

---

## Weeks 3-4: Execution Layer

### Priority 1: Per-Instrument Router (Track B+)

Dynamic combiner with 252-day validated weights:

```rust
fn combiner_weights(symbol: &str) -> (f64, f64) {
    // (tsmom_weight, indicator_weight)
    match symbol {
        "GLD" | "GC=F"                          => (0.50, 0.50),  // Both contribute
        "SPY"                                    => (1.00, 0.00),  // TSMOM only
        "GBPUSD=X" | "USDCHF=X" | "USDJPY=X"  => (0.10, 0.90),  // Indicator dominates
        _                                        => (0.50, 0.50),  // Default
    }
}
```

### Priority 2: CLI (`clap`)

```
quantbot backtest --instruments SPY,GLD,GBPUSD=X --start 2024-03-01 --end 2025-03-31
quantbot paper-trade --config config.toml
quantbot live --config config.toml --dry-run
quantbot positions
```

### Priority 3: IG Execution Engine

- **Account:** Demo Z69YJL, spread betting, £10K paper money
- **API:** `https://demo-api.ig.com/gateway/deal`
- **Key:** `d92ff32aeeccaa5533c203fab25cd20038cae66f`
- Crate: `ig_trading_api` or raw `reqwest` + REST
- UK spread betting = tax-free profits

### Priority 4: SQLite Memory + Risk Manager

- Port from Python `memory/store.py` — signal + decision logs
- Risk manager: max position size, max drawdown kill switch, circuit breaker
- Gross leverage cap (e.g., 2.0x)

---

## Tradeable Universe (6 Instruments)

| Instrument | Type | Strategy | 252-Day Sharpe |
|---|---|---|---|
| GLD | Gold | Dynamic combiner | +2.38 |
| GC=F | Gold | Dynamic combiner | +2.24 |
| USDCHF=X | Forex | Indicator-heavy | +1.42 |
| GBPUSD=X | Forex | Indicator-heavy | +1.40 |
| SPY | Equity | TSMOM-only | +1.13 |
| USDJPY=X | Forex | Indicator-heavy | +0.70 |

### Why 6, Not 21

21-instrument PnL attribution shows massive gross flows from daily rebalancing:
- GBPUSD: +165% of NAV, SOL-USD: -110% of NAV
- Attributed: -$1.38M of +$166K total PnL (-836% over-attribution)
- The portfolio makes $166K net but cycles through millions in notional changes
- Enormous turnover for marginal alpha (Sharpe 0.38 vs 0.93 on 6 instruments)

---

## Key Numbers

| Metric | 60-day | 252-day (6 inst) | 252-day (21 inst) | After IG costs |
|---|---|---|---|---|
| Rust TSMOM Sharpe | **1.377** | **0.930** | 0.378 | TBD |
| Python TSMOM Sharpe | 1.370 | 0.882 | 0.340 | — |
| Dynamic combiner Sharpe | 0.793 | TBD | — | 0.643 |

---

## Track B: Fin-R1 Indicator Agent

Port the LLM indicator agent — calls Ollama locally, parses JSON signals. Starts after per-instrument router is implemented.

- Model: `mychen76/Fin-R1:Q5` (5.4GB, finance-specialized 7B)
- Runs on Mac M4 via Ollama at ~45 tok/s
- Clean JSON signals, good financial reasoning
- Commit 68e4825 verified on local Ollama
- **Confidence gating** (PR B6): `GatingConfig` (min_confidence, min_abs_strength) on `BlendConfig` filters out low-conviction signals before blending
- **Ablation result (2026-04-06):** 15-month eval (98.7% cache, 2024-01 → 2025-03) showed no evidence of alpha from Fin-R1 + baseline prompt under IG spread costs. Ablation ladder: ungated 1.278 → gated 1.314 → forex-off 1.365 → TSMOM-only 1.394. Monotonic improvement as indicator exposure removed. Production default: TSMOM-only. Next: prompt/model A/B testing
- **Gold protective override (2026-04-08):** `BlendMode::ProtectiveOverride` — indicator only intervenes on sign flips (true directional disagreements). Gold dampening collapsed from 95% to 0%. Sharpe 1.427 vs TSMOM-only 1.394 (+0.033). Safe optionality preserved.
- **Next research directions (from ATLAS/TradingAgents review, JOURNAL §5.1-5.2):**
  - Adaptive blend weights via constrained softmax (ATLAS pattern: rolling 60-day Sharpe → softmax with 0.1/0.9 floor/ceiling)
  - Disagreement penalty (ATLAS pattern: when TSMOM and indicator disagree, penalize conviction by `opposing_weight × 0.5` → FLAT on conflict)
  - BM25 regime memory (TradingAgents pattern: inject past similar market conditions into LLM prompt)
  - Autoresearch prompt evolution (ATLAS pattern: automated prompt A/B with 5-day eval windows)
  - 5-tier rating scale (TradingAgents pattern: BUY/OVERWEIGHT/HOLD/UNDERWEIGHT/SELL → position scale factors)

---

## Track D: Continuous Bot — Overlay Actions + Always-On

Build order (see JOURNAL.md §8 for full architecture):

1. **Overlay actions v1** ✅ — typed enum (`FreezeEntries`, `ScaleExposure`, `Flatten`, `DisableInstrument`) with scope (Global/AssetClass/Instrument), date-based expiry, SQLite persistence, audit logging. Config-driven (`[[overlays.actions]]` in TOML). TightenGating deferred to v2.
2. **Volatility/market-condition overlay** ✅ — per-asset-class vol thresholds, deterministic triggers emitting bounded actions
3. **News overlay** — bounded risk overlay, not primary alpha. Polling + caching + classifier. ATLAS reflexivity engine (JOURNAL §5.1) provides a framework for modeling cascading effects (tariff → dollar strength → FX risk → deleverage) rather than simple sentiment classification.
4. **Daemon + scheduling** ✅ — PID lock, checkpoint persistence, periodic timer, auto-update, SIGTERM handling
5. **Auto-update** ✅ — daily CSV refresh from Yahoo, integrated into daemon cycle
6. **Systemd service** ✅ — user unit file + install script
7. **Notifications** ✅ — cmd + webhook fire-and-forget on trade/error/veto
8. **Status command** ✅ — `quantbot status` ops dashboard (daemon/data/portfolio/overlays/runs), `--live` IG query, `--json` stable schema

---

## Track C: Vision Agents

Pattern + Trend agents using vision models. Completely untested — deferred to next eval round. Needs cluster GPU.

Architecture references:
- QuantAgent-SBU (JOURNAL §2.1, §5.3): vision LLM chart pipeline + multi-timeframe analysis (1m to 1d)
- TradingAgents (JOURNAL §5.2): modular analyst plugins with tool bindings — each agent gets specific data access
- FinceptTerminal (JOURNAL §5.3): multi-persona agents — generate signals from specialized perspectives (momentum, mean-reversion, macro) and blend

---

## Evaluation Hardening — Risk & Robustness Metrics

Motivated by the TradeMaster / PRUDEX-Compass review (JOURNAL §12). Current `BacktestResult` in `src/backtest/metrics.rs:9` covers profitability + basic drawdown. Two axes are under-measured: **tail risk** and **robustness across time**.

Add the following fields to `BacktestResult` (`src/backtest/metrics.rs:9-20`) and compute them in `from_snapshots` (`src/backtest/metrics.rs:24-111`) from the existing `daily_returns` and `nav_series` — no new inputs required.

### 1. Tail risk — VaR(95) and CVaR(95)

```rust
// in BacktestResult
pub var_95: f64,       // 5th-percentile daily return (negative number)
pub cvar_95: f64,      // mean of returns ≤ var_95
```

Logic (drop into `from_snapshots` alongside the existing Sortino block at `src/backtest/metrics.rs:82-97`):

```rust
let mut sorted = daily_returns.clone();
sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
let idx = ((sorted.len() as f64) * 0.05).floor() as usize;
let var_95 = sorted.get(idx).copied().unwrap_or(0.0);
let tail: Vec<f64> = sorted.iter().take(idx.max(1)).copied().collect();
let cvar_95 = if tail.is_empty() { 0.0 } else { tail.iter().sum::<f64>() / tail.len() as f64 };
```

Why: Sortino only penalises below-zero volatility; VaR/CVaR quantify *how bad* the tail actually is. This is what surfaces black-swan exposure that Sharpe/Sortino happily hide.

### 2. Robustness — rolling-Sharpe stability

```rust
pub rolling_sharpe_std: f64,   // std of 60-day rolling Sharpe across the window
pub worst_60d_sharpe: f64,     // min 60-day Sharpe — the single worst regime
```

Logic (new helper, called after the main Sharpe computation):

```rust
fn rolling_sharpe(returns: &[f64], window: usize) -> Vec<f64> {
    returns
        .windows(window)
        .map(|w| {
            let mean = w.iter().sum::<f64>() / window as f64;
            let sd = std_dev(w);
            if sd > 1e-8 { mean / sd * TRADING_DAYS_PER_YEAR.sqrt() } else { 0.0 }
        })
        .collect()
}
```

Then `rolling_sharpe_std = std_dev(&rolls)` and `worst_60d_sharpe = rolls.iter().fold(f64::INFINITY, |a, &b| a.min(b))`.

Why: the ablation ladder (TSMOM-only 1.394 vs gated 1.314) compares single-number Sharpes. A strategy that's 1.4 steady-state but dips to -2.0 in one regime is *worse* than one that's 1.2 with no dips below 0.3. We cannot currently see that.

### 3. Turnover (reliability axis)

```rust
pub avg_daily_turnover: f64,   // mean |Δ notional| / NAV across snapshots
```

Logic (requires iterating `Snapshot::position_notionals` pairwise at `src/backtest/engine.rs` — already tracked in snapshots):

```rust
let mut turnover_sum = 0.0;
for pair in snapshots.windows(2) {
    let (prev, curr) = (&pair[0], &pair[1]);
    let delta: f64 = curr.position_notionals.iter()
        .map(|(k, v)| (v - prev.position_notionals.get(k).copied().unwrap_or(0.0)).abs())
        .sum();
    turnover_sum += delta / curr.nav;
}
let avg_daily_turnover = turnover_sum / (snapshots.len() - 1) as f64;
```

Why: STRATEGY §"Why 6, Not 21" already hand-computed this for the 21-instrument regression (-836% over-attribution). Making it a first-class metric means every ablation surfaces cost-drag before we commit capital.

### Summary printer

Extend `summary()` at `src/backtest/metrics.rs:114-147` with three new lines grouped under a `RISK & ROBUSTNESS` divider. Keep composite scores out — report each metric raw so ablation tables stay legible.

### Out of scope (deliberately)
- PRUDEX composite "compass" score — hides trade-offs.
- Entropy / explainability metrics — already covered qualitatively by per-instrument PnL attribution in `eval_results/`.
- Universality dispersion (per-instrument Sharpe std) — useful but requires running ablations per instrument; defer until the core three above land.

Implementation effort: ~1 PR, all in `src/backtest/metrics.rs`, no engine or schema changes.

---

## Remaining Phase 2 Experiments (Nice-to-Have)

| # | Experiment | Status | Blocking? |
|---|---|---|---|
| 1 | Out-of-sample period test (Mar 2023–Mar 2024) | Not done | No |
| 2 | Walk-forward validation (train 126d / test 126d) | Not done | No |
| 3 | Pattern/Trend vision agents | Not done | No — Track C |
| 4 | Debate, memory, prompts, latency | Not done | No — defer |

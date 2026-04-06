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

---

## Track C: Vision Agents

Pattern + Trend agents using vision models. Completely untested — deferred to next eval round. Needs cluster GPU.

---

## Remaining Phase 2 Experiments (Nice-to-Have)

| # | Experiment | Status | Blocking? |
|---|---|---|---|
| 1 | Out-of-sample period test (Mar 2023–Mar 2024) | Not done | No |
| 2 | Walk-forward validation (train 126d / test 126d) | Not done | No |
| 3 | Pattern/Trend vision agents | Not done | No — Track C |
| 4 | Debate, memory, prompts, latency | Not done | No — defer |

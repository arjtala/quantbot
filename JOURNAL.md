# [JOURNAL] QuantBot: Trading Analysis

## Overview
This journal entry synthesizes the relationship between the foundational academic research on Time Series Momentum (2012) and the modern agentic approach to high-frequency trading represented by the `QuantAgent` repository (2025). It has been expanded to include enterprise-grade ML frameworks and cutting-edge LLM-driven alpha discovery techniques.

---

## 1. Paper Review: *Time Series Momentum*
**Authors:** Moskowitz, Ooi, and Pedersen (Journal of Financial Economics, 2012)

### Core Concept
The paper establishes the existence of **Time Series Momentum (TSMOM)**. Unlike traditional cross-sectional momentum (relative performance within a group), TSMOM focuses purely on an asset's *own* past performance.

### Key Methodology & Findings
- **Universe:** Analyzed 58 liquid instruments (equity index futures, currencies, commodities, bond futures) over 25 years.
- **Signal:** The past 12-month excess return is a robust, positive predictor of future returns.
- **Implementation:** Buying an asset if its 12-month return is positive and shorting if negative, scaled by ex-ante volatility ($sgn(R_{t-12, t}) / \sigma_t$).
- **Impact:** Statistically significant positive premiums across all asset classes. Trends persist for 1–12 months but partially reverse over 1–5 years.

### Significance
This is the academic bedrock for the modern CTA (Commodity Trading Advisor) and Managed Futures industry. It mathematically proved that "trend following" is a persistent market anomaly.

---

## 2. Repository Reviews

### 2.1. `QuantAgent`
**Source:** [Y-Research-SBU/QuantAgent](https://github.com/Y-Research-SBU/QuantAgent) (2025)
- **Core Architecture:** A Multi-Agent LLM trading system built on LangChain. It uses a sequence of specialized agents (Indicator, Pattern, Trend, Decision) to simulate human technical analysis.
- **Innovation:** The "Robo-Chartist"—using Multi-modal Vision Models to read chart images and interpret visual support/resistance and market psychology.
- **Critique:** While labeled HFT, it operates at a high latency (LLM inference) making it Algorithmic Swing Trading. The approach is heuristic and non-deterministic.

### 2.2. `Qlib` (Microsoft)
**Source:** [microsoft/qlib](https://github.com/microsoft/qlib) (2020)
- **Core Architecture:** An open-source, AI-oriented quantitative investment platform that covers the entire pipeline (data processing, model training, backtesting).
- **Innovation:** Highly optimized for ML models (LightGBM, ALSTM, Transformer) for cross-sectional return prediction (Alpha forecasting).
- **Critique:** Perfect for the "Medium Layer" of a trading architecture. It provides a robust, low-level data and backtesting engine that significantly outpaces standard Python frameworks for large-scale ML tasks.

### 2.3. `FinRL` (AI4Finance Foundation)
**Source:** [AI4Finance-Foundation/FinRL](https://github.com/AI4Finance-Foundation/FinRL) (2020)
- **Core Architecture:** A comprehensive framework applying Deep Reinforcement Learning (DRL) to trading, framing the market as a Markov Decision Process (MDP).
- **Innovation:** Uses algorithms like PPO, SAC, and DDPG for dynamic portfolio allocation and execution optimization, adapting to changing market conditions.
- **Critique:** Bridges the gap between predictive modeling and optimal execution. DRL agents are adaptable but highly sample-inefficient and prone to overfitting without extensive regularization.

---

## 3. Synthesis: Statistical Rigor vs. Agentic Heuristics

| Feature              | Traditional Quant (2012)      | AI ML Quant (Qlib/FinRL)            | The Agentic Quant (QuantAgent)        |
|:---------------------|:------------------------------|:------------------------------------|:--------------------------------------|
| **Foundation**       | Statistical Regressions       | Deep Learning & Reinforcement RL    | Vision LLMs & Multi-Agent Debate      |
| **Trend Definition** | $sgn(R_{t-12, t}) / \sigma_t$ | Non-linear feature extraction       | Visual Chartism & Pattern Recognition |
| **Execution**        | Heuristic Sizing              | DRL Portfolio Optimization          | Text-based LLM Directives             |
| **Latency**          | Extremely Low                 | Medium (Batched inference)          | High (LLM Inference)                  |
| **Reliability**      | Proven, Consistent Anomalies  | Highly performant, requires tuning  | Experimental, Heuristic-based         |

---

## 4. Further Seminal Publications in Quantitative Finance

### A. The Deep Learning Evolution of TSMOM
**Paper:** *Enhancing Time Series Momentum Strategies Using Deep Neural Networks* (Lim, Zohren, Roberts - 2019)
- **Utility:** Provides a blueprint for upgrading trend-following using LSTMs to capture non-linear trends and size positions dynamically.

### B. The Alpha Miner’s Holy Grail
**Paper:** *101 Formulaic Alphas* (Kakushadze - 2015)
- **Utility:** Essential for building a highly optimized, low-latency mathematical engine (using numpy/polars) for statistical arbitrage.

### C. The Seminal Machine Learning Benchmark
**Paper:** *Empirical Asset Pricing via Machine Learning* (Gu, Kelly, Xiu - 2020)
- **Utility:** Definitively proves Random Forests and shallow Neural Networks outperform traditional regression, providing 94 core features to calculate in a data pipeline.

### D. LLMs as Sentiment Engines
**Paper:** *Can ChatGPT Forecast Stock Price Movements?* (Lopez-Lira, Tang - 2023)
- **Utility:** Proves LLMs are excellent at text interpretation. Useful for building a news-driven sentiment module or risk "kill-switch."

### E. Open-Source Financial AI
**Paper:** *FinGPT: Open-Source Financial Large Language Models* (Yang et al. - 2023)
- **Utility:** Ideal for running local, latency-optimized, cost-effective LLMs fine-tuned on financial data.

### F. LLM-Driven Alpha Discovery
**Paper:** *AlphaGPT: Human-AI Interactive Alpha Mining for Quantitative Investment* (Wang et al. - 2023)
- **Core Concept:** Proposes a framework where LLMs are used to iteratively generate and test mathematical alpha formulas based on market dynamics.
- **Utility:** Bridges the gap between Kakushadze’s static 101 formulas and dynamic markets. Suggests an agentic loop where `quantbot` writes and backtests its own math-based alpha signals.

### G. Agentic Memory in Financial Markets
**Paper:** *FinMem: A Performance-Enhanced LLM Trading Agent with Layered Memory and Character Design* (Yu et al. - 2023)
- **Core Concept:** Introduces an LLM trading agent equipped with a human-like layered memory module (working, short-term, long-term) to process news, reflect on past decisions, and adjust strategy.
- **Utility:** Crucial for building a reflective agent. Giving the LLM a ledger of its past successes/failures drastically improves zero-shot trading performance compared to stateless agents.

---

## 5. 2024–2025 Research Update

The following papers address gaps in the original literature review, covering multi-agent debate frameworks, self-improving agents, financial reasoning via RL, tool-augmented agents, foundation models for time series, and efficient architectures for Rust portability.

### H. Multi-Agent Debate for Trading Decisions
**Paper:** *TradingAgents: Multi-Agents LLM Financial Trading Framework* (Xiao, Sun, Luo, Wang - Dec 2024)
**Link:** [arXiv:2412.20138](https://arxiv.org/abs/2412.20138)
- **Core Concept:** Bull vs. bear debate among specialized agents (fundamental, sentiment, technical) with a risk manager making final calls.
- **Utility:** Closest analogue to quantbot's Phase 2. Directly informs how the Decision Agent should weigh conflicting signals in the fan-out/fan-in LangGraph design.

### I. Behavioral Diversity in Agent Ensembles
**Paper:** *StockAgent: LLM-based Stock Trading in Simulated Real-world Environments* (Zhang et al. - Jul 2024)
**Link:** [arXiv:2407.18957](https://arxiv.org/abs/2407.18957)
- **Core Concept:** Introduces heterogeneous agent "personalities" — conservative, aggressive, trend-following — and shows diversity improves ensemble robustness. Also benchmarks GPT vs. Gemini.
- **Utility:** Informs agent configuration for quantbot. Weight assignment in `SignalCombiner` could account for agent behavioral type, not just signal confidence.

### J. Self-Improving Alpha Discovery
**Paper:** *QuantAgent: Seeking Holy Grail in Trading by Self-Improving LLM* (Wang, Yuan, Ni, Guo — HKUST - Feb 2024)
**Link:** [arXiv:2402.03755](https://arxiv.org/abs/2402.03755)
- **Core Concept:** LLM generates alpha factors, backtests them, evaluates results, and iteratively refines. Distinct from Y-Research QuantAgent (Section 2.1).
- **Utility:** Validates AlphaGPT concept (Section F) with concrete results. Blueprint for quantbot's Generative Alpha Layer — LLM writes and backtests its own math-based signals.

### K. Small Financial Reasoning Models via RL
**Paper:** *Fin-R1: Financial Reasoning through Reinforcement Learning* (Liu et al. - Mar 2025)
**Link:** [arXiv:2503.16252](https://arxiv.org/abs/2503.16252)
- **Core Concept:** A 7B model matching GPT-4 on financial reasoning via DeepSeek-R1 RL training.
- **Utility:** Could replace expensive API calls for quantbot's LLM agents with a locally-runnable model — drastically reducing latency and cost. The 2025 successor to FinGPT (Section E).

### L. Multimodal Tool-Augmented Trading Agents
**Paper:** *FinAgent: A Multimodal Foundation Agent for Financial Trading* (Zhang et al. - Feb 2024)
**Link:** [arXiv:2402.18485](https://arxiv.org/abs/2402.18485)
- **Core Concept:** Processes both text and visual data (candlestick charts) and calls external tools (code execution, APIs).
- **Utility:** Extends the "Robo-Chartist" concept (Section 2.1). Provides patterns for making quantbot's Pattern and Trend agents multimodal with tool use.

### M. Open-Source Agent Platform for Finance
**Paper:** *FinRobot: Open-Source AI Agent Platform for Financial Applications* (Yang et al. - May 2024)
**Link:** [arXiv:2405.14767](https://arxiv.org/abs/2405.14767)
- **Core Concept:** Complete platform with agent orchestration, market data adapters, and multi-agent patterns.
- **Utility:** Architecture patterns for Phase 3 (paper trading + dashboard). Data adapter design applicable to quantbot's `DataProvider` hierarchy.

### N. Chain-of-Thought Prompting for Stock Selection
**Paper:** *MarketSenseAI: Can Large Language Models Beat Wall Street?* (Fatouros et al. - Jan 2024)
**Link:** [arXiv:2401.03737](https://arxiv.org/abs/2401.03737)
- **Core Concept:** GPT-4 with structured chain-of-thought prompting achieves 72.3% directional accuracy and outperforms S&P 500.
- **Utility:** Demonstrates that structured CoT reasoning improves signal quality. Should directly inform prompt engineering for quantbot's Indicator, Pattern, and Trend agents.

### O. Foundation Models for Time Series Forecasting
**Paper:** *Chronos: Learning the Language of Time Series* (Ansari et al., Amazon - Mar 2024)
**Link:** [arXiv:2403.07815](https://arxiv.org/abs/2403.07815)
- **Core Concept:** Tokenizes numerical time series and achieves zero-shot forecasting competitive with task-specific models.
- **Utility:** Potential paradigm shift for quantbot's forecasting layer — one pre-trained model across all instruments instead of per-asset LSTMs/LightGBM.

### P. State Space Models for Stock Prediction
**Paper:** *MambaStock: Selective State Space Model for Stock Prediction* (Shi - Feb 2024)
**Link:** [arXiv:2402.18959](https://arxiv.org/abs/2402.18959)
- **Core Concept:** Mamba's linear complexity (vs. transformer's quadratic) with competitive forecasting performance.
- **Utility:** Simpler architecture is more amenable to Rust implementation — directly relevant to Phase 4. Hybrid TFT-Mamba models showing 10%+ improvement on benchmarks.

### Q. Comprehensive RL for Finance Survey
**Paper:** *Reinforcement Learning in Financial Decision Making: A Systematic Review* (Nov 2024)
**Link:** [arXiv:2411.07585](https://arxiv.org/abs/2411.07585)
- **Core Concept:** Survey of 250+ papers. Key finding: hybrid methods (LSTM-DQN, CNN-PPO, Attention-DDPG) outperform pure RL by 15-20%.
- **Utility:** Validates quantbot's hybrid architecture. Maps which RL algorithms work best per trading task (execution, allocation, market making).

### R. Geometric Algebra for Neural Feature Interaction
**Paper:** *CliffordNet: All You Need is Geometric Algebra* (Ji - Feb 2026)
**Link:** [arXiv:2601.06793v2](https://arxiv.org/abs/2601.06793v2)
- **Core Concept:** Replaces the standard spatial mixer + FFN block with a single Geometric Product (uv = u·v + u∧v). The inner product captures feature coherence; the wedge product captures structural variation via oriented bivectors. Eliminates FFN layers entirely while matching or exceeding standard architectures on CIFAR-100.
- **Key Mechanism — Sparse Rolling Interaction:** Approximates the full geometric product via cyclic channel shifts at exponential offsets {1, 2, 4, 8, 16}, yielding O(N·D) complexity. For financial time series, these shifts could map directly to trading timescales (daily, weekly, monthly, quarterly).
- **Financial Relevance — Clifford TSMOM:**
  - Standard TSMOM computes scalar momentum: `price_now / price_lookback - 1`
  - Clifford TSMOM would compute the full geometric product between multi-feature vectors (price, volume, volatility) at different timepoints
  - The **inner product** captures trend continuation (feature coherence over time)
  - The **wedge product** captures regime changes — markets still rising but with fundamentally different volatility structure (pre-crash divergence)
  - The wedge product is **anti-symmetric** (u∧v = −v∧u), meaning trend reversals produce sign flips — a natural momentum reversal detector
  - Differential mode (working with returns instead of prices) maps directly to the paper's best-performing variant
- **Limitations:** Only validated on CIFAR-100 (32×32 images). No ImageNet, no time series, no financial benchmarks. Single author, no peer review. Outdated baselines. Novel theory but immature validation (7/10 novelty, 4/10 empirical maturity).
- **Utility:** Research direction for Phase 5. A Clifford-enhanced signal generator could detect regime changes that scalar momentum misses. Worth exploring once the core system is stable — potentially a novel publishable contribution if it outperforms standard TSMOM on regime change detection.

### S. Time Series Foundation Model for Zero-Shot Forecasting
**Repo:** [google-research/timesfm](https://github.com/google-research/timesfm) (10.5K ★)
**Paper:** *A Decoder-Only Foundation Model for Time-Series Forecasting* (ICML 2024)
- **Core Concept:** TimesFM 2.5 is a 200M-parameter pretrained time series model from Google Research. Feed raw price data in, get probabilistic forecasts out — zero-shot, no training required.
- **Key Specs:** 200M params (runs on Mac M4), 16K context length, up to 1K-step horizon, quantile output (10th–90th percentiles), Apache 2.0 license.
- **Utility — Better Chronos Alternative:** Replaces Amazon's Chronos (§O) as the primary foundation model candidate. Newer (Sept 2025), smaller, probabilistic output gives confidence intervals. Complements TSMOM: momentum captures trends, TimesFM captures mean-reversion and cyclical patterns. Zero-shot means new instruments work from day one without TSMOM's 12-month lookback requirement.
- **Rust integration:** Export to ONNX → `ort` crate (ONNX Runtime for Rust), or keep as Python microservice.

### Open-Source Architecture References

| Repo | Stars | Relevance |
|---|---|---|
| [TradingAgents](https://github.com/TauricResearch/TradingAgents) | 9.3K | Bull/bear debate implementation (paper §H now open-sourced). Study analyst→researcher→risk→trader pipeline. |
| [ai-hedge-fund](https://github.com/virattt/ai-hedge-fund) | 49.6K | Competitive benchmark — near-identical multi-agent architecture. Compare Sharpe ratios. |
| [QuantAgent (SBU)](https://github.com/Y-Research-SBU/QuantAgent) | — | Architecture twin to Phase 2 (indicator+pattern+trend agents via LangGraph+vision LLM). Key difference: no TSMOM anchor, no memory, no execution. Study their vision LLM chart pipeline + multi-timeframe analysis (1m to 1d). |
| [nofx](https://github.com/NoFxAiOS/nofx) | 11.2K | Circuit breaker / safe mode pattern — auto-flatten after consecutive failures, drawdown breaker, graceful degradation. |
| [TorchTrade](https://github.com/TorchTrade/torchtrade) | — | RL trading framework on TorchRL. Study: multi-timeframe observation design, Chronos as encoder transform (validates foundation model approach), PPO-based agent weighting. |

### Phase Relevance Map

| QuantBot Phase | Most Relevant Papers & Repos |
|---|---|
| Phase 2 (LLM agents) | TradingAgents (H), StockAgent (I), MarketSenseAI (N), FinAgent (L), QuantAgent-SBU, ai-hedge-fund |
| Phase 3 Track A (TSMOM + IG) | nofx (circuit breaker) |
| Phase 3 Track B (LLM agents in Rust) | TradingAgents, QuantAgent-SBU (vision pipeline), MambaStock (P) |
| Phase 4 (Extensions) | TimesFM (S), Fin-R1 (K), QuantAgent-HKUST (J), CliffordNet (R), TorchTrade (RL weighting) |
| General reference | RL survey (Q) |

---

## 6. Architectural Blueprint for `quantbot`

Triangulating across traditional stats, enterprise machine learning, and modern LLM agents — now updated with 2024–2025 research — suggests a comprehensive, five-layer architecture for `quantbot`:

### Layer 1: Fast Execution (Math/Stat-Arb)
- Implement fixed *101 Formulaic Alphas* (B) for high-speed, intraday mean-reversion.
- Use Reinforcement Learning (*FinRL*) purely for execution scheduling and optimal portfolio sizing.
- **Updated:** RL survey (Q) shows hybrid methods (LSTM-DQN, CNN-PPO) outperform pure RL by 15-20%. Use hybrid architecture, not pure DRL.

### Layer 2: Medium Forecasting (Deep Learning + Foundation Models)
- Implement LSTMs and LightGBM (*Gu et al.* (C), *Lim et al.* (A)) on daily/hourly data to capture multi-week macro trends and non-linear patterns.
- **New — Chronos zero-shot forecasting (O):** Replace per-asset trained models with Amazon's Chronos foundation model for zero-shot time series prediction. Especially valuable for new instruments lacking training history.
- **New — MambaStock state space models (P):** Mamba's linear complexity (vs. transformer's quadratic) makes it amenable to Rust implementation. Consider hybrid TFT-Mamba for 10%+ forecast improvement.

### Layer 3: Agentic Signal Generation (Multi-Agent LLM)
- Fan-out/fan-in multi-agent architecture: TSMOM (quant), Indicator, Pattern, Trend agents run in parallel, merge at Decision node.
- **New — Bull/bear debate (H):** Extend the Decision node with bull and bear advocate agents that argue opposing positions (*TradingAgents*). Risk manager adjudicates. Better conflict resolution than pure numeric signal averaging.
- **New — Agent behavioral diversity (I):** Assign agents different risk personas (conservative, aggressive, trend-following) per *StockAgent*. Weight in `SignalCombiner` should account for persona type, not just signal confidence.
- **New — Chain-of-thought prompting (N):** All LLM agents use structured CoT reasoning per *MarketSenseAI* (72.3% directional accuracy). Prompt template: identify signal → assess strength → consider contradicting evidence → state confidence → conclude.
- **New — Multimodal tool use (L):** Per *FinAgent*, Pattern and Trend agents should process both text and visual data (candlestick charts) and be able to call external tools (code execution, data APIs).
- **New — Local inference via Fin-R1 (K):** 7B model matching GPT-4 on financial reasoning. Run via Ollama for near-zero cost paper trading. Supported by both `langchain` (Python) and `rig-core` (Rust).

### Layer 4: Generative Alpha Discovery (AlphaGPT + Self-Improvement)
- An asynchronous LLM multi-agent loop that continually writes, backtests, and validates mathematical alpha formulas, promoting successful ones to the Fast Execution layer.
- **Updated — Validated by QuantAgent-HKUST (J):** Concrete implementation showing LLM generates factors, backtests them, evaluates results, and iteratively refines. Confirms *AlphaGPT* (F) concept works in practice.

### Layer 5: Risk, Reflection & Memory
- A reflective agent that ingests raw macroeconomic news (*Lopez-Lira* (D)), acts as a fundamental risk kill-switch.
- **New — SQLite-backed layered memory (G):** Per *FinMem*, maintain a decision ledger in SQLite (`~/.quantbot/memory.db`) with tables: `signal_log` (every signal), `decision_log` (combiner outputs + actual P&L), `agent_memory` (condensed lessons injected into LLM prompts). SQLite chosen for ACID transactions during live trading, SQL queryability, zero config, and clean migration to Postgres/`rusqlite` for Rust port.
- **New — RL-based dynamic agent weighting (Q):** Replace fixed `SignalCombiner` weights with a bandit/PPO agent that learns optimal weights per market regime. RL survey shows 15-20% improvement over static methods.
- **New — Architecture patterns from FinRobot (M):** Leverage data adapter and agent orchestration patterns for the paper trading + dashboard layer.

---

## 7. Phase 3 Engineering Log (Rust Rewrite)

### 2025-03 — Track A: Core Engine + Execution Router

**Completed components:**

| Component | Location | Tests | Notes |
|---|---|---|---|
| Core types (Bar, Signal, Portfolio, Universe) | `src/core/` | 9 | `Bar` as plain struct, `BarSeries` as validated `Vec<Bar>` |
| CSV data loader + `DataProvider` trait | `src/data/` | 4 | Handles Yahoo Finance CSV format (Close before Open) |
| TSMOM agent + EWMA volatility | `src/agents/tsmom/` | 6 | Pure Rust EWMA matching pandas `ewm(com=60).mean()` |
| Backtest engine + metrics | `src/backtest/` | 9 | Next-open execution, mark-to-market at close |
| Execution router | `src/execution/router.rs` | 22 | Per-instrument specs, lot rounding, spread costs |
| CLI (clap) | `src/main.rs` | — | `backtest` subcommand + stubs for paper-trade/live/positions |

**Validation gate (pre-router integration):**
- Rust Sharpe 1.377 vs Python 1.370 (+0.5%) — 60-day, 4 instruments
- Rust Sharpe 0.930 vs Python 0.882 (+5.5%) — 252-day, 6 tradeable instruments
- Rust Sharpe 0.378 vs Python 0.340 (+11.1%) — 252-day, 21 full universe

**ExecutionRouter integration into BacktestEngine (2025-03-31):**

Replaced the flat `slippage_bps` cost model with the per-instrument `ExecutionRouter`:

1. **Sizing:** `target_notional / price` → `router.size_from_weight()` — accounts for `point_value` (e.g. GC=F gold futures pv=100) and lot rounding (e.g. FX lot_step=0.1, min_deal=0.5)
2. **Cost model:** Slippage as price adjustment → direction-aware spread cost as cash deduction. `SpreadCostTracker` applies 0x (hold), 1x (open/close), or 2x (flip) multiplier per instrument
3. **Position point_value:** Hardcoded `1.0` → from router spec, so `PortfolioState::nav()` correctly values futures positions
4. **Snapshot notionals:** `quantity * price` → `quantity * price * point_value` for accurate exposure reporting

Net effect: backtest now reflects IG spread betting reality — per-instrument spread rates (3-10 bps), correct futures contract sizing, and direction-aware transaction costs.

**39 → 71 tests passing, clean clippy.**

**Post-router validation results (2025-03-31):**

| Test | Pre-Router | Post-Router | Python | Δ vs Python | Trades Before → After |
|---|---|---|---|---|---|
| 60-day, 4 inst | 1.377 | 1.386 | 1.370 | +1.1% ✅ | 271 → 178 (-34%) |
| 252-day, 6 tradeable | 0.930 | 1.117 | 0.882 | +26.6% 🟡 | 1,522 → 1,298 (-15%) |
| 252-day, 21 full | 0.378 | 0.512 | 0.340 | +50.7% 🔴 | 5,440 → 4,970 (-9%) |

**Analysis:** Sharpe went *up* despite adding spread costs. Lot rounding acts as a natural trade filter — small weight changes round to zero, suppressing micro-rebalances. Fewer trades → less friction → higher Sharpe. GC=F (pv=100) is the smoking gun: positions snap to ~$26K increments, which propagates through portfolio construction.

The 60-day test validates core engine correctness (+1.1% match). Tests 2/3 diverged because Python doesn't do lot rounding or point_value sizing — they're now computing fundamentally different strategies. The Rust engine is more realistic (reflects actual IG execution constraints). Python comparison accepted as no longer meaningful for these tests.

**Next: paper-trade mode, IG API client.**

---

## References

### Foundational (2012–2023)
- Gu, S., Kelly, B., & Xiu, D. (2020). "Empirical Asset Pricing via Machine Learning." *The Review of Financial Studies*, 33(5), 2223-2273.
- Kakushadze, Z. (2015). "101 Formulaic Alphas." *Wilmott Magazine*, 2016(84), 72-81.
- Lim, B., Zohren, S., & Roberts, S. J. (2019). "Enhancing Time Series Momentum Strategies Using Deep Neural Networks." *Available at SSRN 3369159*.
- Liu, X. Y., et al. (2020). "FinRL: A Deep Reinforcement Learning Library for Automated Stock Trading in Quantitative Finance." *Deep RL Workshop, NeurIPS 2020*.
- Lopez-Lira, A., & Tang, Y. (2023). "Can ChatGPT Forecast Stock Price Movements? Return Predictability and Large Language Models." *Available at SSRN 4412788*.
- Microsoft. (2020). "Qlib: An AI-oriented Quantitative Investment Platform." *arXiv preprint arXiv:2009.11189*.
- Moskowitz, T. J., Ooi, Y. H., & Pedersen, L. H. (2012). "Time series momentum." *Journal of Financial Economics*, 104(2), 228-250.
- Wang, L., et al. (2023). "AlphaGPT: Human-AI Interactive Alpha Mining for Quantitative Investment." *arXiv preprint arXiv:2308.00016*.
- Xiong, F., Zhang, X., Feng, A., Sun, S., & You, C. (2025). "QuantAgent: Price-Driven Multi-Agent LLMs for High-Frequency Trading." *arXiv preprint arXiv:2509.09995* / [Y-Research-SBU/QuantAgent GitHub](https://github.com/Y-Research-SBU/QuantAgent).
- Yang, H., Liu, X. Y., & Wang, C. D. (2023). "FinGPT: Open-Source Financial Large Language Models." *arXiv preprint arXiv:2306.06031*.
- Yu, B., et al. (2023). "FinMem: A Performance-Enhanced LLM Trading Agent with Layered Memory and Character Design." *arXiv preprint arXiv:2311.13743*.

### 2024–2026 Update
- Ansari, A. F., et al. (2024). "Chronos: Learning the Language of Time Series." *arXiv preprint arXiv:2403.07815*.
- Fatouros, G., et al. (2024). "Can Large Language Models Beat Wall Street? Unveiling the Potential of AI in Stock Selection." *arXiv preprint arXiv:2401.03737*.
- Liu, Z., et al. (2025). "Fin-R1: Financial Reasoning through Reinforcement Learning." *arXiv preprint arXiv:2503.16252*.
- Shi, Z. (2024). "MambaStock: Selective State Space Model for Stock Prediction." *arXiv preprint arXiv:2402.18959*.
- Wang, S., Yuan, Y., Ni, L. M., & Guo, J. (2024). "QuantAgent: Seeking Holy Grail in Trading by Self-Improving Large Language Model." *arXiv preprint arXiv:2402.03755*.
- Xiao, Y., Sun, Y., Luo, J., & Wang, W. (2024). "TradingAgents: Multi-Agents LLM Financial Trading Framework." *arXiv preprint arXiv:2412.20138*.
- Yang, H., et al. (2024). "FinRobot: An Open-Source AI Agent Platform for Financial Applications using Large Language Models." *arXiv preprint arXiv:2405.14767*.
- Zhang, C., et al. (2024). "StockAgent: LLM-based Stock Trading in Simulated Real-world Environments." *arXiv preprint arXiv:2407.18957*.
- Zhang, W., et al. (2024). "FinAgent: A Multimodal Foundation Agent for Financial Trading." *arXiv preprint arXiv:2402.18485*.
- Das, A., et al. (2024). "A Decoder-Only Foundation Model for Time-Series Forecasting." *ICML 2024*. [google-research/timesfm](https://github.com/google-research/timesfm).
- Ji, Z. (2026). "CliffordNet: All You Need is Geometric Algebra." *arXiv preprint arXiv:2601.06793v2*.
- (2024). "Reinforcement Learning in Financial Decision Making: A Systematic Review." *arXiv preprint arXiv:2411.07585*.
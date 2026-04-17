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

### T. Sparse MoE for General Reasoning
**Model:** [nvidia/Nemotron-Cascade-2-30B-A3B](https://huggingface.co/nvidia/Nemotron-Cascade-2-30B-A3B) (NVIDIA - 2025)
- **Core Concept:** 30B total parameters, only 3B active via Mixture-of-Experts (MoE). Achieves reasoning performance competitive with much larger dense models at a fraction of the compute.
- **Evaluation for QuantBot:** Impressive general reasoning but not the right fit for the indicator agent. Fin-R1 7B (§K) already proved that domain specialization (financial reasoning via RL) beats general reasoning ability for financial signals. Nemotron also needs ~60GB VRAM despite MoE efficiency — overweight for signal generation.
- **Utility:** Bookmark for a hypothetical "AI portfolio manager" layer above the signal generators — a meta-reasoning agent that interprets cross-strategy performance, regime context, and allocation decisions. That's a Phase 5+ problem. Stick with Fin-R1 for the indicator agent.

### U. AI-Oriented Quantitative Investment Platform
**Repo:** [microsoft/qlib](https://github.com/microsoft/qlib) (Microsoft - 2020, actively maintained)
- **Core Concept:** Open-source ML platform covering the full quant pipeline: data processing, model training (LightGBM, LSTM, Transformer, GNN, RL), backtesting, and portfolio optimization. Primarily targets Chinese A-share markets (CSI300/CSI500) and US equities.
- **Evaluation for QuantBot:** Wrong tool for the job. Python-only (QuantBot has a validated Rust engine). Equity/stock-picking focused (QuantBot is multi-asset TSMOM). Overkill architecture with its own data layer, workflow engine, and nested decision framework — would fight the framework to do simple trend following. No spread betting / CFD support.
- **What to steal:** (1) RD-Agent concept — LLM-driven factor mining loop maps directly to the planned indicator agent's automated alpha discovery. (2) Online model rolling — automatic model retraining pipeline for production drift management, relevant when QuantBot reaches live deployment.
- **Verdict:** Research reference, not a dependency. QuantBot's strength is simplicity — 6 instruments, TSMOM, Rust, IG execution.

### Open-Source Architecture References

| Repo | Stars | Relevance |
|---|---|---|
| [ATLAS](https://github.com/chrisworsey55/atlas-gic) | — | **Deep-dived (§5.1).** 25-agent Darwinian selection system with meta-weighting, autoresearch, reflexivity simulation. Closest architectural analog to quantbot's multi-agent blend. |
| [TradingAgents](https://github.com/TauricResearch/TradingAgents) | 9.3K | **Deep-dived (§5.2).** Bull/bear debate implementation (paper §H now open-sourced). Hierarchical analyst→researcher→risk→trader pipeline with BM25 memory. |
| [ai-hedge-fund](https://github.com/virattt/ai-hedge-fund) | 49.6K | Competitive benchmark — near-identical multi-agent architecture. Compare Sharpe ratios. |
| [QuantAgent (SBU)](https://github.com/Y-Research-SBU/QuantAgent) | — | Architecture twin to Phase 2 (indicator+pattern+trend agents via LangGraph+vision LLM). Key difference: no TSMOM anchor, no memory, no execution. Study their vision LLM chart pipeline + multi-timeframe analysis (1m to 1d). |
| [nofx](https://github.com/NoFxAiOS/nofx) | 11.2K | Circuit breaker / safe mode pattern — auto-flatten after consecutive failures, drawdown breaker, graceful degradation. |
| [TorchTrade](https://github.com/TorchTrade/torchtrade) | — | RL trading framework on TorchRL. Study: multi-timeframe observation design, Chronos as encoder transform (validates foundation model approach), PPO-based agent weighting. |
| [FinceptTerminal](https://github.com/Fincept-Corporation/FinceptTerminal) | — | **Reviewed (§5.3).** C++/Qt6 Bloomberg-class terminal with 30+ LLM agent personas, multi-engine backtesting, IG broker adapter. SuperAgent router uses LLM-based intent classification. |
| [OpenStock](https://github.com/Open-Dev-Society/OpenStock) | — | **Reviewed (§5.3).** Next.js stock tracker. One borrowable pattern: multi-provider LLM fallback (Gemini → Siray.ai). |

### 5.1 ATLAS Deep-Dive — Darwinian Selection + Meta-Weighting

**Source:** [chrisworsey55/atlas-gic](https://github.com/chrisworsey55/atlas-gic) (2025)

ATLAS is a multi-agent LLM trading system with 25 agents organized in 4 layers, Darwinian selection across 5 market-regime cohorts, and a reflexivity simulation engine. It is the closest open-source architectural analog to quantbot's multi-agent blend — but oriented toward equity stock-picking rather than multi-asset TSMOM.

#### Architecture: 4-Layer Agent Hierarchy

- **Layer 1 (Macro, 10 agents):** Central bank, geopolitical, China, dollar, yield curve, commodities, volatility, emerging markets, news sentiment, institutional flow
- **Layer 2 (Sector, 7 agents):** Semiconductor, energy, biotech, consumer, industrials, financials + relationship mapper
- **Layer 3 (Superinvestors, 4 agents):** Druckenmiller (macro/momentum), Aschenbrenner (AI/compute), Baker (deep tech), Ackman (quality)
- **Layer 4 (Decision, 4 agents):** CRO (risk adversary), Alpha Discovery, Execution, CIO (final synthesis)

Each agent receives macro regime as context before making picks. Layers feed forward: macro → sector → philosophy → decision.

#### Borrowable Pattern 1: Constrained Softmax Meta-Weighting

ATLAS scores cohorts on 50% hit rate + 50% normalized Sharpe, then applies softmax with floor/ceiling constraints (MIN=0.2, MAX=0.8) to prevent concentration. This is more sophisticated than quantbot's fixed per-asset-class blend weights.

```python
# atlas-gic/janus.py:202-239
def _softmax_with_constraints(self, scores: Dict[str, float]) -> Dict[str, float]:
    max_score = max(scores.values())
    exp_scores = {k: math.exp(v - max_score) for k, v in scores.items()}
    total = sum(exp_scores.values())
    weights = {k: v / total for k, v in exp_scores.items()}
    # Apply floor/ceiling constraints
    for cohort in weights:
        if weights[cohort] < self.MIN_WEIGHT:
            weights[cohort] = self.MIN_WEIGHT
```

**Quantbot application:** Replace static blend weights with adaptive softmax over rolling Sharpe. Floor prevents any strategy from being zeroed out entirely. Ceiling prevents over-concentration during lucky streaks. For quantbot: `MIN_WEIGHT=0.1, MAX_WEIGHT=0.9` on TSMOM vs indicator blend ratios per asset class, recalculated weekly from a 60-day rolling window.

#### Borrowable Pattern 2: Conviction-Weighted Disagreement Penalty

When agents disagree on direction, ATLAS penalizes conviction proportionally to opposing weight — preventing false consensus and surfacing genuine conflicts.

```python
# atlas-gic/janus.py:374-396
long_weighted = sum(e["conviction"] * e["weight"] for e in longs)
short_weighted = sum(e["conviction"] * e["weight"] for e in shorts)
if contested:
    disagreement_penalty = opposing_conviction * 0.5
    final_conviction = max(0, base_conviction - disagreement_penalty)
```

**Quantbot application:** When TSMOM says LONG and indicator says SHORT, current combiner uses weighted average which produces a weak LONG. ATLAS's approach would instead reduce conviction to near-zero (via penalty), producing a FLAT or very small position. This aligns with the ablation finding that FLAT selectivity *is* the risk management — disagreement should shrink positions, not blend them.

#### Borrowable Pattern 3: Autoresearch / Prompt Evolution

The autoresearch loop identifies the worst-performing agent by rolling 60-day Sharpe, generates a single targeted prompt modification via Claude, runs it for 5 trading days, then commits or reverts based on Sharpe improvement. 54 modifications attempted, 16 kept (30% success rate).

**Quantbot application:** Directly maps to the planned prompt A/B testing pipeline. Current approach requires manual prompt editing + cache fill + replay. ATLAS's automated version: (1) identify worst-performing instrument by rolling Sharpe, (2) generate prompt modification with Claude reviewing recent wrong calls, (3) cache fill on 5-day window, (4) replay comparison, (5) commit or revert `prompts/indicator_system.md`. Could be a `quantbot eval autoresearch` subcommand.

#### Borrowable Pattern 4: Forward Training on Simulated Scenarios

MiroFish generates 5 future scenarios (base, bull, bear, tail_up, tail_down) with 30-day correlated price paths. Agents make recommendations blind, then are scored against outcomes. Performance in simulations adjusts agent weights independently from live P&L.

```python
# atlas-gic/mirofish_trainer.py:223
score = 0.5 + (avg_return / 0.40)  # +20% return = 1.0, -20% = 0.0
if avg_return < 0:
    score *= (1 - conviction * 0.3)  # Penalize wrong high-conviction
```

**Quantbot application:** Phase 4+. Generate synthetic OHLCV scenarios (using TimesFM §S or Monte Carlo with historical vol), run indicator agent, score against synthetic outcomes. Enables weight calibration without waiting for live market data.

#### Borrowable Pattern 5: Reflexivity Engine (Soros-inspired)

Models 5 feedback loops: price→fundamentals (15%+ moves trigger credit events), P&L→behavior (10%+ drawdown triggers forced selling), narrative→flows (3+ analyst consensus drives retail following), market→policy (15%+ drawdown triggers CB easing), reflexive reversal (5+ rounds same direction = extremes). Generates branching scenarios scored against real 5-day outcomes.

**Quantbot application:** Research direction for news overlay (Track D3). Instead of simple sentiment classification, model how news creates cascading effects: tariff announcement → dollar strength → FX position risk → force deleveraging. The reflexivity lens explains *why* news matters for trading, not just *what* the sentiment is.

#### What to Ignore

- **Equity stock-picking orientation** — ATLAS is designed for 150+ equities, not 6-instrument multi-asset TSMOM
- **Fully autonomous prompt evolution** — autoresearch is interesting but live prompt mutation is risky for production. Use offline replay first
- **CIO layer complexity** — backtest showed portfolio returns bottlenecked on orchestration, not signal quality. CIO was independently downweighted to 0.3 (minimum) by the Darwinian system. Lesson: keep the decision layer simple

### 5.2 TradingAgents Deep-Dive — Hierarchical Debate + BM25 Memory

**Source:** [TauricResearch/TradingAgents](https://github.com/TauricResearch/TradingAgents) (2024)
**Paper:** arXiv:2412.20138 (§H above)

TradingAgents is the open-source implementation of the Multi-Agents LLM Financial Trading Framework paper. It orchestrates specialized LLM agents through a hierarchical pipeline: analysts → researcher debate → trader → risk debate → portfolio manager. Built on LangGraph with modular analyst plugins and BM25-based experience memory.

#### Architecture: Hierarchical Pipeline

1. **Analysis phase** — 4 analyst types (market, fundamentals, news, social) generate reports using specialized tools
2. **Investment debate** — Bull and Bear researchers argue for max `N` rounds, Research Manager judges
3. **Trade decision** — Trader synthesizes research into a conviction-rated proposal
4. **Risk debate** — Aggressive, Neutral, and Conservative risk analysts argue perspectives
5. **Final decision** — Portfolio Manager synthesizes all inputs into BUY/OVERWEIGHT/HOLD/UNDERWEIGHT/SELL

#### Borrowable Pattern 1: Structured Debate with Round Limits

Bull and Bear researchers alternate arguments, each referencing the other's last point plus past memory of similar situations. Conditional routing ensures convergence.

```python
# TradingAgents/tradingagents/agents/utils/conditional_logic.py:46-67
def should_continue_debate(self, state: AgentState) -> str:
    if state["investment_debate_state"]["count"] >= 2 * self.max_debate_rounds:
        return "Research Manager"
    if state["investment_debate_state"]["current_response"].startswith("Bull"):
        return "Bear Researcher"
    return "Bull Researcher"
```

The debate prompt structure forces adversarial engagement:

```python
# TradingAgents/tradingagents/agents/researchers/bull_researcher.py:3-56
prompt = f"""You are a Bull Analyst advocating for investing in the stock...
Key points to focus on:
- Growth Potential...
- Competitive Advantages...
- Bear Counterpoints: Critically analyze the bear argument with specific data...
Conversation history of the debate: {history}
Last bear argument: {current_response}
Reflections from similar situations: {past_memory_str}
"""
```

**Quantbot application:** Track C debate agents. Instead of the current Python fan-out/fan-in graph, implement a structured Rust debate loop: TSMOM case → LLM counter-case → round limit → adjudicator. The round limit (2-3 max) controls latency. The adjudicator maps to quantbot's combiner but with richer context than numeric blending.

#### Borrowable Pattern 2: BM25 Memory — Offline, No API Calls

Uses Best Matching 25 lexical search to find similar past market situations. No embedding API required — works offline with zero cost.

```python
# TradingAgents/tradingagents/agents/utils/memory.py:1-145
class FinancialSituationMemory:
    def get_memories(self, current_situation: str, n_matches: int = 1) -> List[dict]:
        # Tokenize query, get BM25 scores, return top-n matches with similarity
        scores = self.bm25.get_scores(query_tokens)
        # Returns: matched_situation, recommendation, similarity_score
```

Memory is built via reflection after observing returns:

```python
# TradingAgents/tradingagents/agents/utils/reflection.py:57-80
def _reflect_on_component(self, component_type, report, situation, returns_losses):
    messages = [
        ("system", detailed_reflection_prompt),
        ("human", f"Returns: {returns_losses}\nAnalysis: {report}\nMarket context: {situation}")
    ]
    # Result is added to memory for future similar trades
```

**Quantbot application:** Implement a `RegimeMemory` in Rust backed by SQLite. Store (market_conditions_text, indicator_recommendation, actual_5d_return) tuples. On each run, compute BM25 similarity to current TA snapshot, inject top-2 memories into the LLM prompt: "In similar conditions on 2024-08-15, going SHORT was correct (5d return: -2.3%)." This is a natural extension of FinMem (§G) but without the embedding cost. Rust BM25 crates: `bm25` or implement directly (IDF + TF scoring over tokenized text).

#### Borrowable Pattern 3: 5-Tier Rating Scale

Bridges verbose LLM reasoning to discrete trading actions:

```python
# TradingAgents/tradingagents/graph/signal_processing.py:1-33
def process_signal(self, full_signal: str) -> str:
    messages = [
        ("system", "Extract the rating as exactly one of: BUY, OVERWEIGHT, HOLD, UNDERWEIGHT, SELL..."),
        ("human", full_signal),
    ]
    return self.quick_thinking_llm.invoke(messages).content
```

| Rating | Position Action |
|---|---|
| BUY | Full weight per asset-class allocation |
| OVERWEIGHT | 75% of full weight |
| HOLD | Maintain existing position (no change) |
| UNDERWEIGHT | 25% of full weight |
| SELL | Exit / reverse direction |

**Quantbot application:** Replace the current binary LONG/SHORT/FLAT with a 5-tier output from the LLM indicator. Map to position scale factors: BUY=1.0, OVERWEIGHT=0.75, HOLD=existing, UNDERWEIGHT=0.25, SELL=-1.0. This naturally bridges the gap between "indicator says SHORT" and "how much to short" — currently lost in the combiner's strength×confidence multiplication.

#### Borrowable Pattern 4: Modular Analyst Plugins with Tool Bindings

Each analyst type gets specific tools:

```python
# TradingAgents/tradingagents/graph/setup.py:54-85
if "market" in selected_analysts:
    analyst_nodes["market"] = create_market_analyst(self.quick_thinking_llm)
    tool_nodes["market"] = self.tool_nodes["market"]  # MACD, RSI, SMA, etc.
if "fundamentals" in selected_analysts:
    # get_fundamentals, get_balance_sheet, get_cashflow, get_income_statement
```

```python
# TradingAgents/tradingagents/graph/trading_graph.py:156-190
self.tool_nodes = {
    "market": ToolNode([get_stock_data, get_indicators]),
    "social": ToolNode([get_news]),
    "news": ToolNode([get_news, get_global_news, get_insider_transactions]),
    "fundamentals": ToolNode([get_fundamentals, get_balance_sheet, ...])
}
```

**Quantbot application:** Define `IndicatorTool` trait in Rust — each indicator agent gets access to specific TA functions. Market analyst gets MACD/RSI/Bollinger (already in `ta.rs`), trend analyst gets trendlines/support-resistance (Track C), macro analyst gets VIX/rate differentials (Phase 4). Tool selection can be specified in TOML config per agent.

#### Borrowable Pattern 5: Two-Tier LLM Strategy

```python
# TradingAgents/tradingagents/default_config.py:1-37
DEFAULT_CONFIG = {
    "deep_think_llm": "gpt-5.4",       # Complex reasoning (Research Manager, Portfolio Manager)
    "quick_think_llm": "gpt-5.4-mini", # Quick tasks (Analysts, debaters)
}
```

**Quantbot application:** Use Fin-R1 7B for per-instrument indicator signals (fast, 6 calls/run), use a larger model (qwen3:32b or Claude) for the weekly meta-reasoning or overlay classification (1 call/week). Current architecture uses the same model for everything — wasteful on simple tasks, underpowered on complex ones.

#### Borrowable Pattern 6: Hierarchical State with Nested Debate Substates

```python
# TradingAgents/tradingagents/agents/utils/agent_states.py:1-73
class InvestDebateState(TypedDict):
    bull_history: str      # All bull contributions
    bear_history: str      # All bear contributions
    history: str           # Full debate chronology
    current_response: str  # Latest message
    judge_decision: str    # Research Manager's final call
    count: int             # Debate round counter
```

Agents append to state without overwriting:

```python
# From bull_researcher.py
new_investment_debate_state = {
    "history": history + "\n" + argument,        # Append
    "bull_history": bull_history + "\n" + argument,
    "bear_history": investment_debate_state.get("bear_history", ""),  # Preserve
    "current_response": argument,
    "count": investment_debate_state["count"] + 1,
}
```

**Quantbot application:** For Track C debate, define `DebateState` struct in Rust:

```rust
struct DebateState {
    bull_history: Vec<String>,
    bear_history: Vec<String>,
    current_speaker: Speaker,
    round: usize,
    judge_decision: Option<SignalDirection>,
}
```

Natural for append-only state machines. Serialize to SQLite for replay.

#### What to Ignore

- **Equity/single-stock focus** — designed for NVDA-style analysis, not multi-asset TSMOM
- **Full LangGraph dependency** — heavy Python orchestration. Quantbot should implement the debate *pattern* in Rust, not depend on LangGraph
- **yfinance data dependency** — TradingAgents uses live yfinance; quantbot has its own CSV pipeline
- **No backtesting engine** — TradingAgents makes single-point decisions with no backtest infrastructure

### 5.3 Research Survey — QuantAgent, OpenStock, FinceptTerminal, awesome-ai-in-finance

Reviewed 2026-04-17. Four repos assessed for quantbot integration potential.

#### QuantAgent (Y-Research-SBU) — Vision LLM Chart Analysis

**Source:** [Y-Research-SBU/QuantAgent](https://github.com/Y-Research-SBU/QuantAgent) — already reviewed in §2.1.

Additional observations from deep-dive: the multi-report synthesis pattern in `decision_agent.py` (3 independent analyst reports → single JSON decision with risk-reward ratio) is a simpler version of TradingAgents' debate. Their provider abstraction (`TradingGraph._create_llm()`) supports per-agent model selection (mini for indicators, large for decisions) — matches the two-tier LLM strategy recommended in §5.2.

**Verdict:** Architecture reference for Track C vision agents. No production-usable code.

#### OpenStock (Open-Dev-Society) — LLM Fallback Pattern

**Source:** [Open-Dev-Society/OpenStock](https://github.com/Open-Dev-Society/OpenStock)

Consumer stock tracking platform (Next.js/TypeScript). One borrowable pattern: **multi-provider LLM fallback** — primary model (Gemini) fails → automatic switchover to secondary (Siray.ai) with zero downtime.

**Quantbot application:** Add `fallback_url`/`fallback_model` fields to `LlmConfig`. In `LlmClient::chat()`, if primary returns 5xx/timeout after retries, try fallback before returning Flat. Simple resilience for production runs.

**Verdict:** Single pattern worth borrowing. Not an integration target.

#### FinceptTerminal (Fincept-Corporation) — SuperAgent Router + Multi-Engine Backtest

**Source:** [Fincept-Corporation/FinceptTerminal](https://github.com/Fincept-Corporation/FinceptTerminal)

C++/Qt6 Bloomberg-class desktop terminal. 30+ LLM agent personas (Buffett, Dalio, macro strategists), multi-engine backtesting (Zipline/VectorBT/Backtrader), and broker adapters including IG.

Borrowable patterns:
1. **SuperAgent router** — LLM-based intent classification (not keyword matching) to route queries to the right agent. More sophisticated than quantbot's static `BlendCategory` routing.
2. **Unified backtest provider interface** — `BaseProvider` abstraction across multiple backtest engines with common `PerformanceMetrics`/`Trade` types.
3. **Multi-persona agents** — 20+ investor personalities. For quantbot: instead of one "trading analyst" prompt, generate signals from specialized perspectives (momentum analyst, mean-reversion analyst, macro analyst) and blend outputs.

**Verdict:** Architecture reference, especially for multi-persona agent design (Phase 4+). C++/Python stack prevents code reuse.

#### awesome-ai-in-finance (georgezouq) — Curated Reference List

**Source:** [georgezouq/awesome-ai-in-finance](https://github.com/georgezouq/awesome-ai-in-finance)

Actively maintained curated list. Most relevant entries for quantbot:

| Entry | Category | Relevance |
|---|---|---|
| [ATLAS](https://github.com/chrisworsey55/atlas-gic) | Multi-agent | Deep-dived above (§5.1) |
| [TradingAgents](https://github.com/TauricResearch/TradingAgents) | Multi-agent | Deep-dived above (§5.2) |
| [Ensemble-Strategy](https://github.com/AI4Finance-LLC/Deep-Reinforcement-Learning-for-Automated-Stock-Trading-Ensemble-Strategy-ICAIF-2020) | Signal blending | DRL ensemble for signal combination. Academic foundation for adaptive blend weights |
| [skfolio](https://github.com/skfolio/skfolio) | Portfolio opt | sklearn-based portfolio optimization. Could inform risk/sizing math |
| [alphalens](https://github.com/quantopian/alphalens) | Factor analysis | **Actionable:** validate whether LLM indicator signal adds alpha via factor tear sheets |
| [CRNG](https://github.com/brotto/crng) | Risk | Fat-tail distribution modeling. Relevant for circuit breaker calibration |
| [MarS](https://github.com/microsoft/MarS) | Simulation | Microsoft market simulation engine. Backtest architecture reference |
| "Financial Statement Analysis with LLMs" (SSRN 4835311) | Paper | GPT-4 outperforms analysts. Sharpe improvement validation |
| "LLMs Meet Finance" (arXiv 2504.13125) | Paper | Fine-tuning pipeline (SFT → DPO → RL) for trading LLMs. Actionable for Fin-R1 successors |

**Verdict:** Bookmark as ongoing reference. alphalens specifically worth using to validate LLM signal quality using cached signals.

### Phase Relevance Map

| QuantBot Phase | Most Relevant Papers & Repos |
|---|---|
| Phase 2 (LLM agents) | TradingAgents (H, §5.2), StockAgent (I), MarketSenseAI (N), FinAgent (L), QuantAgent-SBU, ai-hedge-fund |
| Phase 3 Track A (TSMOM + IG) | nofx (circuit breaker), Qlib (U, online rolling for production) |
| Phase 3 Track B (LLM agents in Rust) | ATLAS (§5.1, meta-weighting), TradingAgents (§5.2, debate+memory), MambaStock (P) |
| Phase 3 Track D (overlays) | ATLAS (§5.1, reflexivity engine), FinceptTerminal (§5.3, SuperAgent router) |
| Phase 4 (Extensions) | TimesFM (S), Fin-R1 (K), QuantAgent-HKUST (J), CliffordNet (R), TorchTrade (RL weighting), alphalens (factor validation) |
| Phase 5+ (Meta-reasoning) | Nemotron-Cascade (T), Qlib RD-Agent (U), ATLAS autoresearch (§5.1) |
| General reference | RL survey (Q), awesome-ai-in-finance (§5.3) |

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
| Backtest engine + metrics | `src/backtest/` | 13 | Next-open execution, mark-to-market at close, `generate_targets()` for paper-trade |
| Execution router | `src/execution/router.rs` | 22 | Per-instrument specs, lot rounding, spread costs |
| CLI (clap) | `src/main.rs` | — | `backtest` + `paper-trade` subcommands, stubs for live/positions |

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

**Paper-trade mode implemented (2025-03-31):**

Added `paper-trade` CLI command — a single-shot signal pipeline that runs TSMOM on the latest CSV data and outputs target positions, orders, margin, and spread costs.

Architecture: `generate_targets()` on `BacktestEngine` runs one iteration of signal→risk limits→sizing→diff→orders. Reuses all existing logic (TSMOMAgent, ExecutionRouter, risk limits) without modifying the backtest `run()` loop.

Sample output (2025-03-28 data):
- **Long gold:** GLD at +0.40 weight (1,388 shares, $400K) + GC=F at +0.40 weight (1 lot, $312K). Both at max weight — 40%+ 252-day returns
- **Short SPY:** -0.16 weight (282 shares, $156K). Weak short — negative 21d/63d momentum but positive 252d
- **FX:** Long GBPUSD +0.33, Short USDCHF -0.36, Short USDJPY -0.26. Dollar weakness theme
- **Total margin:** $158,544 (15.9% of NAV) — conservative

**Known limitation — correlated gold exposure:**
GLD + GC=F are ~0.99 correlated but 1/N allocation treats them as independent. Combined notional is ~$712K on a $1M portfolio (71% gold exposure). The TSMOM-only 6-instrument universe has this structural concentration risk. Resolution: when the per-instrument router gets the signal combiner (Track B), add correlation-aware position limits or dedup correlated exposures. Acceptable for now — TSMOM-only on 6 instruments is the validated strategy.

**Qlib evaluation:**
Evaluated [microsoft/qlib](https://github.com/microsoft/qlib). Impressive ML research platform (LightGBM, LSTM, Transformer model zoo) but wrong tool: Python-only, equity/Chinese A-share focused, overkill architecture for multi-asset TSMOM. Two ideas worth stealing later: (1) RD-Agent pattern for LLM-driven factor discovery maps to the planned indicator agent, (2) online model rolling pipeline for production drift management.

**75 tests passing, clean clippy.**

**Next: position state persistence (JSON file for consecutive paper-trade diffs), then IG API client.**

### 2025-03/04 — Track A: IG Execution + Live Infrastructure (PRs 1-8)

**PR 1 — Boundary + Config + Compile-Time Shape:**

Established the execution trait boundary and TOML config. `ExecutionEngine` trait with async methods (health_check, get_positions, place_orders, flatten_all). `PaperExecutionEngine` for simulated fills. `AppConfig` with `IgConfig` + per-instrument `InstrumentConfig` including `ig_point_value`. `config.example.toml` with all 6 instruments.

**PR 2 — IG Demo Round-Trip:**

Full `IgClient` REST client: auth (CST/X-SECURITY-TOKEN), rate limiting (1050ms between calls), retry on 5xx, re-auth on 401. `IgExecutionEngine` via `tokio::sync::Mutex<IgClient>` — sequential orders with 500ms confirm delay. `SymbolMapper` for bidirectional symbol↔epic lookup. Safety valves: `--instrument`, `--max-orders`, `--max-size`, `--flatten`. JSONL audit logging to `data/audit/`. Integration test passing on IG demo account Z69YJL (auth→place 0.5 GBPUSD→confirm→flatten, 12s end-to-end).

Key learning: `tokio::sync::Mutex` required (not `std::sync`) because guard is held across `.await` points. reqwest needs `native-tls` — `rustls-tls` fails to connect to IG's API.

**PR 3 — Reconciliation + Safety:**

`positions_to_signed()`, `compute_deltas()`, `verify_positions()` with per-instrument tolerance. `CircuitBreaker` for consecutive failure tracking and pre-trade checks. `positions` subcommand. Full live loop: generate targets → fetch positions → compute deltas → circuit breaker → place deltas → post-trade verify → save state. Running twice → 0 orders (idempotent). Dust deltas tracked and reported.

**PR 4 — Audit Logging + Run Summaries:**

`AuditLogger` with `BufWriter<File>`, per-run JSONL append. Events: run_start, targets, auth_ok, health_check_ok, positions_fetched, reconcile, breaker_check, execution_skipped, orders_submitted, orders_confirmed, verify, run_end. `RunSummary` struct for `--json` output. Write failures never block trading.

**PR 4b — Audit Log Polish:**

Schema v2. Timestamps normalized to Z suffix. `signed_qty` renamed to `signed_deal_size`. Float noise removed (1dp rounding). `auth_ok`/`health_check_ok` events added.

**PR 5 — SQLite Recording + History CLI:**

`rusqlite` (bundled) added. `src/db.rs` with 4 tables (runs, signals, orders, positions), WAL mode, `PRAGMA busy_timeout=5000`. `src/recording.rs` — `Recorder` struct with typed `record_*` methods, `write_failed` tracking. `history` subcommand with `--run`, `--instrument`, `--last`, `--json` filters. DB at `data/quantbot.db`.

**PR 5b — SQLite Polish:**

`PRAGMA user_version` for schema versioning with migration support (v1→v2). Batch inserts in transactions. `--status` and `--date` filters for history. `db_write_failed` flag in `RunSummary`.

**PR 6 — Risk Agent:**

`src/agents/risk/mod.rs` — `RiskAgent` with hard-veto authority. Checks: gross leverage, per-instrument exposure, drawdown from peak NAV. `RiskConfig` in TOML (optional `[risk]` section). `risk_state` table in SQLite for peak NAV persistence. `risk_check` audit event. Veto → outcome `RISK_VETO`, logged to audit + SQLite. 10 unit tests.

**PR 7 — Data Pipeline (Yahoo Update + Freshness Gate):**

`YahooClient` (`src/data/yahoo.rs`): Yahoo v8 chart API with rate limiting (500ms), null filtering, serde response parsing. `DataUpdater` (`src/data/updater.rs`): CSV merge/append, last_date detection, `update_all` orchestrator, `discover_symbols`. `freshness.rs`: `previous_trading_day` weekday logic, `check_all_fresh`, max_stale_days tolerance (default 3). `data` CLI subcommand. Freshness gate in `run_live` with `--allow-stale` override. 20 unit tests + 5 Yahoo mockito tests.

Key learning: mockito tests need `.no_proxy()` on reqwest client to bypass HTTP proxy on cluster login node. Also fixed in IG client tests.

**PR 8 — NAV Mark-to-Market:**

`src/execution/mtm.rs` — `mark_to_market()` pure function computing NAV = initial_cash + Σ(signed_size × (current_price - open_level)) from live IG positions + latest bar close prices. MTM NAV feeds into `generate_targets` sizing, risk agent drawdown check, state file save, and audit log. IG engine created early in `run_live` and reused for MTM + `run_rebalance` (single auth). Paper engine falls back to state file NAV. 6 unit tests.

**200+ tests passing, clean clippy. Track A complete.**

---

### 2026-04 — Track B: Multi-Agent Plumbing

**PR B1 — Multi-Agent Plumbing + Dummy RSI Indicator (2026-04-04):**

Track A complete (8 PRs, 200+ tests). Track B begins with compile-time shape and multi-agent recording infrastructure.

| Component | Location | Tests | Notes |
|---|---|---|---|
| `SignalAgent` trait | `src/agents/mod.rs` | — | Object-safe: `name()`, `signal_type()`, `generate_signal()`. `Box<dyn SignalAgent>` ready for B4/B5 |
| `impl SignalAgent for TSMOMAgent` | `src/agents/tsmom/mod.rs` | 0 new | Delegates to existing inherent method — Rust prefers inherent for direct calls, so 200+ tests unaffected |
| `DummyIndicatorAgent` (RSI) | `src/agents/indicator/mod.rs` | 7 | 14-period RSI via Wilder's smoothing (inline, no `ta` crate). RSI<30→Long, RSI>70→Short, else Flat |
| SQLite schema v3 | `src/db.rs` | 1 new | `agent_name TEXT NOT NULL DEFAULT 'tsmom'` on signals, migration from v2, `idx_signals_agent` index |
| `SignalRecord` type | `src/recording.rs` | 0 new (updated) | Replaces `HashMap<String, (SignalDirection, f64, f64)>` for multi-agent provenance |
| Indicator in `run_live` | `src/main.rs` | — | Behind `#[cfg(feature = "track-b")]`. Advisory signals recorded to SQLite with `weight=0`. Printed in report. |
| `track-b` cargo feature | `Cargo.toml` | — | No extra deps — RSI is inline arithmetic |

**Design decisions:**
- `DummyIndicatorAgent` produces real LONG/SHORT/FLAT signals so the routing/combiner (PR B5) can be tested before LLM integration. Only TSMOM drives weights/sizing in B1 — indicator signals are recorded but advisory.
- `generate_targets()` signature stays `&TSMOMAgent`, not `&[&dyn SignalAgent]` — generalization deferred to B4/B5.
- No changes to `TargetSnapshot`, `run_backtest`, `run_paper_trade`, or audit JSONL in B1.

**All tests pass with and without `track-b`, clean clippy.**

**PR B2 — LLM Indicator Client + TA Features (2026-04-04):**

Replaced DummyIndicatorAgent's inline RSI with a full TA computation suite and an LLM-based indicator agent. All code gated behind `track-b` feature.

| Component | Location | Tests | Notes |
|---|---|---|---|
| TA computations | `src/agents/indicator/ta.rs` | 15 | Extracted `compute_rsi`, added SMA, EMA, MACD, Bollinger, ATR. `TaSnapshot::compute()` + `format_for_prompt()` |
| LLM HTTP client | `src/agents/indicator/llm_client.rs` | 5 | OpenAI-compatible `/v1/chat/completions`. Rate limiting (200ms), retry on 5xx, `LlmConfig` with serde defaults |
| Response parser | `src/agents/indicator/parser.rs` | 11 | Strip `<think>` blocks → markdown fences → JSON parse → regex fallback. Clamps values, direction aliases |
| System prompt | `src/agents/indicator/prompt.txt` | — | Trading analyst role, JSON output schema, indicator guidelines. `include_str!` |
| LLM indicator agent | `src/agents/indicator/llm_agent.rs` | 3 | `LlmIndicatorAgent` with `tokio::sync::Mutex<LlmClient>`. Async→sync via `block_in_place`. Graceful degradation → Flat |
| Config + wiring | `src/config.rs`, `src/main.rs` | 1 | Feature-gated `llm: Option<LlmConfig>`. `Box<dyn SignalAgent>` dynamic dispatch. Agent column in display |

**Design decisions:**
- `SignalAgent` stays sync/object-safe — bridged with `block_in_place` + `Handle::current().block_on()`.
- Sequential LLM calls per instrument (6 × 30s max = 180s worst case). Acceptable for advisory signals.
- `OnceLock<Regex>` for think-block regex (compiled once). `regex` crate added to deps.
- TOML ignores unknown keys when `track-b` disabled — existing configs work unchanged.

**All tests pass with and without `track-b`, clean clippy. +1350 lines.**

**PR B3 — Per-Instrument Signal Combiner + Pipeline Integration (2026-04-04):**

Wires indicator signals into sizing via per-asset-class combiner with configurable blend weights. Absorbs the planned PR B5 per-instrument router — fixed global weights are wrong, instrument-type routing is the alpha (gold 50/50, equity 100/0 TSMOM, forex 10/90 indicator-heavy). Blending gated: `enabled=false` preserves existing TSMOM-only behavior.

| Component | Location | Tests | Notes |
|---|---|---|---|
| Blend config types | `src/config.rs` | 3 | `BlendCategory`, `BlendWeights`, `BlendConfig` with `weights_for()` safe lookup. Validation warns missing categories, errors on zero-sum |
| Signal combiner | `src/agents/combiner.rs` (NEW) | 11 | `blend_category()`, `combine_signals()`, `build_combined_signal()`. Vol-scalar normalization, graceful TSMOM-only fallback |
| Engine refactor | `src/backtest/engine.rs` | 1 | Shared `build_snapshot()` helper, `generate_targets_with_overrides()` for combined pipeline |
| Pipeline integration | `src/main.rs` | — | `run_live` + `run_paper_trade` blending, latency tracking, blending summary display, 3-layer SQLite recording (tsmom/indicator/combined) |

**Design decisions:**
- Per-asset-class routing via `BlendCategory` enum, not global 80/20. `BlendCategory` is separate from `AssetClass` (GLD=Equity and GC=F=Futures both map to Gold).
- Combiner is a pure function: no state, no async, fully testable. Vol-scalar from TSMOM signal ensures apples-to-apples scale for indicator weights.
- Graceful fallback to TSMOM-only when indicator is flat, confidence=0, llm_success=0, or missing — per instrument, with `indicator_used` flag logged.
- `build_snapshot()` refactor eliminates code duplication between `generate_targets()` and `generate_targets_with_overrides()`.
- Paper-trade requires `--config` flag for blending; absent or `enabled=false` → TSMOM-only, zero behavior change.
- Backtest unchanged — blending in backtest deferred until cache/replay or deterministic heuristic mode (PR B4/B5).

**All tests pass with and without `track-b`, clean clippy. +950 lines.**

**PR B4 — Runtime Prompt Loading with Hash Provenance (2026-04-04):**

Decoupled system prompt from compiled binary. `PromptLoader` loads from optional `prompt_path` file with graceful fallback to embedded `prompt.txt`. SHA-256 hash (truncated to 16 hex chars) of raw file bytes provides deterministic provenance for cache/replay and prompt A/B testing. No normalization — any edit changes the hash.

| Component | Location | Tests | Notes |
|---|---|---|---|
| Prompt loader | `src/agents/indicator/prompt_loader.rs` (NEW) | 7 | `load()` with file/embedded fallback, empty file detection, `sha256_short()` reusable hash |
| Config | `src/agents/indicator/llm_client.rs` | — | `prompt_path: Option<String>` on `LlmConfig` |
| Agent wiring | `src/agents/indicator/llm_agent.rs` | — | Uses `PromptLoader` instead of `include_str!`, exposes `loaded_prompt()` accessor |
| Audit event | `src/audit.rs` | — | `prompt_info` event (hash, source, model) |
| SQLite schema v4 | `src/db.rs` | 2 | `prompt_hash`, `prompt_source`, `llm_model` nullable columns on `runs`, v3→v4 migration |
| Recorder | `src/recording.rs` | — | `record_prompt_info()` writes to runs row |

**All tests pass with and without `track-b`, clean clippy.**

**PR B5a — LLM Cache Write-Through (2026-04-04):**

Every LLM indicator call (success or error) is now cached to SQLite with a deterministic key = `(llm_model, prompt_hash, instrument, eval_date, ta_hash)`. INSERT OR IGNORE semantics ensure entries are never overwritten. Enables deterministic replay in B5b.

| Component | Location | Tests | Notes |
|---|---|---|---|
| Cache table | `src/db.rs` | 5 | Schema v5: `llm_cache` table, `LlmCacheEntry` struct, `insert_llm_cache` (INSERT OR IGNORE), `get_llm_cache` (for B5b), v4→v5 migration |
| Signal trait | `src/agents/mod.rs` | — | `take_cache_entries()` default method on `SignalAgent` (returns empty vec for non-LLM agents) |
| Agent caching | `src/agents/indicator/llm_agent.rs` | 7 | Collects cache entries in `generate_signal_async` (eval_date from bars, ta_hash from user prompt, latency measured). `take_cache_entries()` drains accumulated entries |
| Recorder | `src/recording.rs` | — | `record_llm_cache_entries()` non-blocking batch write with count logging |
| Pipeline | `src/main.rs` | — | `run_live` writes via recorder, `run_paper_trade` writes directly to Db |

**Design decisions:**
- `ta_hash` = SHA-256 of the exact user prompt string sent to `chat()` (includes instrument prefix + formatted TA snapshot). Same bars + same instrument = same hash, regardless of prompt source.
- `cache_key` = pipe-delimited composite: `model|prompt_hash|instrument|eval_date|ta_hash`. Human-readable, deterministic.
- `std::sync::Mutex` for `cache_entries` (brief lock, no .await held), `tokio::sync::Mutex` for LLM client (held across .await).
- Cache write failures never block trading — same pattern as all other SQLite writes.
- `sha256_short()` extracted as pub function in `prompt_loader.rs` for reuse.

**All tests pass with and without `track-b`, clean clippy. +480 lines.**

**PR B5b — LLM Client Fix for Ollama Thinking Models (2026-04-05):**

Fixed LLM client to work with Ollama thinking models (qwen3, Fin-R1) that use a separate `reasoning` field instead of `<think>` blocks. Diagnosed via raw body logging — content was empty because: (1) `content: null` crashed serde, (2) thinking tokens consumed the entire 512-token budget, (3) 30s timeout too short for local inference.

| Component | Location | Notes |
|---|---|---|
| Response types | `src/agents/indicator/llm_client.rs` | `content: Option<String>`, `reasoning: Option<String>` on `ChatMessage`. `stream: false` in request. Diagnostic body snippet (300 chars) on empty/parse failures |
| Token budget | `src/agents/indicator/llm_client.rs` | Default `max_tokens` 512→4096 (thinking models need CoT + answer) |
| Config | `config.example.toml` | `max_tokens = 4096`, `timeout_secs = 120` |
| System prompt | `prompts/indicator_system.md` | Moved from `src/agents/indicator/prompt.txt` as single source of truth. `include_str!` path updated in `prompt_loader.rs` |

**Verified end-to-end on both qwen3:14b and Fin-R1:Q5.** Fin-R1 notably more opinionated (strong short on gold) vs qwen3's conservative flat calls. Both produce valid parsed JSON with `llm_ok=1, parse_ok=1` for all 6 instruments.

**+70 lines net, 2 new tests (null content, reasoning-without-content).**

---

**PR B5c — Replay Harness: CachedIndicatorAgent + eval replay (2026-04-05):**

Offline deterministic replay of cached LLM indicator responses through the backtest engine. Enables Sharpe comparison of blended (TSMOM + LLM) vs TSMOM-only strategies without network calls or GPU time.

| Component | Location | Notes |
|---|---|---|
| CachedIndicatorAgent | `src/agents/indicator/cached_agent.rs` (NEW) | `SignalAgent` impl. Reconstructs cache keys identically to `LlmIndicatorAgent`. Cache miss → Flat + `llm_success=0.0`. `CoverageReport` with hit/miss per instrument. 8 tests |
| Coverage query | `src/db.rs` | `llm_cache_coverage(model, prompt_hash)` — pre-flight count of OK cache entries per instrument. 1 test |
| Blended backtest | `src/backtest/engine.rs` | `run_blended()` — daily TSMOM → indicator → combiner → risk limits → sizing. Feature-gated `track-b`. Separate from `run()` |
| CLI | `src/main.rs` | `quantbot eval replay --config --model --prompt-hash [--start --end --eval-start --instruments --json]`. Runs blended + TSMOM-only baseline, prints side-by-side comparison + coverage report |
| vol_scalar fix | `src/agents/tsmom/mod.rs` | Moved EWMA vol computation before `avg_sign==0` early return. Flat TSMOM signals now carry `vol_scalar`/`ann_vol` metadata for correct indicator weight scaling |

First end-to-end test showed 1.5% cache coverage (1 entry per instrument) → identical blended/TSMOM results (expected). Full comparison requires batch cache population across the eval window.

**+905 lines (new file + modifications), 14 new tests, 284 total passing.**

---

**PR B6 — Confidence Gating for Indicator Signals (2026-04-06):**

15-month eval replay showed the LLM indicator is PnL-neutral but adds 41 extra trades, creating spread cost drag that reduces Sharpe from 1.394 to 1.278. Confidence gating filters weak signals before blending.

| Component | Location | Tests | Notes |
|---|---|---|---|
| GatingConfig | `src/config.rs` | 1 | `min_confidence` + `min_abs_strength`, serde defaults 0.0 (no gating). Optional on `BlendConfig` |
| Gating logic | `src/agents/combiner.rs` | 3 | `should_use_indicator()` rejects below thresholds, `combine_signals()` threads from `blend_config.gating` |
| Example config | `config.example.toml` | — | Commented `[blending.gating]` section (0.70/0.30 suggested) |

Design: gating lives inside `BlendConfig` so `combine_signals()` signature is unchanged — zero call-site modifications needed. No hysteresis in v1 (stateless); simple thresholds address the diagnosed problem (low-conviction churn). Hysteresis can be added later if edge-of-boundary oscillation appears.

**+123 lines, 4 new tests.**

---

**Ablation Study — Fin-R1 + Baseline Prompt, No Evidence of Alpha (2026-04-06):**

Systematic ablation of LLM indicator blending over 15-month eval window (2024-01-01 → 2025-03-31, 98.7% cache coverage). Goal: determine whether Fin-R1 indicator adds net value after realistic IG spread costs.

| Config | Sharpe | Δ vs TSMOM | Extra Trades | Spread Residual |
|---|---|---|---|---|
| TSMOM-only (baseline) | 1.394 | — | — | — |
| Ungated (all indicator) | 1.278 | -0.116 | +41 | — |
| Gated 0.70/0.30 | 1.314 | -0.080 | +34 | 36,013 (19.3%) |
| Forex off, gold 50/50 | 1.365 | -0.029 | +23 | 14,657 (7.5%) |

Per-instrument attribution (forex-off ablation):

| Instrument | Blnd PnL | TSMOM PnL | Delta | Ind Used% |
|---|---|---|---|---|
| GC=F | 126,930 | 130,690 | -3,760 | 13% |
| GLD | 99,565 | 99,823 | -258 | 13% |
| SPY | 77,449 | 76,669 | +780 | 9% |
| GBPUSD=X | -32,908 | -31,547 | -1,361 | 5% |
| USDCHF=X | -43,257 | -42,942 | -315 | 6% |
| USDJPY=X | -17,447 | -18,312 | +866 | 8% |

Key findings:
- Ablation ladder is monotonic: removing indicator exposure strictly improves Sharpe
- FX indicator (90% weight) was the primary drag source — USDJPY consistently destructive even at low usage
- Gold indicator (50/50) is PnL-neutral at best, GC=F slightly harmful due to low trade count amplifying wrong calls
- Indicator fires on 5-13% of days with signals that are directionally coin-flip quality
- Confidence gating (0.70/0.30) reduced but could not eliminate the drag

Conclusion: Fin-R1 + baseline prompt `8430ffc768a841ee` does not add alpha under realistic costs. Production default set to TSMOM-only (`blending.enabled = false`). Research pipeline preserved for prompt/model A/B testing.

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
- NVIDIA. (2025). "Nemotron-Cascade-2-30B-A3B." [Hugging Face](https://huggingface.co/nvidia/Nemotron-Cascade-2-30B-A3B).
- (2024). "Reinforcement Learning in Financial Decision Making: A Systematic Review." *arXiv preprint arXiv:2411.07585*.

---

## 8. Continuous Bot Architecture: Deterministic Core + Bounded Overlays

### Design Principle
Keep the trading loop deterministic and safe; let "adaptive/news intelligence" act as a controlled overlay — not an unbounded driver. Otherwise you get non-reproducible behavior and brittle performance.

### Target Architecture

**A. Always-on daemon ("operator")**
Long-running process: schedules jobs (market open/close, daily rebalance, news polling), manages state/recovery, exposes health endpoints. Implementation: systemd/supervisord + cron-like scheduling inside the daemon.

**B. Deterministic strategy engine ("decision core")**
Given timestamp t, market data up to t, and config: compute signals → apply risk → produce target weights → produce orders. Must be replayable from recorded inputs.

**C. Reactive news/risk overlay ("veto/modifier")**
Separate component that can: veto trades, scale exposure down (risk-off), temporarily disable instruments, tighten thresholds/gating. Key constraint: overlay actions must be explainable, logged, and bounded (small number of allowed actions).

**D. Execution + reconciliation + circuit breakers ("safety kernel")**
Already built: reconciliation, circuit breaker, audit logs, SQLite recording. Non-negotiable in an always-on bot.

### "Adjust strategies" without breaking reproducibility
- **Safe:** configuration selection — maintain pre-defined strategy configs, select among them daily/weekly based on regime indicators. Mechanism: bandit/rules-based router with guardrails.
- **Unsafe:** online learning / self-modifying rules — updating weights/prompts live based on short-term PnL is extremely prone to overfitting and "ghost behavior." Do this offline first (paper trading + replay), promote via versioned config.

### Reacting to news and market conditions
- **Market conditions (cheap, reliable, deterministic):** volatility regime (ATR%, realized vol), trend regime (SMA slope/breakout), correlation/concentration checks, liquidity/market-hours constraints. Reproducible and backtestable.
- **News reaction (risk overlay, not primary alpha):** main strategy = quant (TSMOM/blended); news agent = "risk manager" that reduces risk when uncertainty spikes. Concrete actions: "no new positions today," "halve gross leverage for 48h," "disable instrument X for 24h," "tighten gating thresholds." Keep point-in-time alignment (no look-ahead).

### Build Order
1. **Typed overlay actions + persistence** (first — highest leverage)
2. **Overlay sources:** volatility/market-condition overlay (deterministic), then news overlay (bounded)
3. **Intraday bars** (separate from daily TSMOM pipeline)
4. **Daemon + scheduling** (last — ops work once overlays are stable)

### Intraday Cadence (not HFT)
- Periodic: every 30 min during liquid hours (13:30–20:00 UTC), every 2–4 hours outside
- Event triggers: volatility spike, large move (>1.5σ), news detection
- Daily TSMOM determines core direction and max risk budget; intraday overlay adjusts timing/position scaling (small deltas, not full flips)

---

## 9. AutoHedge Review — What to Borrow vs What to Ignore

**Reference:** [The-Swarm-Corporation/AutoHedge](https://github.com/The-Swarm-Corporation/AutoHedge)

### Snapshot
AutoHedge is best understood as a Python **agent hedge fund framework/template** for highly autonomous trading, currently oriented toward **crypto/Solana execution** with role-based agents (director → quant → risk → execution), structured outputs, and continuous operation.

### Bottom Line
Useful as a **framework inspiration**, not as a direct integration candidate for quantbot.

Quantbot's current design target is stronger on the dimensions that matter for this repo:
- deterministic core
- bounded overlays
- replayable evaluation
- explicit risk vetoes
- SQLite/audit trail

AutoHedge would mostly add autonomy and orchestration complexity, not solve the current bottlenecks (signal quality, overlay calibration, live ops validation).

### What to Borrow

#### 1. Strict role separation
Portable design pattern:
- strategy/signal producer
- risk gate
- execution planner
- execution adapter

Quantbot already has the seeds of this; the takeaway is to keep those boundaries crisp and typed.

#### 2. Structured machine-readable handoffs
Worth reinforcing in quantbot:
- non-deterministic modules should emit structured JSON / typed summaries
- downstream modules should consume contracts, not prose

Concrete quantbot examples:
- `ForecastSummary`
- `OverlayAction`
- `RiskDecision`
- `OrderRequest`

#### 3. Logging / operator ergonomics
Framework-style repos often do a good job packaging:
- continuous operation
- status surfaces
- logging across stages

This is relevant to quantbot's daemon/service layer, though the repo already has a safer and more replayable ops foundation.

### What to Ignore

#### 1. Autonomous thesis generation as the trading driver
Not aligned with quantbot.

Reason:
- hard to replay
- hard to evaluate OOS
- easy to create "ghost behavior"
- encourages unbounded strategy drift

In quantbot, adaptive intelligence should remain a **bounded modifier**, not the primary source of trade intent.

#### 2. Solana / wallet-key / crypto-native execution stack
Not useful for the current system.

Quantbot currently lives closer to:
- daily bars
- SPY / GLD / GC=F / FX
- IG / paper-style execution
- deterministic research loops

So code reuse from AutoHedge would be low-value and high-friction.

#### 3. Fully autonomous always-on Python agent stack with live keys
Too much blast radius for the current design goals.

Quantbot should prefer:
- deterministic trading loop
- explicit action constraints
- strong auditability
- isolated execution permissions

### Practical Rule
If borrowing from AutoHedge, borrow **contracts and packaging**, not **autonomy**.

Good borrow:
- "every stage has a typed output"
- "risk is a veto gate"
- "execution is isolated from strategy generation"

Bad borrow:
- "let multiple LLM agents continuously improvise the portfolio"

### Decision
Do **not** add AutoHedge as an integration target or core dependency.

Keep it in the notes bucket as:
- agent orchestration reference
- JSON schema / handoff reference
- productization / ops inspiration

---

## 10. FinGPT Review — Useful for News Overlay Research, Not Core Integration

**Reference:** [AI4Finance-Foundation/FinGPT](https://github.com/AI4Finance-Foundation/FinGPT)

### Snapshot
FinGPT is most relevant to quantbot as a **financial NLP / news research toolkit**, not as a core trading engine. The strongest overlap is in:
- financial text processing
- sentiment / event classification
- finance-specific prompt and label design
- data pipeline ideas for point-in-time text tasks

### Bottom Line
Potentially useful for a **bounded news overlay**, but not something to integrate wholesale into the core system.

Quantbot's current architecture is still the right discipline:
- deterministic core
- bounded overlays
- explicit risk vetoes
- replayable evaluation via caching

FinGPT should only enter through that same gate.

### What to Borrow

#### 1. Finance-specific NLP task design
Use FinGPT as inspiration for narrow text tasks such as:
- sentiment classification
- event severity scoring
- asset-class scope detection
- macro / instrument risk tagging

These tasks fit naturally into quantbot's overlay layer.

#### 2. Data pipeline ideas
The repo is useful for:
- financial news ingestion patterns
- text normalization / preprocessing ideas
- finance-specific labeling strategies
- benchmark task framing

This is likely more useful than adopting the full model stack.

#### 3. Structured outputs for text models
Good lesson for quantbot:
- make text models emit structured classifications
- avoid prose-first decision flows

Target shape should remain narrow and machine-readable, e.g.:
- sentiment
- severity
- scope
- confidence
- action hint

### What to Ignore

#### 1. Whole-framework adoption
Avoid integrating FinGPT as a large dependency bundle or "AI trading system."

Reason:
- too much Python/ML surface area
- weakens evaluation clarity
- adds complexity before proving value

#### 2. Text-to-portfolio autonomy
Avoid any design where text models directly:
- set portfolio direction
- size positions
- drive execution

For quantbot, text should influence **risk overlays**, not become the primary source of trade intent.

#### 3. Any non-point-in-time text pipeline
Strict requirement:
- all text inputs must be point-in-time aligned
- cached outputs must be replayable

Otherwise results are likely contaminated by look-ahead and article revision effects.

### Best Quantbot Use Case

#### Bounded News Overlay
This is the strongest fit.

Desired role for FinGPT-like models:
- classify recent headlines/news as risk-on / neutral / risk-off
- determine affected scope (global / asset class / instrument)
- estimate severity/confidence
- suggest only bounded actions

Allowed outputs should map to existing overlay actions such as:
- `ScaleExposure`
- `FreezeEntries`
- `DisableInstrument`

### Recommended Output Contract
If using a FinGPT-style model, force it into a tight structured contract:

```json
{
  "sentiment": "negative",
  "severity": 0.82,
  "scope": {
    "asset_class": "equity"
  },
  "action_hint": "freeze_entries",
  "confidence": 0.76,
  "reason": "Tariff escalation increases near-term equity downside uncertainty"
}
```

Quantbot should still map this deterministically into a real overlay action.

### Research Plan

#### Phase 1 — Offline cache only
Build a historical text cache:
- raw headline/article metadata
- structured FinGPT output
- parse success/failure
- model version
- created_at

Same discipline as:
- LLM cache
- Kronos forecast cache

#### Phase 2 — Replay through overlay logic
Convert structured text outputs into a very small set of bounded actions:
- severe negative macro/equity news → `FreezeEntries(AssetClass(Equity), until=+1d/+3d)`
- moderate risk-off signal → `ScaleExposure(AssetClass(Equity), factor=0.5, until=+1d)`
- severe instrument event → `DisableInstrument(sym, until=+1d)`

#### Phase 3 — Compare against existing overlays
Run replay comparisons:
1. TSMOM only
2. TSMOM + volatility overlay
3. TSMOM + FinGPT news overlay
4. TSMOM + volatility + FinGPT
5. later: TSMOM + Kronos + FinGPT

Metrics:
- Sharpe
- max drawdown
- turnover
- number of overlay days
- tail-period behavior
- false-positive freeze/scale rate

### First Practical Experiment
Do **not** start with full article reasoning or direct trade decisions.

Start with:
- daily headline bundle classification
- one-day overlay horizon
- instruments / asset classes: SPY, GLD, GC=F
- actions limited to `ScaleExposure` and `FreezeEntries`

### Decision
FinGPT is more relevant to quantbot than AutoHedge, but still belongs in the **research-input** bucket, not the core architecture bucket.

Use it, if at all, as:
- a future bounded news overlay input
- a source of finance-specific NLP task design
- a data/prompt reference

Do **not** use it as:
- a trading brain
- an execution system
- a justification for weakening replayability or boundedness

---

## 11. NautilusTrader Review — Strong Architecture Reference, Not a Near-Term Migration

**Reference:** [nautechsystems/nautilus_trader](https://github.com/nautechsystems/nautilus_trader)

### Snapshot
NautilusTrader is much closer to quantbot's operating philosophy than agent-first repos:
- deterministic event-driven engine
- strong replay / simulation emphasis
- adapter-based integration model
- serious backtest/live parity mindset

It is therefore useful as an **architecture benchmark** and source of implementation patterns. It is **not** a good near-term migration target because the framework is large and the current quantbot bottlenecks are still signal quality, overlay calibration, and live validation rather than engine generality.

### Bottom Line
Borrow **engine patterns**, **event contracts**, and **adapter interfaces**.

Do **not** attempt to replace quantbot with NautilusTrader right now.

---

### What to Borrow

#### 1. Explicit event types inside quantbot
Instead of letting modules call each other too directly, push more state transitions through explicit events.

Concrete sketch:

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum BotEvent {
    MarketDataUpdated {
        instrument: String,
        eval_date: chrono::NaiveDate,
    },
    ForecastProduced {
        instrument: String,
        eval_date: chrono::NaiveDate,
        model_name: String,
        model_version: String,
    },
    OverlayProposed {
        eval_date: chrono::NaiveDate,
        source: String, // "volatility", "news", "kronos"
        action_json: String,
    },
    OverlayApplied {
        eval_date: chrono::NaiveDate,
        action_json: String,
    },
    RiskRejected {
        eval_date: chrono::NaiveDate,
        reason: String,
    },
    OrdersGenerated {
        eval_date: chrono::NaiveDate,
        count: usize,
    },
    ExecutionAck {
        instrument: String,
        status: String,
    },
}
```

Reason: this makes replay/live parity easier. The daemon, replay engine, and status command can all consume the same state transitions.

#### 2. Adapter traits for all "external intelligence"
Treat non-core systems as adapters, not as first-class strategy engines.

Concrete sketch:

```rust
pub trait ForecastAdapter {
    fn produce_summary(
        &self,
        instrument: &str,
        eval_date: chrono::NaiveDate,
    ) -> anyhow::Result<crate::forecast::ForecastSummary>;
}

pub trait TextRiskAdapter {
    fn classify_bundle(
        &self,
        scope: &str,
        eval_date: chrono::NaiveDate,
    ) -> anyhow::Result<TextRiskSummary>;
}
```

Then the concrete implementations can be:
- cached SQLite replay
- Python sidecar
- live HTTP service

without changing the trading engine.

#### 3. Unified replay/live decision path
One of the biggest Nautilus-style lessons: do not fork logic too early between replay and live.

Desired quantbot shape:

```rust
fn run_decision_cycle(ctx: &DecisionContext) -> anyhow::Result<DecisionResult> {
    let base_targets = compute_deterministic_targets(&ctx.bars, &ctx.config)?;
    let overlays = collect_overlays(ctx)?;
    let final_targets = apply_all_overlays(base_targets, overlays, ctx.eval_date);
    let risk_checked = run_risk_gate(final_targets, &ctx.risk)?;
    let orders = generate_orders(risk_checked, &ctx.positions, &ctx.router)?;
    Ok(DecisionResult { overlays, orders })
}
```

Then:
- replay mode builds `DecisionContext` from caches and historical state
- live mode builds `DecisionContext` from current bars and live positions

The **same function** should decide.

#### 4. Separate "strategy", "risk", and "execution planning"
Even when they are all local Rust modules, keep the boundaries hard.

Concrete sketch:

```rust
pub struct StrategyOutput {
    pub target_weights: std::collections::HashMap<String, f64>,
    pub metadata: serde_json::Value,
}

pub struct RiskOutput {
    pub approved_weights: std::collections::HashMap<String, f64>,
    pub veto_reason: Option<String>,
}

pub struct ExecutionPlan {
    pub orders: Vec<OrderRequest>,
    pub warnings: Vec<String>,
}
```

This is a healthier scaling pattern than letting one function do:
- signal generation
- overlay combination
- risk gate
- order planning
- execution side effects

#### 5. Normalize data/provider boundaries
NautilusTrader is strong on adapters. Quantbot should keep moving in that direction.

Concrete sketch:

```rust
pub trait BarProvider {
    fn load_history(
        &self,
        instrument: &str,
        start: Option<chrono::NaiveDate>,
        end: Option<chrono::NaiveDate>,
    ) -> anyhow::Result<crate::core::bar::BarSeries>;
}

pub trait ExecutionVenue {
    fn get_positions(&self) -> anyhow::Result<Vec<LivePosition>>;
    fn submit_orders(&self, orders: &[OrderRequest]) -> anyhow::Result<Vec<OrderReceipt>>;
}
```

Then Yahoo, CSV, IG, and any future broker all live behind explicit interfaces.

---

### What to Ignore

#### 1. Full framework migration now
Avoid:
- rewriting quantbot around NautilusTrader
- forcing current overlay research into a much larger abstraction layer
- solving infrastructure generality before proving signal value

Current priority remains:
- validate Kronos
- calibrate overlays
- test bounded news / text ideas
- harden live ops

#### 2. Premature complexity in venue/instrument modeling
NautilusTrader handles a broad universe of venues and instruments. Quantbot does not need all of that immediately.

Useful local rule:
- build the smallest interface that supports current needs
- do not generalize to dozens of venue types until actually needed

#### 3. Any refactor that breaks current replayability
A framework-inspired rewrite is only good if replay remains:
- deterministic
- cache-driven
- easy to inspect

If that gets worse, the refactor is a regression regardless of elegance.

---

### Best Concrete Lessons for Quantbot

#### A. Record event stream, not just final artifacts
Today quantbot records:
- signals
- orders
- overlay actions
- positions

Potential improvement:

```rust
pub struct EventRecorder {
    db: std::sync::Arc<std::sync::Mutex<Db>>,
}

impl EventRecorder {
    pub fn record(&self, run_id: &str, event: &BotEvent) {
        let json = serde_json::to_string(event).unwrap_or_default();
        // future: insert into bot_events(run_id, event_type, event_json, ts)
        eprintln!("EVENT {run_id} {json}");
    }
}
```

That would make debugging daemon/live discrepancies much easier.

#### B. Formalize overlay sources as pluggable producers
Current overlays are already moving in the right direction. Make the contract explicit:

```rust
pub trait OverlayProducer {
    fn name(&self) -> &'static str;
    fn produce(
        &self,
        ctx: &OverlayContext,
    ) -> anyhow::Result<Vec<crate::overlay::OverlayAction>>;
}
```

Examples:
- `VolatilityOverlayProducer`
- `NewsOverlayProducer`
- `KronosOverlayProducer`

This makes composition cleaner and more testable.

#### C. Make state transitions explicit in the daemon
Instead of "daemon runs a big loop", prefer explicit phases:

```rust
pub enum CyclePhase {
    RefreshData,
    BuildContext,
    ProduceSignals,
    ProduceOverlays,
    ApplyRisk,
    PlanOrders,
    Execute,
    Reconcile,
    Persist,
}
```

Even if implemented locally, this creates a more auditable ops story.

---

### Proposed Incremental Refactor Path (Borrowing the Good Parts)

#### Step 1 — add explicit internal event type
Low risk, high debugging value.

#### Step 2 — define `OverlayProducer` and `ForecastAdapter` traits
Lets Kronos/news/replay/live adapters share the same shape.

#### Step 3 — converge replay/live into a single decision-cycle function
Highest-value architecture cleanup once current feature work stabilizes.

#### Step 4 — optionally add event recording table
Only after the event taxonomy settles.

---

### Decision
NautilusTrader is one of the best external repos to study for **engine quality** and **replay/live parity**, but it should remain a **reference architecture**, not a dependency or migration target for now.

Best use:
- copy the patterns
- keep the current quantbot scope
- improve internal contracts incrementally

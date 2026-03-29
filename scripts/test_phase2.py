#!/usr/bin/env python3
"""End-to-end Phase 2 test: run all agents on one instrument."""

from __future__ import annotations

import json
import sys
from datetime import date

# Step 1: Fetch data
print("=" * 60)
print("  PHASE 2 END-TO-END TEST")
print("=" * 60)

instrument = "SPY"
print(f"\n[1/7] Fetching data for {instrument}...")
from quantbot.data.yahoo import YahooProvider

provider = YahooProvider()
bars = provider.fetch_bars(instrument, date(2022, 1, 1), date(2025, 1, 1))
print(f"  Got {len(bars)} bars ({bars.index[0].date()} → {bars.index[-1].date()})")

# Step 2: TSMOM agent (no LLM)
print(f"\n[2/7] Running TSMOM agent...")
from quantbot.agents.tsmom.agent import TSMOMAgent

tsmom = TSMOMAgent()
tsmom_signal = tsmom.generate_signal(bars, instrument)
print(f"  Direction: {tsmom_signal.direction.value}")
print(f"  Strength:  {tsmom_signal.strength:.3f}")
print(f"  Confidence: {tsmom_signal.confidence:.3f}")
ann_vol = tsmom_signal.metadata.get('ann_vol')
print(f"  Vol: {ann_vol:.3f}" if ann_vol else "  Vol: N/A (insufficient data)")

# Step 3: Indicator agent (text LLM)
print(f"\n[3/7] Running Indicator agent (qwen3:14b via Ollama)...")
from quantbot.agents.indicator.tools import compute_all_indicators

indicators = compute_all_indicators(bars)
print(f"  Indicators computed: RSI={indicators['rsi_14']}, MACD={indicators['macd_histogram']:.4f}")

from quantbot.agents.indicator.agent import make_indicator_node

indicator_node = make_indicator_node()
indicator_result = indicator_node({
    "instrument": instrument,
    "bars": bars,
    "signals": [],
    "debate": {},
    "decision": {},
    "memory_context": "",
    "metadata": {},
})
ind_signal = indicator_result["signals"][0]
print(f"  Direction: {ind_signal.direction.value}")
print(f"  Strength:  {ind_signal.strength:.3f}")
print(f"  Confidence: {ind_signal.confidence:.3f}")
print(f"  Reasoning: {ind_signal.metadata.get('reasoning', 'N/A')[:200]}")

# Step 4: Pattern agent (vision LLM)
print(f"\n[4/7] Running Pattern agent (qwen3-vl via Ollama)...")
from quantbot.agents.pattern.agent import make_pattern_node

pattern_node = make_pattern_node()
pattern_result = pattern_node({
    "instrument": instrument,
    "bars": bars,
    "signals": [],
    "debate": {},
    "decision": {},
    "memory_context": "",
    "metadata": {},
})
pat_signal = pattern_result["signals"][0]
print(f"  Direction: {pat_signal.direction.value}")
print(f"  Strength:  {pat_signal.strength:.3f}")
print(f"  Confidence: {pat_signal.confidence:.3f}")
print(f"  Patterns: {pat_signal.metadata.get('patterns_identified', 'N/A')}")

# Step 5: Signal Combiner
print(f"\n[5/7] Running Signal Combiner...")
from quantbot.agents.decision.combiner import SignalCombiner

combiner = SignalCombiner()
all_signals = [tsmom_signal, ind_signal, pat_signal]
combined = combiner.combine(all_signals)
print(f"  Combined direction: {combined.direction.value}")
print(f"  Combined strength:  {combined.strength:.3f}")
print(f"  Combined confidence: {combined.confidence:.3f}")
print(f"  Contributions: {combined.metadata.get('agent_contributions', {})}")

# Step 6: Memory store
print(f"\n[6/7] Testing SQLite memory store...")
from quantbot.memory.store import MemoryStore

store = MemoryStore(":memory:")  # in-memory for test
for sig in all_signals:
    store.log_signal(sig)
decision_id = store.log_decision(
    instrument=instrument,
    direction=combined.direction.value,
    strength=combined.strength,
    confidence=combined.confidence,
    signals=all_signals,
)
store.update_decision_outcome(decision_id, actual_return=0.005, outcome="win")
context = store.build_memory_context(instrument)
print(f"  Logged {len(all_signals)} signals + 1 decision")
print(f"  Memory context:\n    {context.replace(chr(10), chr(10) + '    ')}")

# Step 7: Summary
print(f"\n[7/7] Summary")
print("=" * 60)
print(f"  Instrument: {instrument}")
print(f"  TSMOM:      {tsmom_signal.direction.value:>5} (s={tsmom_signal.strength:+.2f}, c={tsmom_signal.confidence:.2f})")
print(f"  Indicator:  {ind_signal.direction.value:>5} (s={ind_signal.strength:+.2f}, c={ind_signal.confidence:.2f})")
print(f"  Pattern:    {pat_signal.direction.value:>5} (s={pat_signal.strength:+.2f}, c={pat_signal.confidence:.2f})")
print(f"  Combined:   {combined.direction.value:>5} (s={combined.strength:+.2f}, c={combined.confidence:.2f})")
print("=" * 60)
print("\nPhase 2 end-to-end test PASSED!")

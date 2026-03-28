"""Time-Series Momentum agent (Moskowitz, Ooi, Pedersen — JFE 2012).

Signal = average sign of trailing k-month returns for k in {1, 3, 6, 12}.
Position sized to target annualized volatility of 40%.
"""

from __future__ import annotations

import numpy as np
import pandas as pd

from quantbot.agents.base import QuantAgent
from quantbot.agents.tsmom.volatility import ewma_volatility
from quantbot.core.signal import Signal, SignalDirection, SignalType
from quantbot.data.bar import BarDataFrame

# Default lookback windows in trading days (approximate months)
DEFAULT_LOOKBACKS = (21, 63, 126, 252)  # ~1m, 3m, 6m, 12m
VOL_TARGET = 0.40  # 40% annualized target volatility
EWMA_COM = 60  # center of mass for vol estimation


class TSMOMAgent(QuantAgent):
    """Time-series momentum agent.

    For each lookback window, computes the sign of the trailing return.
    The signal strength is the average sign across all lookbacks.
    Confidence is based on agreement across lookbacks.
    """

    name = "TSMOM"

    def __init__(
        self,
        lookbacks: tuple[int, ...] = DEFAULT_LOOKBACKS,
        vol_target: float = VOL_TARGET,
        ewma_com: int = EWMA_COM,
    ) -> None:
        self.lookbacks = lookbacks
        self.vol_target = vol_target
        self.ewma_com = ewma_com

    def generate_signal(self, bars: BarDataFrame, instrument: str) -> Signal:
        """Generate a TSMOM signal for a single instrument.

        Args:
            bars: OHLCV DataFrame with DatetimeIndex (must have >= max(lookbacks)+1 rows).
            instrument: The instrument symbol.

        Returns:
            A Signal with direction, strength, and vol-scaled metadata.
        """
        close = bars["Close"]
        returns = close.pct_change().dropna()

        if len(returns) < max(self.lookbacks) + 1:
            return self._flat_signal(instrument, "insufficient data")

        # Trailing returns for each lookback
        signs = []
        trailing_rets = {}
        for lb in self.lookbacks:
            ret = close.iloc[-1] / close.iloc[-1 - lb] - 1.0
            trailing_rets[f"ret_{lb}d"] = ret
            signs.append(np.sign(ret))

        # Average sign across lookbacks → strength in [-1, 1]
        avg_sign = float(np.mean(signs))

        # Confidence = fraction of lookbacks that agree with the majority direction
        if avg_sign == 0:
            return self._flat_signal(instrument, "conflicting signals", trailing_rets)
        majority = np.sign(avg_sign)
        agreement = sum(1 for s in signs if s == majority) / len(signs)

        # Volatility estimate
        ann_vol = ewma_volatility(returns, com=self.ewma_com)
        current_vol = float(ann_vol.iloc[-1])

        if current_vol < 1e-8:
            return self._flat_signal(instrument, "zero volatility", trailing_rets)

        # Vol-scaled position: target / realized
        vol_scalar = self.vol_target / current_vol

        direction = SignalDirection.LONG if avg_sign > 0 else SignalDirection.SHORT
        strength = float(np.clip(avg_sign, -1.0, 1.0))

        return Signal(
            instrument=instrument,
            direction=direction,
            strength=strength,
            confidence=agreement,
            agent_name=self.name,
            signal_type=SignalType.QUANT,
            horizon_days=21,
            metadata={
                **trailing_rets,
                "ann_vol": current_vol,
                "vol_scalar": vol_scalar,
                "lookbacks": list(self.lookbacks),
            },
        )

    def compute_target_weight(self, signal: Signal) -> float:
        """Compute the vol-targeted portfolio weight for a signal.

        Returns the fraction of NAV to allocate (signed).
        """
        vol_scalar = signal.metadata.get("vol_scalar", 1.0)
        return signal.strength * signal.confidence * vol_scalar

    def _flat_signal(
        self,
        instrument: str,
        reason: str,
        metadata: dict | None = None,
    ) -> Signal:
        meta = metadata or {}
        meta["flat_reason"] = reason
        return Signal(
            instrument=instrument,
            direction=SignalDirection.FLAT,
            strength=0.0,
            confidence=0.0,
            agent_name=self.name,
            signal_type=SignalType.QUANT,
            horizon_days=21,
            metadata=meta,
        )

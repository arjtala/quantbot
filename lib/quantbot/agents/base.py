"""Base agent interface."""

from __future__ import annotations

from abc import ABC, abstractmethod

from quantbot.core.signal import Signal
from quantbot.data.bar import BarDataFrame


class QuantAgent(ABC):
    """Base class for deterministic quantitative agents."""

    name: str

    @abstractmethod
    def generate_signal(self, bars: BarDataFrame, instrument: str) -> Signal:
        """Produce a trading signal from bar data."""
        ...

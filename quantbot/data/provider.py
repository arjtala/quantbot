"""Base data provider interface."""

from __future__ import annotations

from abc import ABC, abstractmethod
from datetime import date

from quantbot.data.bar import BarDataFrame


class DataProvider(ABC):
    """Abstract interface for market data sources."""

    @abstractmethod
    def fetch_bars(
        self,
        instrument: str,
        start: date,
        end: date,
        interval: str = "1d",
    ) -> BarDataFrame:
        """Fetch OHLCV bars for the given instrument and date range."""
        ...

    @abstractmethod
    def fetch_latest(
        self,
        instrument: str,
        count: int = 252,
        interval: str = "1d",
    ) -> BarDataFrame:
        """Fetch the most recent `count` bars."""
        ...

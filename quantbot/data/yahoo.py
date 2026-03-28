"""Yahoo Finance data provider."""

from __future__ import annotations

from datetime import date, timedelta

import yfinance as yf

from quantbot.data.bar import BarDataFrame, validate_bars
from quantbot.data.provider import DataProvider
from quantbot.data.universe import get_instrument


class YahooProvider(DataProvider):
    """Wraps yfinance for OHLCV data."""

    def fetch_bars(
        self,
        instrument: str,
        start: date,
        end: date,
        interval: str = "1d",
    ) -> BarDataFrame:
        ticker = get_instrument(instrument).ticker
        df = yf.download(
            ticker,
            start=str(start),
            end=str(end),
            interval=interval,
            auto_adjust=True,
            progress=False,
        )
        # yfinance may return MultiIndex columns for single ticker
        if hasattr(df.columns, "levels") and len(df.columns.levels) > 1:
            df = df.droplevel(level=1, axis=1)
        return validate_bars(df)

    def fetch_latest(
        self,
        instrument: str,
        count: int = 252,
        interval: str = "1d",
    ) -> BarDataFrame:
        # Fetch extra to ensure we get enough bars after weekends/holidays
        buffer_days = int(count * 1.6) + 30
        end = date.today() + timedelta(days=1)
        start = end - timedelta(days=buffer_days)
        bars = self.fetch_bars(instrument, start, end, interval)
        return bars.iloc[-count:]

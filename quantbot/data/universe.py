"""Instrument definitions and predefined universes."""

from __future__ import annotations

from dataclasses import dataclass
from enum import Enum


class AssetClass(Enum):
    CRYPTO = "CRYPTO"
    EQUITY = "EQUITY"
    FUTURES = "FUTURES"
    FX = "FX"


class DataSource(Enum):
    YAHOO = "YAHOO"
    CCXT = "CCXT"


@dataclass(frozen=True)
class Instrument:
    symbol: str  # canonical identifier
    name: str
    asset_class: AssetClass
    data_source: DataSource = DataSource.YAHOO
    yahoo_ticker: str | None = None  # override if different from symbol
    point_value: float = 1.0  # contract multiplier for futures

    @property
    def ticker(self) -> str:
        return self.yahoo_ticker or self.symbol


# ---------------------------------------------------------------------------
# Predefined universes
# ---------------------------------------------------------------------------

CRYPTO_UNIVERSE = [
    Instrument("BTC-USD", "Bitcoin", AssetClass.CRYPTO),
    Instrument("ETH-USD", "Ethereum", AssetClass.CRYPTO),
    Instrument("SOL-USD", "Solana", AssetClass.CRYPTO),
    Instrument("BNB-USD", "Binance Coin", AssetClass.CRYPTO),
]

EQUITY_UNIVERSE = [
    Instrument("SPY", "S&P 500 ETF", AssetClass.EQUITY),
    Instrument("QQQ", "Nasdaq 100 ETF", AssetClass.EQUITY),
    Instrument("IWM", "Russell 2000 ETF", AssetClass.EQUITY),
    Instrument("EFA", "EAFE ETF", AssetClass.EQUITY),
    Instrument("EEM", "Emerging Markets ETF", AssetClass.EQUITY),
    Instrument("TLT", "20+ Year Treasury ETF", AssetClass.EQUITY),
    Instrument("GLD", "Gold ETF", AssetClass.EQUITY),
]

FUTURES_UNIVERSE = [
    Instrument("ES=F", "E-mini S&P 500", AssetClass.FUTURES, point_value=50.0),
    Instrument("NQ=F", "E-mini Nasdaq 100", AssetClass.FUTURES, point_value=20.0),
    Instrument("GC=F", "Gold Futures", AssetClass.FUTURES, point_value=100.0),
    Instrument("CL=F", "Crude Oil Futures", AssetClass.FUTURES, point_value=1000.0),
    Instrument("ZB=F", "30-Year T-Bond", AssetClass.FUTURES, point_value=1000.0),
]

ALL_INSTRUMENTS = CRYPTO_UNIVERSE + EQUITY_UNIVERSE + FUTURES_UNIVERSE

INSTRUMENT_MAP: dict[str, Instrument] = {inst.symbol: inst for inst in ALL_INSTRUMENTS}


def get_instrument(symbol: str) -> Instrument:
    """Look up an instrument by symbol, or create a default one."""
    if symbol in INSTRUMENT_MAP:
        return INSTRUMENT_MAP[symbol]
    # Fallback: treat as equity with point_value=1
    return Instrument(symbol, symbol, AssetClass.EQUITY)

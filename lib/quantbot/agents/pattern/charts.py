"""Chart generation for the Pattern Agent."""

from __future__ import annotations

import io

import pandas as pd

from quantbot.agents.shared.chart_renderer import render_candlestick


def render_pattern_chart(
    bars: pd.DataFrame,
    instrument: str,
    last_n: int = 60,
) -> io.BytesIO:
    """Render a clean candlestick chart for pattern recognition.

    Includes volume bars but no overlays — the vision LLM should
    identify patterns from raw price action.
    """
    # Add simple moving averages as subtle context
    close = bars["Close"]
    overlays = {
        "SMA20": close.rolling(20).mean(),
        "SMA50": close.rolling(50).mean(),
    }

    return render_candlestick(
        bars=bars,
        title=f"{instrument} — Pattern Analysis",
        overlays=overlays,
        show_volume=True,
        last_n=last_n,
    )

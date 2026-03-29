"""Shared chart rendering for vision-based agents.

Generates candlestick chart images as BytesIO for LLM vision input.
Key fix over QuantAgent: always save to BytesIO BEFORE plt.close().
"""

from __future__ import annotations

import io
from typing import Any

import matplotlib
import matplotlib.pyplot as plt
import numpy as np
import pandas as pd

# Use non-interactive backend for server/headless environments
matplotlib.use("Agg")


def render_candlestick(
    bars: pd.DataFrame,
    title: str = "",
    overlays: dict[str, pd.Series] | None = None,
    support_levels: list[float] | None = None,
    resistance_levels: list[float] | None = None,
    trendlines: list[dict[str, Any]] | None = None,
    show_volume: bool = True,
    last_n: int = 60,
    figsize: tuple[int, int] = (14, 8),
) -> io.BytesIO:
    """Render a candlestick chart to BytesIO (PNG).

    Args:
        bars: OHLCV DataFrame with DatetimeIndex.
        title: Chart title.
        overlays: Named series to overlay (e.g., {"SMA20": sma_series}).
        support_levels: Horizontal support lines.
        resistance_levels: Horizontal resistance lines.
        trendlines: List of dicts with "start"/"end" (x,y) tuples.
        show_volume: Whether to show volume subplot.
        last_n: Number of most recent bars to display.
        figsize: Figure size.

    Returns:
        BytesIO containing the PNG image.
    """
    df = bars.iloc[-last_n:].copy()

    n_axes = 2 if show_volume else 1
    height_ratios = [3, 1] if show_volume else [1]
    fig, axes = plt.subplots(
        n_axes, 1, figsize=figsize, sharex=True,
        gridspec_kw={"height_ratios": height_ratios},
    )
    if n_axes == 1:
        axes = [axes]

    ax_price = axes[0]

    # Candlestick bars
    x = np.arange(len(df))
    opens = df["Open"].values
    closes = df["Close"].values
    highs = df["High"].values
    lows = df["Low"].values

    colors = ["#26a69a" if c >= o else "#ef5350" for o, c in zip(opens, closes)]

    # Wicks
    for i in range(len(df)):
        ax_price.plot([x[i], x[i]], [lows[i], highs[i]], color=colors[i], linewidth=0.8)

    # Bodies
    body_width = 0.6
    for i in range(len(df)):
        bottom = min(opens[i], closes[i])
        height = abs(closes[i] - opens[i])
        ax_price.bar(x[i], height, bottom=bottom, width=body_width, color=colors[i])

    # Overlays (moving averages, etc.)
    if overlays:
        for name, series in overlays.items():
            aligned = series.reindex(df.index).dropna()
            if len(aligned) > 0:
                overlay_x = [list(df.index).index(idx) for idx in aligned.index if idx in df.index]
                ax_price.plot(overlay_x, aligned.values, label=name, linewidth=1.2, alpha=0.8)
        ax_price.legend(loc="upper left", fontsize=8)

    # Support / resistance lines
    if support_levels:
        for level in support_levels:
            ax_price.axhline(y=level, color="#26a69a", linestyle="--", alpha=0.6, linewidth=1)

    if resistance_levels:
        for level in resistance_levels:
            ax_price.axhline(y=level, color="#ef5350", linestyle="--", alpha=0.6, linewidth=1)

    # Trendlines
    if trendlines:
        for tl in trendlines:
            start = tl.get("start", (0, 0))
            end = tl.get("end", (len(df) - 1, closes[-1]))
            ax_price.plot(
                [start[0], end[0]], [start[1], end[1]],
                color=tl.get("color", "#1976d2"),
                linestyle="-",
                linewidth=1.5,
                alpha=0.7,
            )

    # X-axis labels
    tick_spacing = max(1, len(df) // 10)
    tick_positions = x[::tick_spacing]
    tick_labels = [df.index[i].strftime("%m/%d") for i in range(0, len(df), tick_spacing)]
    ax_price.set_xticks(tick_positions)
    ax_price.set_xticklabels(tick_labels, rotation=45, fontsize=8)

    ax_price.set_ylabel("Price")
    ax_price.set_title(title or "Candlestick Chart")
    ax_price.grid(True, alpha=0.2)

    # Volume subplot
    if show_volume and len(axes) > 1:
        ax_vol = axes[1]
        ax_vol.bar(x, df["Volume"].values, width=body_width, color=colors, alpha=0.6)
        ax_vol.set_ylabel("Volume")
        ax_vol.grid(True, alpha=0.2)

    plt.tight_layout()

    # CRITICAL: Save to BytesIO BEFORE plt.close() (QuantAgent bug fix)
    buf = io.BytesIO()
    fig.savefig(buf, format="png", dpi=150, bbox_inches="tight")
    plt.close(fig)

    buf.seek(0)
    return buf

"""Application configuration via pydantic-settings + .env file."""

from __future__ import annotations

from pathlib import Path
from typing import Literal

from pydantic import Field
from pydantic_settings import BaseSettings, SettingsConfigDict

PROJECT_ROOT = Path(__file__).resolve().parent.parent
PROMPTS_DIR = PROJECT_ROOT / "quantbot" / "prompts"


class QuantbotSettings(BaseSettings):
    model_config = SettingsConfigDict(
        env_file=".env",
        env_file_encoding="utf-8",
        extra="ignore",
    )

    # --- LLM API Keys ---
    openai_api_key: str = ""
    anthropic_api_key: str = ""

    # --- Per-Agent Model Selection ---
    indicator_model: str = "gpt-4o"
    pattern_model: str = "gpt-4o"
    trend_model: str = "gpt-4o"
    debate_model: str = "claude-sonnet-4-20250514"
    decision_model: str = "claude-opus-4-20250514"

    # --- LLM Provider (for routing) ---
    default_provider: Literal["openai", "anthropic"] = "openai"

    # --- TSMOM Parameters ---
    tsmom_lookbacks: tuple[int, ...] = (21, 63, 126, 252)
    tsmom_vol_target: float = 0.40
    tsmom_ewma_com: int = 60

    # --- Signal Combiner Weights ---
    weight_tsmom: float = 0.50
    weight_indicator: float = 0.20
    weight_pattern: float = 0.15
    weight_trend: float = 0.15

    # --- Risk Limits ---
    max_position_pct: float = 0.20
    max_gross_leverage: float = 2.0
    max_single_trade_pct: float = 0.05

    # --- Instruments ---
    instruments: str = "BTC-USD,SPY,ES=F,GC=F"

    # --- Data ---
    data_cache_dir: str = "~/.quantbot/data"

    # --- Memory ---
    memory_db_path: str = "~/.quantbot/memory.db"

    # --- Debate ---
    debate_enabled: bool = True
    debate_max_rounds: int = 2

    # --- Backtest Cost Control ---
    backtest_model: str = "gpt-4o-mini"
    cache_llm_responses: bool = True

    @property
    def instrument_list(self) -> list[str]:
        return [s.strip() for s in self.instruments.split(",")]


def load_prompt(name: str) -> str:
    """Load a prompt template from the prompts/ directory.

    Args:
        name: Filename without extension (e.g., "indicator_system").

    Returns:
        The prompt text.
    """
    path = PROMPTS_DIR / f"{name}.md"
    if not path.exists():
        raise FileNotFoundError(f"Prompt template not found: {path}")
    return path.read_text(encoding="utf-8").strip()


# Singleton — import and use directly
settings = QuantbotSettings()

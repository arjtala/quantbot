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

    # --- LLM API Keys (optional if using Ollama/SGLang) ---
    openai_api_key: str = ""
    anthropic_api_key: str = ""

    # --- Ollama ---
    ollama_base_url: str = "http://localhost:11434"

    # --- SGLang / Custom OpenAI-compatible endpoint ---
    # For self-hosted models via SGLang, vLLM, TGI, etc.
    # Set this to your endpoint URL; uses OpenAI client with custom base_url.
    openai_base_url: str = ""  # e.g. "http://slurm-node:30000/v1"

    # --- Per-Agent Model Selection ---
    # Routing rules:
    #   "ollama:<model>"  → Ollama (local)
    #   "claude*"         → Anthropic API
    #   "gpt*"/"o1*"/"o3" → OpenAI API
    #   "sglang:<model>"  → Custom OpenAI-compatible endpoint (SGLang/vLLM)
    #   anything else     → Ollama (default)
    indicator_model: str = "qwen3:14b"
    pattern_model: str = "qwen3-vl"
    trend_model: str = "qwen3-vl"
    debate_model: str = "qwen3:14b"
    decision_model: str = "qwen3:14b"

    # --- LLM Provider (for routing) ---
    default_provider: Literal["openai", "anthropic", "ollama", "sglang"] = "ollama"

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
    backtest_model: str = "qwen3:14b"
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

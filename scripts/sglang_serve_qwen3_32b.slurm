#!/bin/bash
#SBATCH --job-name=quantbot-qwen3-32b
#SBATCH --account=dream
#SBATCH --qos=h200_comm_shared
#SBATCH --gres=gpu:h200:1       # 32B dense model — fits on 1 H200 (140GB)
#SBATCH --cpus-per-task=16
#SBATCH --mem=64G
#SBATCH --time=24:00:00
#SBATCH --output=logs/sglang-qwen3-32b-%j.out
#SBATCH --error=logs/sglang-qwen3-32b-%j.err

# ============================================================================
# QuantBot — Qwen3-32B Model Server
#
# Strong general reasoning model. Direct comparison with DeepSeek-R1-Distill-
# Qwen-32B (same size, different training). Qwen3 uses /think and /no_think
# mode — enable thinking for best reasoning quality.
# ============================================================================

MODEL="Qwen/Qwen3-32B"
PORT=30000
TP=1

# --- Activate environment ---
eval "$(conda shell.bash hook 2>/dev/null)"
conda activate quantbot-cluster

# --- Print connection info ---
echo "============================================"
echo "  SGLang Model Server"
echo "  Model:  $MODEL"
echo "  Node:   $(hostname)"
echo "  Port:   $PORT"
echo "  URL:    http://$(hostname):$PORT/v1"
echo "============================================"
echo ""
echo "To use from quantbot, set in .env:"
echo "  OPENAI_BASE_URL=http://$(hostname):$PORT/v1"
echo "  INDICATOR_MODEL=sglang:$MODEL"
echo ""

# --- Launch SGLang server ---
python -m sglang.launch_server \
    --model-path "$MODEL" \
    --host 0.0.0.0 \
    --port "$PORT" \
    --tp "$TP" \
    --trust-remote-code \
    --log-level info

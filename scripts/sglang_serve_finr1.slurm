#!/bin/bash
#SBATCH --job-name=quantbot-finr1
#SBATCH --account=dream
#SBATCH --qos=h200_comm_shared
#SBATCH --gres=gpu:h200:1       # 7B model — 1 H200 is plenty
#SBATCH --cpus-per-task=16
#SBATCH --mem=32G               # 7B needs ~14GB GPU RAM, minimal system RAM
#SBATCH --time=24:00:00
#SBATCH --output=logs/sglang-finr1-%j.out
#SBATCH --error=logs/sglang-finr1-%j.err

# ============================================================================
# QuantBot — Fin-R1 (7B) Model Server
#
# Finance-specialized reasoning model (DeepSeek-R1 RL-trained on financial data).
# Matches GPT-4 on financial reasoning benchmarks at 7B parameters.
# Uses <think> tags like DeepSeek-R1-Distill.
# ============================================================================

MODEL="TheFinAI/Fin-R1-7B"
PORT=30000
TP=1

# --- Activate environment ---
source activate quantbot-cluster || conda activate quantbot-cluster

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

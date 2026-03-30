#!/bin/bash
#SBATCH --job-name=quantbot-llm
#SBATCH --account=dream
#SBATCH --qos=h200_comm_shared
#SBATCH --gres=gpu:h200:1       # 1 H200 for 32B models
#SBATCH --cpus-per-task=16
#SBATCH --mem=64G
#SBATCH --time=24:00:00
#SBATCH --output=logs/sglang-server-%j.out
#SBATCH --error=logs/sglang-server-%j.err

# ============================================================================
# QuantBot — SGLang Model Server
#
# Serves an LLM via OpenAI-compatible API for the quantbot eval pipeline.
#
# Recommended models (adjust MODEL below):
#   1 H200:  deepseek-ai/DeepSeek-R1-Distill-Qwen-32B  (best reasoning/GPU)
#   1 H200:  Qwen/Qwen3-32B                              (strong reasoning)
#   1 H200:  SUFE-AIFLM-Lab/Fin-R1                           (finance-specialized)
#   8 H200:  Qwen/Qwen3-235B-A22B  (MoE, --tp 8, needs 1TB RAM)
#   4 H200:  deepseek-ai/DeepSeek-V3.2  (--tp 4)
#   8 H200:  deepseek-ai/DeepSeek-R1  (--tp 8)
# ============================================================================

# --- Configuration ---
MODEL="deepseek-ai/DeepSeek-R1-Distill-Qwen-32B"
# MODEL="Qwen/Qwen3-235B-A22B"
PORT=30000
TP=1  # tensor parallelism — set to number of GPUs

# --- Activate environment ---
eval "$(conda shell.bash hook 2>/dev/null)"
conda activate quantbot-cluster

# --- Print connection info ---
echo "============================================"
echo "  SGLang Model Server"
echo "  Model:  $MODEL"
echo "  Node:   $(hostname)"
echo "  Port:   $PORT"
echo "  TP:     $TP"
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

#!/bin/bash
#SBATCH --job-name=quantbot-llm
#SBATCH --account=dream
#SBATCH --qos=h200_comm_shared
#SBATCH --gres=gpu:h200:8       # 8 H200 for 235B MoE
#SBATCH --cpus-per-task=16
#SBATCH --mem=1024G
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
#   1 H200:  TheFinAI/Fin-R1-7B                           (finance-specialized)
#   8 H200:  Qwen/Qwen3-235B-A22B  (MoE, --tp 8, needs 1TB RAM)
#   4 H200:  deepseek-ai/DeepSeek-V3.2  (--tp 4)
#   8 H200:  deepseek-ai/DeepSeek-R1  (--tp 8)
# ============================================================================

# --- Configuration ---
# MODEL="deepseek-ai/DeepSeek-R1-Distill-Qwen-32B"
MODEL="Qwen/Qwen3-235B-A22B"
PORT=30000
TP=8  # tensor parallelism — set to number of GPUs

# --- Activate environment ---
source activate quantbot-cluster || conda activate quantbot-cluster

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

# --- Clear flashinfer JIT cache (workaround for compilation failures) ---
FLASHINFER_CACHE="$HOME/.cache/flashinfer"
echo "Clearing flashinfer JIT cache: $FLASHINFER_CACHE"
rm -rf "$FLASHINFER_CACHE"

# --- Launch SGLang server ---
python -m sglang.launch_server \
    --model-path "$MODEL" \
    --host 0.0.0.0 \
    --port "$PORT" \
    --tp "$TP" \
    --mem-fraction-static 0.80 \
    --disable-cuda-graph \
    --disable-custom-all-reduce \
    --attention-backend triton \
    --sampling-backend pytorch \
    --trust-remote-code \
    --log-level info

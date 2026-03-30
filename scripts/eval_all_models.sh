#!/bin/bash
# QuantBot — Launch 3 model evals in parallel on 3 H200 GPUs
#
# Submits 3 SGLang server jobs (one per model, each on its own H200),
# waits for all servers to be ready, then runs evals in parallel.
#
# Usage:
#   bash scripts/eval_all_models.sh
#
# Results land in:
#   eval_results/deepseek-ai_DeepSeek-R1-Distill-Qwen-32B/
#   eval_results/TheFinAI_Fin-R1-7B/
#   eval_results/Qwen_Qwen3-32B/

set -e

WORKERS=4
DAYS=60
DATA_DIR="data"

# Each model gets a unique port to avoid collisions if co-located
declare -A MODELS
MODELS[deepseek]="deepseek-ai/DeepSeek-R1-Distill-Qwen-32B"
MODELS[finr1]="TheFinAI/Fin-R1-7B"
MODELS[qwen3]="Qwen/Qwen3-32B"

declare -A PORTS
PORTS[deepseek]=30000
PORTS[finr1]=30001
PORTS[qwen3]=30002

declare -A MEM
MEM[deepseek]=64G
MEM[finr1]=32G
MEM[qwen3]=64G

LOG_DIR="logs"
mkdir -p "$LOG_DIR"

echo "========================================================================"
echo "  MULTI-MODEL EVAL — 3 models × 21 instruments × ${DAYS} days"
echo "========================================================================"
echo ""

# --- Step 1: Submit 3 SGLang server jobs ---
echo "Submitting SGLang server jobs..."
declare -A JOBIDS
declare -A SLURM_LOGS

for key in deepseek finr1 qwen3; do
    MODEL="${MODELS[$key]}"
    PORT="${PORTS[$key]}"
    MEMREQ="${MEM[$key]}"

    JOBID=$(sbatch --parsable \
        --job-name="qb-${key}" \
        --account=dream \
        --qos=h200_comm_shared \
        --gres=gpu:h200:1 \
        --cpus-per-task=16 \
        --mem="$MEMREQ" \
        --time=24:00:00 \
        --output="${LOG_DIR}/sglang-${key}-%j.out" \
        --error="${LOG_DIR}/sglang-${key}-%j.err" \
        --wrap="#!/bin/bash
eval \"\$(conda shell.bash hook 2>/dev/null)\"
conda activate quantbot-cluster
echo \"NODE=\$(hostname) PORT=${PORT} MODEL=${MODEL}\"
python -m sglang.launch_server \
    --model-path '${MODEL}' \
    --host 0.0.0.0 \
    --port ${PORT} \
    --tp 1 \
    --trust-remote-code \
    --log-level info
")

    JOBIDS[$key]=$JOBID
    echo "  ${key}: job ${JOBID} (${MODEL} on port ${PORT})"
done

echo ""

# --- Step 2: Wait for all jobs to start and get node assignments ---
echo "Waiting for jobs to start..."
declare -A NODES

for key in deepseek finr1 qwen3; do
    JOBID="${JOBIDS[$key]}"
    echo -n "  ${key} (job ${JOBID}): "

    while true; do
        STATE=$(squeue -j "$JOBID" -h -o "%T" 2>/dev/null || echo "UNKNOWN")
        if [ "$STATE" = "RUNNING" ]; then
            NODE=$(squeue -j "$JOBID" -h -o "%N" 2>/dev/null)
            NODES[$key]=$NODE
            echo "RUNNING on ${NODE}"
            break
        elif [ "$STATE" = "PENDING" ]; then
            echo -n "."
            sleep 10
        else
            echo "FAILED (state: ${STATE})"
            echo "Check: sacct -j ${JOBID} --format=State,ExitCode"
            exit 1
        fi
    done
done

echo ""

# --- Step 3: Wait for SGLang servers to be ready ---
echo "Waiting for SGLang servers to load models..."

for key in deepseek finr1 qwen3; do
    NODE="${NODES[$key]}"
    PORT="${PORTS[$key]}"
    echo -n "  ${key} (${NODE}:${PORT}): "

    for i in $(seq 1 120); do  # up to 20 min
        if curl -s --noproxy "$NODE" --max-time 3 "http://${NODE}:${PORT}/v1/models" >/dev/null 2>&1; then
            echo "READY"
            break
        fi
        if [ "$i" -eq 120 ]; then
            echo "TIMEOUT — server didn't respond after 20 min"
            echo "Check: cat ${LOG_DIR}/sglang-${key}-${JOBIDS[$key]}.out"
            exit 1
        fi
        echo -n "."
        sleep 10
    done
done

echo ""
echo "All 3 servers ready. Starting evals..."
echo ""

# --- Step 4: Run 3 evals in parallel ---
PIDS=()

for key in deepseek finr1 qwen3; do
    MODEL="${MODELS[$key]}"
    NODE="${NODES[$key]}"
    PORT="${PORTS[$key]}"

    (
        export no_proxy="$NODE"
        export NO_PROXY="$NODE"
        export OPENAI_BASE_URL="http://${NODE}:${PORT}/v1"
        export OPENAI_API_KEY="not-needed"
        export DEFAULT_PROVIDER="sglang"
        export INDICATOR_MODEL="sglang:${MODEL}"

        echo "[${key}] Starting eval: ${MODEL} via ${NODE}:${PORT}"
        python scripts/eval_round1.py \
            --days "$DAYS" \
            --data-dir "$DATA_DIR" \
            --workers "$WORKERS" \
            --run-name "${key}" \
            2>&1 | sed "s/^/[${key}] /"
        echo "[${key}] DONE"
    ) &
    PIDS+=($!)
done

# Wait for all evals to finish
FAILURES=0
for i in "${!PIDS[@]}"; do
    wait "${PIDS[$i]}" || ((FAILURES++))
done

echo ""
echo "========================================================================"
echo "  ALL EVALS COMPLETE"
echo "========================================================================"
echo ""

if [ "$FAILURES" -gt 0 ]; then
    echo "  WARNING: ${FAILURES} eval(s) failed — check output above"
fi

# --- Step 5: Print comparison summary ---
echo "  Results:"
for key in deepseek finr1 qwen3; do
    echo "    eval_results/${key}/"
done

echo ""
echo "  Compare with:"
echo "    paste <(head -1 eval_results/deepseek/round1_SPY.csv) <(head -1 eval_results/finr1/round1_SPY.csv)"
echo "    # or load into pandas for full comparison"

# --- Step 6: Cancel server jobs ---
echo ""
echo "  Cleaning up server jobs..."
for key in deepseek finr1 qwen3; do
    scancel "${JOBIDS[$key]}" 2>/dev/null && echo "    Cancelled ${key} (job ${JOBIDS[$key]})" || true
done

echo ""
echo "Done."

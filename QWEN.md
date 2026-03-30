# Running Qwen3-235B-A22B on SGLang (H200 Cluster)

## Model Specs
- **Model:** Qwen/Qwen3-235B-A22B (MoE, ~22B active parameters, 235B total)
- **Weights:** ~440GB in bf16 (118 safetensor shards × 3.8GB each)
- **Context:** 40,960 tokens

## What Works
- SGLang serves the model and responds to `/v1/models` endpoint
- Server starts up, loads weights, allocates KV cache
- Triton attention backend works (no JIT compilation needed)

## What Doesn't Work
- **flashinfer JIT compilation fails** on every inference request
- The MoE allreduce fusion kernel (`trtllm_moe_allreduce_fusion.cu`) fails to compile against the cluster's CUDA toolkit
- 90 compilation errors — `MoeFinalizeAllReduceFusionParams` struct members don't match the installed flashinfer headers
- This is a **flashinfer 0.6.3** incompatibility with the cluster's CUDA/compiler version
- The crash happens lazily on first inference, not at startup — so the server appears healthy but dies on first real request

## Resource Requirements (Confirmed)

| Resource | Minimum | Notes |
|---|---|---|
| GPUs | 8× H200 (140GB each) | 4× H200 OOM during weight loading |
| System RAM | 1TB+ | 484GB RSS measured at runtime; 512GB too tight (SLURM OOM kill) |
| `--tp` | 8 | Tensor parallelism must match GPU count |

## SLURM Config

```bash
#SBATCH --account=dream
#SBATCH --qos=h200_comm_shared     # partition inferred from QOS prefix
#SBATCH --gres=gpu:h200:8
#SBATCH --cpus-per-task=16
#SBATCH --mem=1024G
#SBATCH --time=24:00:00
```

## SGLang Flags Tried

| Flag | Purpose | Result |
|---|---|---|
| `--host 0.0.0.0` | Allow remote connections | **Required** — default `127.0.0.1` blocks external access |
| `--attention-backend triton` | Bypass flashinfer attention | Works for attention |
| `--sampling-backend pytorch` | Bypass flashinfer sampling | Works for sampling |
| `--disable-cuda-graph` | Skip CUDA graph capture | Avoids graph-related crashes |
| `--disable-custom-all-reduce` | Skip flashinfer allreduce | Doesn't prevent MoE kernel |
| `--disable-shared-experts-fusion` | Skip shared expert fusion | Doesn't prevent MoE kernel |
| `--mem-fraction-static 0.80` | Leave GPU headroom | Leaves ~27GB per GPU for activations |
| `--moe-runner-backend triton` | Bypass flashinfer MoE kernels | **Doesn't prevent MoE allreduce kernel** |

## The Core Problem

SGLang's MoE execution path triggers flashinfer's `trtllm_moe_allreduce_fusion` JIT compilation regardless of **all** backend override flags. This kernel is compiled lazily on first inference request — the server starts up and appears healthy, but crashes on the first real query. Every flag combination we tried still triggers the same flashinfer CUDA compilation failure.

The `trtllm_moe_allreduce_fusion.cu` kernel is incompatible with flashinfer 0.6.3 on this cluster's CUDA toolkit — 90 compilation errors due to struct member mismatches in `MoeFinalizeAllReduceFusionParams`.

## Recommended Next Steps

1. **Fix flashinfer** — upgrade/downgrade to a version compatible with the cluster CUDA toolkit, or ask cluster admins
2. **Use vLLM instead** — `vllm serve Qwen/Qwen3-235B-A22B --tensor-parallel-size 8` — different MoE backend, no flashinfer dependency
3. **Use DeepSeek-R1-Distill-Qwen-32B** — dense 32B model (not MoE), 1 H200, avoids the entire problem. #1 priority model for eval anyway.
4. **Pre-compile flashinfer** — compile the kernels on a dev node with the correct CUDA toolkit, copy the cached artifacts
5. **Use a quantized 235B** — FP8/AWQ halves requirements and may use a different code path

## Other Cluster Notes

- **Proxy:** Cluster HTTP proxy blocks outbound HTTPS (Yahoo Finance, HuggingFace via browser). HuggingFace model downloads work (different transport). yfinance does not. Use `scripts/download_data.py` locally and `--data-dir data/` on cluster.
- **`NO_PROXY`:** Must set `NO_PROXY=<compute-node>` when hitting the SGLang server from the login node.
- **HF cache:** Weights download to `~/.cache/huggingface/hub/models--Qwen--Qwen3-235B-A22B/` (~440GB). Not shared across nodes — each new node re-downloads.
- **flashinfer cache:** JIT artifacts at `~/.cache/flashinfer/`. Clearing it doesn't help (same compilation errors on rebuild).

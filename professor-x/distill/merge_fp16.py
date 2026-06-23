#!/usr/bin/env python3
"""Merge the trained LoRA adapter into the fp16 base ON CPU, fully offline.

Path of least resistance after the dead ends:
  · Ollama can't serve LoRA adapters → need a merged model.
  · merged_16bit / 4-bit-dequant kept bitsandbytes → llama.cpp can't convert.
  · 12GB GPU can't hold fp16 8B → merge on CPU.
Needs the fp16 base cached first (snapshot_download Qwen/Qwen3-8B). Produces clean fp16 safetensors
+ the official Qwen3 chat template → llama.cpp convert → q4_K_M → Ollama.
"""
import os, torch
os.environ["HF_HUB_OFFLINE"] = "1"
os.environ["TRANSFORMERS_OFFLINE"] = "1"
from transformers import AutoModelForCausalLM, AutoTokenizer
from peft import PeftModel

HERE = os.path.dirname(os.path.abspath(__file__))
ADAPTER = os.path.join(HERE, "out", "adapter")
OUT = os.path.join(HERE, "out", "gguf")
BASE = os.environ.get("PX_FP16_BASE", "Qwen/Qwen3-8B")

print(f"loading fp16 base {BASE} on CPU…", flush=True)
base = AutoModelForCausalLM.from_pretrained(BASE, torch_dtype=torch.bfloat16,
                                            device_map="cpu", low_cpu_mem_usage=True)
print("attaching adapter + merge_and_unload (CPU)…", flush=True)
model = PeftModel.from_pretrained(base, ADAPTER, device_map="cpu")
model = model.merge_and_unload()
# Official Qwen3 tokenizer carries the correct chat template (no unsloth needed).
tok = AutoTokenizer.from_pretrained(BASE)
os.makedirs(OUT, exist_ok=True)
print("saving merged fp16 →", OUT, flush=True)
model.save_pretrained(OUT, safe_serialization=True)
tok.save_pretrained(OUT)
print("MERGE_FP16_DONE", flush=True)

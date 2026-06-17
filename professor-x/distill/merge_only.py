#!/usr/bin/env python3
"""Recover a hung/incomplete run WITHOUT retraining: merge the already-trained LoRA adapter
(distill/out/adapter) into a 16-bit model at distill/out/gguf, fully OFFLINE.

Why: Unsloth's in-pipeline 16-bit merge re-fetches base files from HF and can hang on a dropped
connection (CLOSE-WAIT). The adapter is already on disk and the 4-bit base is cached, so we force
HF_HUB_OFFLINE and dequant-merge from cache — no network, no hang. Then run the rest of the pipeline
with SKIP_TRAIN (convert -> quantize -> serve -> stop-sanity).
"""
import os, sys
os.environ["HF_HUB_OFFLINE"] = "1"
os.environ["TRANSFORMERS_OFFLINE"] = "1"
import unsloth  # noqa: F401  (must precede transformers)
from unsloth import FastLanguageModel
from unsloth.chat_templates import get_chat_template

HERE = os.path.dirname(os.path.abspath(__file__))
ADAPTER = os.path.join(HERE, "out", "adapter")
OUT = os.path.join(HERE, "out", "gguf")
TEMPLATE = os.environ.get("PX_CHAT_TEMPLATE", "qwen3")

if not os.path.exists(os.path.join(ADAPTER, "adapter_model.safetensors")):
    sys.exit(f"No adapter at {ADAPTER}")

model, tok = FastLanguageModel.from_pretrained(
    model_name=ADAPTER,           # unsloth loads base (from adapter_config) + applies the adapter
    max_seq_length=int(os.environ.get("PX_MAX_SEQ", "8192")),
    load_in_4bit=True,
)
# Re-attach the correct chat template so the merged tokenizer (and the GGUF) carry it.
tok = get_chat_template(tok, chat_template=TEMPLATE)
model.save_pretrained_merged(OUT, tok, save_method="merged_16bit")
print("MERGE_DONE ->", OUT)

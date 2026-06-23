#!/usr/bin/env python3
"""QLoRA fine-tune qwen3:8b on the curated harness trajectories (Lever 1).

The harness scaffolds the 8B to perform well; its verified trajectories are the
model's own good outputs. Fine-tuning on them internalizes the harness's
disposition into the weights (Vygotsky: the learner internalizes the scaffold).
Fits the RTX 3060's ~5GB VRAM headroom via 4-bit + LoRA.

Setup (one-time):
    pip install unsloth "trl<0.10" peft bitsandbytes accelerate

Run:
    python3 distill/train_qlora.py            # trains on distill/data/curated.jsonl
    # → outputs distill/out/adapter/  and a GGUF for Ollama

Then serve (see distill/Modelfile.tmpl + distill/README.md):
    ollama create professor-x-distilled -f distill/Modelfile

IMPORTANT — identity gate (Conserved-Boundary theory): after training, the
ICS of the distilled model must stay >= 0.70 and pass@3 must beat the frozen
baseline by more than the MDE (~0.033). Else the adapter is an identity-death /
no-gain event and must be rejected. The harness measures this:
    professor-x --consciousness-report   # with the distilled model served
"""
import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
DATA = os.environ.get("PX_DATA", os.path.join(HERE, "data", "curated.jsonl"))
OUT = os.path.join(HERE, "out")

# Match the harness's primary model.
BASE_MODEL = os.environ.get("PX_BASE_MODEL", "unsloth/Qwen3-8B-unsloth-bnb-4bit")
MAX_SEQ = int(os.environ.get("PX_MAX_SEQ", "4096"))
EPOCHS = float(os.environ.get("PX_EPOCHS", "2"))
# Base is Qwen3, served via Ollama's Qwen3 template — train with the matching template.
# (Turn 1 used qwen-2.5 here; combined with full-sequence loss it produced a model that
# reasoned but never emitted the action format or stopped. See PLAN_11_10.md.)
CHAT_TEMPLATE = os.environ.get("PX_CHAT_TEMPLATE", "qwen3")
LR = float(os.environ.get("PX_LR", "2e-4"))
DEVICE_MAP = os.environ.get("PX_DEVICE_MAP", "sequential")
GPU_MEMORY_UTILIZATION = float(os.environ.get("PX_GPU_MEMORY_UTILIZATION", "0.5"))
OFFLOAD_EMBEDDING = os.environ.get("PX_OFFLOAD_EMBEDDING", "").lower() in {
    "1", "true", "yes", "on",
}
PER_DEVICE_BATCH_SIZE = int(os.environ.get("PX_BATCH_SIZE", "1"))
GRAD_ACCUM = int(os.environ.get("PX_GRAD_ACCUM", "8"))
WARMUP_STEPS = int(os.environ.get("PX_WARMUP_STEPS", "10"))
MAX_STEPS = int(os.environ.get("PX_MAX_STEPS", "-1"))
SAVE_STRATEGY = os.environ.get("PX_SAVE_STRATEGY", "epoch")
REPORT_TO = [s for s in os.environ.get("PX_REPORT_TO", "").split(",") if s]


def main():
    if not os.path.exists(DATA):
        sys.exit(f"No curated data at {DATA}. Run: python3 distill/curate.py")
    print(f"Training data: {DATA}")
    print(
        "Train config:"
        f" max_seq={MAX_SEQ}"
        f" epochs={EPOCHS}"
        f" lr={LR}"
        f" batch={PER_DEVICE_BATCH_SIZE}"
        f" grad_accum={GRAD_ACCUM}"
        f" device_map={DEVICE_MAP}"
        f" gpu_mem_util={GPU_MEMORY_UTILIZATION}"
        f" offload_embedding={OFFLOAD_EMBEDDING}"
    )

    try:
        from unsloth import FastLanguageModel, is_bfloat16_supported
        from unsloth.chat_templates import get_chat_template
        from datasets import load_dataset
        from transformers import TrainingArguments, Trainer, DataCollatorForSeq2Seq
    except ImportError as e:
        sys.exit(
            f"Missing deps ({e}).\n"
            "Install: pip install unsloth \"trl<0.10\" peft bitsandbytes accelerate datasets"
        )

    model, tokenizer = FastLanguageModel.from_pretrained(
        model_name=BASE_MODEL,
        max_seq_length=MAX_SEQ,
        load_in_4bit=True,
        device_map=DEVICE_MAP,
        gpu_memory_utilization=GPU_MEMORY_UTILIZATION,
        offload_embedding=OFFLOAD_EMBEDDING,
    )
    model = FastLanguageModel.get_peft_model(
        model,
        r=16,
        lora_alpha=32,            # alpha = 2r (Unsloth: more aggressive learning; last run underfit)
        lora_dropout=0.0,
        target_modules=["q_proj", "k_proj", "v_proj", "o_proj",
                        "gate_proj", "up_proj", "down_proj"],
        use_gradient_checkpointing="unsloth",
    )
    tokenizer = get_chat_template(tokenizer, chat_template=CHAT_TEMPLATE)

    ds = load_dataset("json", data_files=DATA, split="train")
    EOS_ID = tokenizer.eos_token_id

    # RAW ReAct format + ASSISTANT-ONLY masking. The bench drives the model via raw /api/generate
    # (SYSTEM_PROMPT, <task>, Thought/Action/Action Input turns, Observation between, stop=Observation:),
    # so we train in that exact raw text. CRITICAL FIX (last run regressed): compute loss ONLY on the
    # model's own assistant spans (Thought/Action/Action Input + their EOS). System, <task>, and
    # Observation (tool output) tokens are masked to -100 — the previous run trained on the whole
    # sequence incl. observations, teaching it to hallucinate tool output and diluting the action
    # signal (loss stalled at 1.64). Build token-level labels manually (TRL 0.9.6 has no qwen3
    # assistant_only_loss). See docs/research + DECISIONS D-010.
    def build(ex):
        ids, labels = [], []
        def add(text, train):
            t = tokenizer(text, add_special_tokens=False)["input_ids"]
            ids.extend(t); labels.extend(t if train else [-100] * len(t))
        for m in ex["messages"]:
            r, c = m["role"], m["content"]
            if r == "system":
                add(c + "\n\n", False)
            elif r == "user":
                add(f"<task>\n{c}\n</task>\n\n", False)
            elif r == "assistant":
                # FORMAT UNIFICATION (Phase 1): the bench prompt ends with the cue "Thought:"
                # (REACT_SUFFIX), so at inference the label is PROVIDED and the model must CONTINUE
                # with "<thought>\nAction:..\nAction Input:..". Mirror that here: emit "Thought:" as
                # MASKED context, train only on the continuation. Prevents the model re-emitting the
                # label ("Thought:Thought:") which made the parser fail -> 0/22. See format-unification plan.
                body = c[len("Thought:"):] if c.startswith("Thought:") else c
                add("Thought:", False)                 # cue provided at inference — masked
                add(body, True)                        # model learns ONLY the continuation
                ids.append(EOS_ID); labels.append(EOS_ID)
                add("\n\n", False)
            elif r == "tool":
                add(f"Observation: {c}\n\n", False)
        return {"input_ids": ids[:MAX_SEQ], "labels": labels[:MAX_SEQ],
                "attention_mask": [1] * len(ids[:MAX_SEQ])}

    ds = ds.map(build, remove_columns=ds.column_names)
    ds = ds.filter(lambda e: any(l != -100 for l in e["labels"]))  # drop all-masked (TRL #3927)

    if os.environ.get("PX_PREFLIGHT"):
        e = ds[0]; um = [i for i, l in enumerate(e["labels"]) if l != -100]
        print(f"PREFLIGHT: {len(ds)} examples | ex0 unmasked(loss) tokens: {len(um)}/{len(e['input_ids'])}")
        print("decoded unmasked (must be ONLY Thought/Action spans + EOS):")
        print(repr(tokenizer.decode([e["input_ids"][i] for i in um]))[:500])
        return

    collator = DataCollatorForSeq2Seq(tokenizer, label_pad_token_id=-100, padding=True)
    trainer = Trainer(
        model=model,
        train_dataset=ds,
        data_collator=collator,
        args=TrainingArguments(
            per_device_train_batch_size=PER_DEVICE_BATCH_SIZE,
            gradient_accumulation_steps=GRAD_ACCUM,
            warmup_steps=WARMUP_STEPS,
            num_train_epochs=EPOCHS,
            max_steps=MAX_STEPS,
            learning_rate=LR,
            weight_decay=0.01,        # regularize the tiny corpus (Unsloth: 0.01-0.1)
            fp16=not is_bfloat16_supported(),
            bf16=is_bfloat16_supported(),
            logging_steps=5,
            optim="adamw_8bit",
            output_dir=os.path.join(OUT, "checkpoints"),
            save_strategy=SAVE_STRATEGY,
            report_to=REPORT_TO,
        ),
    )
    trainer.train()

    os.makedirs(OUT, exist_ok=True)
    model.save_pretrained(os.path.join(OUT, "adapter"))
    tokenizer.save_pretrained(os.path.join(OUT, "adapter"))
    # NOTE: do NOT call save_pretrained_gguf / save_pretrained_merged here — it re-downloads the
    # fp16 base from HF and HANGS on a dropped connection (CLOSE-WAIT). Merge offline instead with
    # the cached fp16 base: `python distill/merge_fp16.py` → out/gguf → convert → quantize → serve.
    print(f"\nLoRA adapter saved to {OUT}/adapter.")
    print("NEXT: python distill/merge_fp16.py  (offline merge) -> convert -> quantize -> serve -> gate")


if __name__ == "__main__":
    main()

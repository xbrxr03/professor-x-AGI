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
DATA = os.path.join(HERE, "data", "curated.jsonl")
OUT = os.path.join(HERE, "out")

# Match the harness's primary model.
BASE_MODEL = os.environ.get("PX_BASE_MODEL", "unsloth/Qwen3-8B-unsloth-bnb-4bit")
MAX_SEQ = int(os.environ.get("PX_MAX_SEQ", "8192"))
EPOCHS = float(os.environ.get("PX_EPOCHS", "2"))
# Base is Qwen3, served via Ollama's Qwen3 template — train with the matching template.
# (Turn 1 used qwen-2.5 here; combined with full-sequence loss it produced a model that
# reasoned but never emitted the action format or stopped. See PLAN_11_10.md.)
CHAT_TEMPLATE = os.environ.get("PX_CHAT_TEMPLATE", "qwen3")
LR = float(os.environ.get("PX_LR", "2e-4"))


def main():
    if not os.path.exists(DATA):
        sys.exit(f"No curated data at {DATA}. Run: python3 distill/curate.py")

    try:
        from unsloth import FastLanguageModel, is_bfloat16_supported
        from unsloth.chat_templates import get_chat_template
        from datasets import load_dataset
        from trl import SFTTrainer, DataCollatorForCompletionOnlyLM
        from transformers import TrainingArguments
    except ImportError as e:
        sys.exit(
            f"Missing deps ({e}).\n"
            "Install: pip install unsloth \"trl<0.10\" peft bitsandbytes accelerate datasets"
        )

    model, tokenizer = FastLanguageModel.from_pretrained(
        model_name=BASE_MODEL,
        max_seq_length=MAX_SEQ,
        load_in_4bit=True,
    )
    model = FastLanguageModel.get_peft_model(
        model,
        r=16,
        lora_alpha=16,
        lora_dropout=0.0,
        target_modules=["q_proj", "k_proj", "v_proj", "o_proj",
                        "gate_proj", "up_proj", "down_proj"],
        use_gradient_checkpointing="unsloth",
    )
    tokenizer = get_chat_template(tokenizer, chat_template=CHAT_TEMPLATE)

    ds = load_dataset("json", data_files=DATA, split="train")

    # RAW ReAct format — train the model in the SAME text format the harness serves it in.
    # The benchmark drives the model via raw /api/generate (NOT a chat template): SYSTEM_PROMPT,
    # then <task>, then "Thought:/Action:/Action Input:" turns with "Observation:" between them,
    # stop=["Observation:"]. A chat-template-trained model is out-of-distribution there and loops.
    # So render the curated messages as that raw text (system content is already SYSTEM_PROMPT).
    # End each assistant turn with EOS so the model also learns a hard stop. See PLAN_11_10.md.
    EOS = tokenizer.eos_token or "<|im_end|>"

    def fmt(ex):
        parts = []
        for m in ex["messages"]:
            role, content = m["role"], m["content"]
            if role == "system":
                parts.append(content)
            elif role == "user":
                parts.append(f"<task>\n{content}\n</task>")
            elif role == "assistant":
                parts.append(content + EOS)   # "Thought:..\nAction:..\nAction Input:..<eos>"
            elif role == "tool":
                parts.append(f"Observation: {content}")
        return {"text": "\n\n".join(parts)}

    ds = ds.map(fmt)

    if os.environ.get("PX_PREFLIGHT"):
        sample = ds[0]["text"]
        print("=== PREFLIGHT raw-format sample (first 1600 chars) ===")
        print(sample[:1600])
        print(f"\n[ends with EOS: {sample.rstrip().endswith(EOS)} | examples: {len(ds)}]")
        return

    trainer = SFTTrainer(
        model=model,
        tokenizer=tokenizer,
        train_dataset=ds,
        dataset_text_field="text",
        max_seq_length=MAX_SEQ,
        packing=False,
        args=TrainingArguments(
            per_device_train_batch_size=1,
            gradient_accumulation_steps=8,
            warmup_steps=10,
            num_train_epochs=EPOCHS,
            learning_rate=LR,
            # Match the model's loaded precision: Ampere+ (e.g. the 3060) loads bf16, and Unsloth
            # rejects fp16 on a bf16 model. Pick automatically so this works across GPUs.
            fp16=not is_bfloat16_supported(),
            bf16=is_bfloat16_supported(),
            logging_steps=5,
            optim="adamw_8bit",
            output_dir=os.path.join(OUT, "checkpoints"),
            save_strategy="epoch",
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

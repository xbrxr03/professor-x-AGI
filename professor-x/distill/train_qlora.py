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
EPOCHS = float(os.environ.get("PX_EPOCHS", "1"))
LR = float(os.environ.get("PX_LR", "2e-4"))


def main():
    if not os.path.exists(DATA):
        sys.exit(f"No curated data at {DATA}. Run: python3 distill/curate.py")

    try:
        from unsloth import FastLanguageModel
        from unsloth.chat_templates import get_chat_template
        from datasets import load_dataset
        from trl import SFTTrainer
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
    tokenizer = get_chat_template(tokenizer, chat_template="qwen-2.5")

    ds = load_dataset("json", data_files=DATA, split="train")

    def fmt(ex):
        # tool-role turns fold into the conversation as observations
        msgs = []
        for m in ex["messages"]:
            role = m["role"]
            if role == "tool":
                msgs.append({"role": "user", "content": f"Observation: {m['content']}"})
            else:
                msgs.append(m)
        return {"text": tokenizer.apply_chat_template(msgs, tokenize=False)}

    ds = ds.map(fmt)

    trainer = SFTTrainer(
        model=model,
        tokenizer=tokenizer,
        train_dataset=ds,
        dataset_text_field="text",
        max_seq_length=MAX_SEQ,
        args=TrainingArguments(
            per_device_train_batch_size=1,
            gradient_accumulation_steps=8,
            warmup_steps=10,
            num_train_epochs=EPOCHS,
            learning_rate=LR,
            fp16=True,
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
    # GGUF export for Ollama
    try:
        model.save_pretrained_gguf(os.path.join(OUT, "gguf"), tokenizer,
                                   quantization_method="q4_k_m")
        print(f"\nGGUF written to {OUT}/gguf — point distill/Modelfile at it.")
    except Exception as e:
        print(f"\nLoRA adapter saved to {OUT}/adapter (GGUF export skipped: {e})")

    print("\nNEXT: serve it, then ICS-gate it:")
    print("  ollama create professor-x-distilled -f distill/Modelfile")
    print("  # set DEFAULT_MODEL to professor-x-distilled, run --consciousness-report")
    print("  # ACCEPT only if pass@3 beats baseline by >0.033 AND ICS stays >=0.70")


if __name__ == "__main__":
    main()

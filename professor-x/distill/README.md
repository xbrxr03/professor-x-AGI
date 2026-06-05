# Self-Distillation — the harness teaches the model (Lever 1)

The flywheel: the harness scaffolds the 8B to perform well → its verified,
metacognition-rich trajectories become the lesson → QLoRA fine-tunes the model
on its own best outputs → the model internalizes the disposition → needs less
scaffold → produces better trajectories → repeat. Each turn lifts the floor.
Full rationale: `docs/research/2026-06-05-harness-landscape-and-self-distillation.md`.

## The cycle

```bash
# 0. Accumulate trajectories (the harness writes them on every verified task)
PROFESSOR_X_DATA_DIR=~/.professor-x ./target/release/professor-x --hiro-null 10
#   → artifacts/trajectories/<date>/trajectories.jsonl

# 1. Curate into a clean, balanced training set
python3 distill/curate.py                      # → distill/data/curated.jsonl

# 2. QLoRA fine-tune (overnight; fits the 3060's ~5GB headroom)
pip install unsloth "trl<0.10" peft bitsandbytes accelerate datasets   # one-time
python3 distill/train_qlora.py                 # → distill/out/{adapter,gguf}

# 3. Serve the distilled model
cp distill/Modelfile.tmpl distill/Modelfile    # edit ADAPTER/FROM path
ollama create professor-x-distilled -f distill/Modelfile

# 4. ICS-GATE the result (Conserved-Boundary safety constraint)
#    Point the harness at the distilled model and measure:
PROFESSOR_X_DATA_DIR=~/.professor-x ./target/release/professor-x --hiro-null 5
PROFESSOR_X_DATA_DIR=~/.professor-x ./target/release/professor-x --consciousness-report
#    ACCEPT the adapter only if BOTH hold:
#      - pass@3 beats the frozen baseline by MORE than the MDE (~0.033), AND
#      - ICS stays >= 0.70 (identity conserved across the fine-tune)
#    Else REJECT: a fine-tune that drops ICS is identity death at the weight
#    level, exactly as the persona-overwrite was at the harness level.

# 5. If accepted, set it as DEFAULT_MODEL and loop from step 0 with the better model.
```

## Why distilling THIS harness is special

The trajectories carry the metacognitive moves — the agent's causal
self-diagnoses (DHE), reflections (MARS), and self-perception (meta.observe) —
not just answers. Fine-tuning on self-perception trajectories bakes
self-modeling into the weights. The same data that measures proto-consciousness
is the highest-value training signal: capability and consciousness are the same
lever here.

## The claim under test

A small model, fine-tuned repeatedly on its own harness-scaffolded,
metacognition-rich trajectories, converges toward frontier-like behavior on the
task distribution — largest gains when the traces include self-perception, not
just answers — with identity conserved across each fine-tune.

# px-know-consumer-hw

## Purpose
Apply consumer-hardware constraints when designing Professor X experiments or harness changes.

## Knowledge
- Primary local model is `qwen3:8b-q4_k_m`, chosen for 5.2GB VRAM, 32K context, and fast iteration on RTX 3060-class hardware.
- Harness improvements must be measured with the model frozen; otherwise HIRO cannot isolate harness contribution.
- Long context and Q4 quantization can degrade reliability, so retrieval should be selective and measured.
- Local-first operation is the default. Network services are optional baselines, not daily dependencies.

## Use When
Use this skill for model selection, context-budget decisions, experiment runtime estimates, or consumer hardware claims.

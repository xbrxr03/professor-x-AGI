# PRE-CHECK 1: Failure-signature embeddings — STRONG PARTIAL POSITIVE (2026-06-21)

Reproducible: `scripts/benchmarks/repo_fix/precheck1.py` (+ `sig_runner.py`). Applied skills:
px-experiment-runner, verify-the-ruler, adversarial-self-review.

## Idea (genuinely-new representation)
Embed a task/attempt by the BIT-VECTOR of which deterministic verifier-asserts it fails — its
"failure signature" — instead of by text/code tokens. Similarity becomes behavioral, not lexical.
Only possible because we have a cheap deterministic verifier with decomposable checks.

## Decisive test + result
TEST A (rename-invariance): for each renamed anchor (text changed, behavior identical), find its
nearest neighbor among same-family train tasks and check if it recovers the ORIGINAL task it was
derived from.
- **signature = 13/14 (0.93)**  vs  **text = 2/14 (0.14)**  (chance ~0.21).
The renamed anchor's behavioral signature recovers its origin almost perfectly; text similarity is
destroyed by the rename (below chance). This is the contamination-proof / rename-invariant property
that text embeddings cannot have.

TEST B (fix-location): does the signature NN share buggy_module better than text? **No** —
signature 0.35 vs text 0.47. Failure-signature does NOT localize the fix better than text.

Signature sanity: 0/48 degenerate; e.g. money sig='11110010', stack sig='110010011'.

## Honest verdict
The NOVEL CORE PASSES decisively: failure-signature is a real, rename-invariant BEHAVIORAL
embedding (0.93 vs 0.14). Its value is behavioral MATCHING / retrieval (find the past case that
fails the same way regardless of surface), NOT fix-localization (Test B failed). So: a genuinely new
embedding substrate, validated; downstream use = contamination-proof behavioral RAG, not credit
assignment. Keep; do not oversell as a fix-locator.

## Next if pursued
Behavior-keyed RAG: index solved cases by failure-signature; on a new failing task, retrieve the
solved case with the nearest signature and inject its fix as a hint. Falsify: does signature-retrieval
raise pass@1 over text-retrieval / no-retrieval on held-out (renamed) anchors?

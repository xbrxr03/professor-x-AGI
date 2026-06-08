# px-deep-research

## Purpose
Conduct rigorous, citation-grounded research that survives a hostile review —
not vibe-summaries. Distilled from the academic-research-skills `deep-research`
13-agent pipeline (`_refs/academic-research-skills/deep-research/`).

## When to use
Investigating a question whose answer must be defensible: literature on a
mechanism, a design decision that needs evidence, or finding the real gap a new
direction fills.

## Workflow (6 phases)
1. **Scope** — Convert the vague topic into ONE FINER research question
   (Feasible, Interesting, Novel, Ethical, Relevant). State in/out of scope and
   2-3 sub-questions. A question you cannot answer is the wrong question.
2. **Investigate** — Systematic search; grade every source on the evidence
   hierarchy (meta-analysis > RCT > cohort > case > expert opinion). Predatory
   journal + conflict-of-interest screen.
3. **Analyze** — Cross-source synthesis: where do sources converge, where
   diverge? Resolve contradictions explicitly. Name the knowledge gap.
4. **Compose** — Findings → Discussion → Limitations. Every claim cited.
5. **Review (devil's advocate)** — Three mandatory checkpoints: cherry-picking
   check, confirmation-bias check, strongest counter-argument, "so what?"
   significance. Critical-severity issues block.
6. **Revise** — Address feedback; unresolved issues become "Acknowledged
   Limitations", never smoothed away.

## Iron rules (non-negotiable)
- Every claim has a citation. No unsupported assertions.
- "Difficult to verify" = FAIL, not "uncertain". If you cannot confirm a source
  exists, it does not enter the report.
- Never fabricate a reference by mixing elements of real papers (vibe-citing).
- Report the full evidence landscape including contradicting findings.
- Every report has an explicit limitations section + AI-assistance disclosure.

## Output Contract
A dated research note under `docs/research/` with: research question, graded
sources, synthesis, named gap, limitations. Confidence claims only where a
citation or local run artifact backs them (see `px-synthesize`).

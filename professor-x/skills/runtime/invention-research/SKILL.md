---
name: invention-research
description: Use when the goal is to invent a genuinely new idea, theory, or research direction rather than summarize existing work. Trigger for cross-disciplinary invention, novelty search, prior-art pressure-testing, white-space discovery, or turning a vague ambition into a falsifiable candidate mechanism.
allowed-tools:
  - Bash(*)
  - Read
  - Grep
  - Glob
---

# Invention Research

## Overview

Use this skill to search for invention territory instead of stopping at
literature summary. The job is to find a candidate mechanism or framework that
survives strong prior-art checks, cross-disciplinary comparison, and an honest
"how would this fail?" pass.

This skill is for invention work, not ordinary research notes.

If the task centers on papers in PDF form, use the `pdf` skill alongside this
one. If the task needs an executable exploratory notebook, use the
`jupyter-notebook` skill after the research question and measurements are clear.

## When To Use

Use this skill when the user asks for any of the following:

- invent something genuinely new
- find white space in a crowded field
- connect ideas across multiple disciplines
- pressure-test whether an idea is actually novel
- turn a fuzzy ambition into a defensible theory candidate
- identify what current systems or papers still fail to explain

Do not use this skill for:

- routine fact lookup
- single-paper summary
- ordinary coding tasks
- implementation work before the research object is clear

## Workflow

### 1. Frame the hard question

Reduce the request to one concrete research question with a clear bar for
success. Good questions target a missing mechanism, not a vibe.

Examples:

- "What mechanism could explain why some internal states become self-relevant,
  globally effective, and stable over time?"
- "What architecture could produce self-improvement without identity drift?"

If the user asks for "something new" in broad terms, define:

- target domain
- what counts as novelty
- what would count as failure

### 2. Read local context first

If the repo already contains research notes, benchmark docs, or architecture
constraints, read those before browsing. Build an inventory of:

- existing theses
- already-tested ideas
- explicit constraints
- dead ends to avoid repeating

### 3. Build a discipline lattice

Identify the 4-8 disciplines most likely to touch the problem. Use
`references/discipline-lattice.md` when needed.

For each discipline, extract:

- what it explains well
- what it leaves out
- what concept or method might transfer

The goal is not breadth for its own sake. The goal is structural overlap.

### 4. Run the prior-art kill pass

Before calling anything novel, search for the strongest adjacent work and try to
kill the claim. Use `references/novelty-pressure-test.md`.

Check:

- same mechanism under another name
- same architecture with different framing
- same measurement idea in a neighboring field
- current repos that already operationalize the core move

Keep a ledger:

- `dead`: prior art already occupies it
- `partial`: exists in fragments but not in the full mechanism
- `survives`: still looks like white space after cross-checking

Weak novelty claims should die fast.

### 5. Run the bridge pass

Search for transferable structure from other fields:

- math
- control
- information theory
- quant / mechanism design
- neuroscience
- psychology
- philosophy
- ML / interpretability

Do not borrow jargon alone. Borrow the constraint, method, or formal object.

Ask:

- what is the hidden conserved quantity?
- what is the scarce resource?
- what is the missing invariant?
- what becomes measurable in one field that is still hand-wavy in another?

### 6. Keep only the surviving mechanism

The output should not be "combine A and B."

The output should be a candidate mechanism with:

- a name
- a core claim
- the gap it fills
- the closest overlaps
- the one thing it adds that those overlaps do not

If that cannot be stated cleanly, the invention has not survived yet.

### 7. Force falsification

Every candidate needs a kill path before it gets promoted. Use
`references/falsification-checklist.md`.

Define:

- what evidence would kill it
- what baseline would beat it
- what measurement would show it is only metaphor
- what part is theory vs engineering layer vs evaluation trick

If the theory cannot fail, it is not ready.

### 8. Produce invention-grade artifacts

The default artifact is a research note or memo containing:

- question
- source lattice
- dead claims
- surviving candidate
- nearest prior art
- falsification criteria
- next experiment or benchmark

Do not present a candidate as "genuinely new" without the caveat that novelty is
provisional unless a full prior-art search has been done.

## Resource Guide

- For deeper step-by-step guidance, read `references/workflow.md`.
- For adjacent fields to scan, read `references/discipline-lattice.md`.
- For novelty-killing questions, read `references/novelty-pressure-test.md`.
- For falsification criteria, read `references/falsification-checklist.md`.

## Output Contract

Return:

- `research_question`
- `discipline_lattice`
- `dead_claims`
- `surviving_candidate`
- `closest_overlaps`
- `why_it_survives_for_now`
- `kill_criteria`
- `next_research_move`

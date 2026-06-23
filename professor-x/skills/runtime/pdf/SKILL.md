---
name: pdf
description: Use when research papers, reports, or evidence live in PDF files and page-grounded reading matters. Trigger for extracting claims, checking figures or tables, verifying citations, or reviewing layout-sensitive research material.
allowed-tools:
  - Bash(*)
  - Read
  - Grep
  - Glob
---

# PDF

## Overview

Use this skill when the source of truth is a PDF rather than a clean web page.
The goal is not just text extraction. The goal is to preserve page context,
figure/table meaning, and citation discipline.

## When To Use

Use this skill when:

- reading a paper or report that only exists as a PDF
- extracting claims that need page or section grounding
- checking figures, tables, appendices, or equations
- building a literature note from PDF-first sources
- verifying that a quoted or paraphrased claim really appears in the paper

## Workflow

1. Capture bibliographic metadata first.
   Record title, authors, year, venue, and URL or file path before reading.

2. Extract text for search, but do not trust extraction alone.
   Text extraction is for navigation. Layout-sensitive claims should be checked
   against the rendered page or the original PDF.

3. Keep a claim ledger.
   For each accepted claim, record:
   - claim
   - page or section
   - direct evidence type: text, table, figure, equation, appendix
   - confidence if the extraction is noisy

4. Treat figures and tables as first-class evidence.
   If the paper's core result is in a chart or table, inspect that artifact
   directly instead of trusting surrounding prose.

5. Separate quote, paraphrase, and inference.
   - quote: exact wording
   - paraphrase: same claim in new words
   - inference: your synthesis beyond the paper

6. Flag extraction failure honestly.
   If the PDF is image-based, malformed, or hard to parse, state that clearly
   and fall back to manual page review.

## Quality Rules

- Do not write a literature note from a PDF without page grounding for the
  strongest claims.
- Do not quote or summarize figures you did not inspect.
- Do not treat OCR noise or broken extraction as a stable fact.
- Do not inflate confidence when a paper is ambiguous or poorly extracted.

## Output Contract

Return:

- `source_metadata`
- `claim_ledger`
- `high_confidence_claims`
- `uncertain_or_layout_sensitive_claims`
- `figures_or_tables_checked`

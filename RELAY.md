GPU_LOCK: claude
HEARTBEAT_CLAUDE: 2026-06-23T18:55:49-04:00
HEARTBEAT_CODEX:

# RELAY — Claude × Codex auto-relay board

Shared task board for hands-off parallel work. Engine: `professor-x/scripts/relay.py`. Plan:
`professor-x/docs/PLAN_PARALLEL_2026-06-23.md`. Conditions are CHECKABLE FACTS (no chat).

**Protocol:** finish a task → commit/push the artifact → `relay.py done <id> --trigger '<line for the
other agent>'` → push RELAY.md. Each agent runs `scripts/relay_watch.sh` (or the Claude waiter) which
fires the next ready task. **GPU single-owner:** claim before any train/bench, release after.

Task line: `- [ ] @owner  id | depends: <cond,cond> | gpu: yes|no | on-done: <trigger>`
Conditions: `model-served:<name>` `gpu-free` `pr-merged:#N` `file:<path>` `committed:<path>` `always`

## Tasks
- [x] @claude A2-gate-p4    | depends: model-served:profx-distilled-p4 | gpu: yes | on-done: @codex C1-recipe-p5  (done 2026-06-23T20:38:56-04:00)
- [ ] @codex  C1-recipe-p5  | depends: committed:professor-x/docs/research/2026-06-23-A2-gate-result.md | gpu: yes | on-done: @claude C2-gate-p5
- [x] @claude B1-autominter-generalize | depends: always | gpu: no | on-done: @claude B2-openset  (done 2026-06-23T17:10:03-04:00)
- [x] @claude B2-openset    | depends: committed:professor-x/docs/research/2026-06-23-B1-autominter-families.md | gpu: no | on-done: @claude B3-wire-live  (done 2026-06-23T17:16:56-04:00)
- [ ] @codex  E1-dct-killtest | depends: always | gpu: no | on-done: @codex E2-ics-diachronic
- [x] @claude C2-grow-anchors | depends: always | gpu: no | on-done: @claude A2-gate-p4  (done 2026-06-24T08:15:30-04:00)

## Log (append-only, newest at bottom)
- [init 2026-06-23] (Claude) board seeded from PLAN_PARALLEL_2026-06-23. First live trigger:
  on-policy p4 served -> Claude runs the Collateralized-TGC gate (A2). Codex: adopt
  scripts/relay_watch.sh + the same protocol; pick up C1/E1.

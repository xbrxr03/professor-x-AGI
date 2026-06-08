#!/usr/bin/env python3
"""Live, polished transcript of Professor X working — styled like a coding-agent CLI.

Usage:  ./prof-x-stream.py          (follows today's event log)
        ./prof-x-stream.py --plain  (no colour, e.g. piping to a file)
"""
import json
import os
import shutil
import subprocess
import sys
import textwrap
from datetime import date

PLAIN = "--plain" in sys.argv or not sys.stdout.isatty()

# ── colours ────────────────────────────────────────────────────────────────
def _c(code):
    return "" if PLAIN else f"\033[{code}m"

RESET   = _c("0")
DIM     = _c("2")
BOLD    = _c("1")
GREY    = _c("38;5;245")
CYAN    = _c("38;5;44")
BLUE    = _c("38;5;75")
GREEN   = _c("38;5;78")
RED     = _c("38;5;203")
YELLOW  = _c("38;5;179")
MAGENTA = _c("38;5;176")
WHITE   = _c("38;5;252")

def width():
    return max(60, shutil.get_terminal_size((100, 40)).columns)

def wrap(text, indent, first_prefix):
    w = width()
    body_indent = " " * indent
    lines = textwrap.wrap(text, width=w - indent, break_long_words=True) or [""]
    out = [f"{first_prefix}{lines[0]}"]
    out += [f"{body_indent}{l}" for l in lines[1:]]
    return "\n".join(out)

def short(s, n):
    s = " ".join((s or "").split())
    return s if len(s) <= n else s[: n - 1] + "…"

# ── event rendering ──────────────────────────────────────────────────────────
def render(ev):
    et = ev.get("event_type", "")
    summ = ev.get("summary", "")
    pay = ev.get("payload", {}) or {}
    ts = (ev.get("timestamp", "")[11:19]) or ""

    if et == "task.started":
        desc = short(summ.replace("started task:", "").strip(), width() - 6)
        return f"\n{BOLD}{CYAN}⏺ TASK{RESET}  {WHITE}{desc}{RESET}"

    if et == "llm.response":
        thought = pay.get("preview", "")
        # strip the leading "Thought:" label if present, keep it readable
        thought = thought.replace("Thought:", "").strip()
        # cut off at the Action: marker so we show reasoning, not the raw action
        for marker in ("Action:", "Action Input:"):
            i = thought.find(marker)
            if i > 0:
                thought = thought[:i].strip()
        if not thought:
            return None
        body = wrap(short(thought, 400), 4, f"  {DIM}{GREY}· {RESET}{GREY}")
        return f"{body}{RESET}"

    if et == "tool.started":
        # summary like: running tool 'fs.read' :: path=...
        tool = pay.get("tool") or _extract_tool(summ)
        detail = ""
        if "::" in summ:
            detail = summ.split("::", 1)[1].strip()
        return f"  {GREEN}⏺{RESET} {BOLD}{tool}{RESET}{DIM}{GREY}  {short(detail, width()-len(tool)-8)}{RESET}"

    if et == "tool.succeeded":
        out = short(pay.get("output_preview", ""), width() - 8)
        if not out:
            return f"    {DIM}{GREEN}⎿  ok{RESET}"
        return f"    {DIM}{GREEN}⎿  {out}{RESET}"

    if et == "tool.failed":
        return f"    {RED}⎿  ✗ {short(summ, width()-8)}{RESET}"

    if et == "react.duplicate_action":
        return f"    {MAGENTA}⊘  duplicate action blocked — nudged to move on{RESET}"

    if et == "react.circuit_breaker":
        return f"    {MAGENTA}‖  circuit breaker (3 failures) — backing off{RESET}"

    if et.startswith("policy.den"):
        return f"    {YELLOW}·  {short(summ, width()-8)}{RESET}"

    if et == "task.succeeded":
        return f"  {BOLD}{GREEN}✔ done{RESET}"

    if et == "task.failed":
        fm = short(pay.get("failure_mode", summ), width() - 12)
        return f"  {BOLD}{RED}✗ failed{RESET}  {DIM}{GREY}{fm}{RESET}"

    if et.startswith("hiro.round.started"):
        return f"\n{BOLD}{BLUE}{'━'*width()}{RESET}\n{BOLD}{BLUE}  {summ}{RESET}\n{BOLD}{BLUE}{'━'*width()}{RESET}"

    if et.startswith("hiro.round.completed"):
        return f"\n{BOLD}{BLUE}  {summ}{RESET}"

    return None

def _extract_tool(summ):
    if "'" in summ:
        try:
            return summ.split("'")[1]
        except IndexError:
            pass
    return summ

# ── follow loop ────────────────────────────────────────────────────────────
def main():
    here = os.path.dirname(os.path.abspath(__file__))
    f = os.path.join(here, "artifacts", "events", f"{date.today().isoformat()}.jsonl")
    print(f"{DIM}{GREY}── Professor X live ──  {f}")
    print(f"   Ctrl-C to stop (does not stop the run) ──{RESET}\n")
    while not os.path.exists(f):
        print(f"{DIM}waiting for event log…{RESET}")
        import time; time.sleep(2)

    # follow with tail -F for robustness
    p = subprocess.Popen(["tail", "-n", "25", "-F", f],
                         stdout=subprocess.PIPE, text=True, bufsize=1)
    try:
        for line in p.stdout:
            line = line.strip()
            if not line:
                continue
            try:
                ev = json.loads(line)
            except json.JSONDecodeError:
                continue
            out = render(ev)
            if out is not None:
                print(out, flush=True)
    except KeyboardInterrupt:
        pass
    finally:
        p.terminate()

if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""Auto-relay engine for the Claude × Codex parallel setup (stdlib only, git-native).

Reads/updates RELAY.md (the shared task board), evaluates *checkable-fact* conditions (no chat), and
manages the single-GPU lock so the two agents hand off automatically.

A task line in RELAY.md:
  - [ ] @claude  A2-gate-p4 | depends: model-served:profx-distilled-p4 | gpu: yes | on-done: @codex C1
Conditions (facts, AND-joined by ','):
  model-served:<name>   ollama has the model           gpu-free            no GPU compute proc + lock free
  pr-merged:#<n>        gh PR merged                   file:<path>         path exists in the worktree
  committed:<path>      path tracked in trunk HEAD      always              unconditional
Commands:
  relay.py ready --agent claude        # tasks I own whose depends are satisfied (gpu respected)
  relay.py check '<cond,cond>'         # eval a condition string -> exit 0 if all true
  relay.py done <id> [--trigger '...'] # mark [x], optional append a new task, bump heartbeat
  relay.py gpu <claim|release|status> --agent <a>
  relay.py heartbeat --agent <a>
"""
import argparse, os, re, subprocess, sys, datetime

HERE = os.path.dirname(os.path.abspath(__file__))
RELAY = os.environ.get("RELAY_FILE", os.path.join(os.path.dirname(HERE), "..", "RELAY.md"))
RELAY = os.path.abspath(RELAY)
TASK_RE = re.compile(r"^- \[(?P<done>[ x])\]\s+@(?P<agent>\w+)\s+(?P<id>[\w.-]+)\s*\|(?P<rest>.*)$")


def now(): return datetime.datetime.now().astimezone().isoformat(timespec="seconds")
def read(): return open(RELAY).read().splitlines() if os.path.exists(RELAY) else []
def write(lines): open(RELAY, "w").write("\n".join(lines) + "\n")


def gpu_lock():
    for ln in read():
        m = re.match(r"GPU_LOCK:\s*(\w+)", ln)
        if m: return m.group(1)
    return "free"


def gpu_busy():
    """True if a GPU compute process is actually running (independent of the soft lock)."""
    try:
        out = subprocess.run(["nvidia-smi", "--query-compute-apps=pid", "--format=csv,noheader"],
                             capture_output=True, text=True, timeout=15).stdout.strip()
        return bool(out)
    except Exception:
        return False


def field(rest, key):
    m = re.search(rf"{key}:\s*([^|]+)", rest)
    return m.group(1).strip() if m else None


def cond_true(cond):
    cond = cond.strip()
    if cond in ("", "always"):
        return True
    kind, _, arg = cond.partition(":")
    kind = kind.strip(); arg = arg.strip()
    if kind == "model-served":
        try:
            return arg in subprocess.run(["ollama", "list"], capture_output=True, text=True, timeout=20).stdout
        except Exception:
            return False
    if kind == "gpu-free":
        return gpu_lock() == "free" and not gpu_busy()
    if kind == "pr-merged":
        n = arg.lstrip("#")
        try:
            return subprocess.run(["gh", "pr", "view", n, "--json", "state", "-q", ".state"],
                                  capture_output=True, text=True, timeout=20).stdout.strip() == "MERGED"
        except Exception:
            return False
    if kind == "file":
        return os.path.exists(arg if os.path.isabs(arg) else os.path.join(os.path.dirname(RELAY), arg))
    if kind == "committed":
        try:
            return subprocess.run(["git", "cat-file", "-e", f"origin/prereboot-flywheel-prep:{arg}"],
                                  cwd=os.path.dirname(RELAY), capture_output=True, timeout=20).returncode == 0
        except Exception:
            return False
    return False   # unknown condition = not satisfied (fail closed)


def all_conds(depends):
    return all(cond_true(c) for c in (depends or "always").split(","))


def parse_tasks():
    out = []
    for i, ln in enumerate(read()):
        m = TASK_RE.match(ln)
        if m:
            d = m.groupdict(); d["line"] = i
            d["depends"] = field(d["rest"], "depends") or "always"
            d["gpu"] = (field(d["rest"], "gpu") or "no").lower() in ("yes", "true", "1")
            d["on_done"] = field(d["rest"], "on-done")
            out.append(d)
    return out


def cmd_ready(a):
    rdy = []
    for t in parse_tasks():
        if t["done"] == "x" or t["agent"] != a.agent:
            continue
        if not all_conds(t["depends"]):
            continue
        if t["gpu"] and (gpu_lock() not in ("free", a.agent) or gpu_busy()):
            continue
        rdy.append(t)
    for t in rdy:
        print(f"{t['id']}\t(depends: {t['depends']}\tgpu: {t['gpu']})")
    sys.exit(0 if rdy else 3)   # exit 3 = nothing ready (watcher keeps polling)


def cmd_check(a):
    ok = all_conds(a.cond)
    print("TRUE" if ok else "FALSE")
    sys.exit(0 if ok else 1)


def cmd_done(a):
    lines = read()
    for t in parse_tasks():
        if t["id"] == a.id:
            lines[t["line"]] = lines[t["line"]].replace("- [ ]", "- [x]", 1) + f"  (done {now()})"
    if a.trigger:
        # append a new task line verbatim (the on-done trigger for the other agent)
        idx = next((i for i, l in enumerate(lines) if l.strip().lower().startswith("## tasks")), len(lines) - 1)
        lines.insert(idx + 1, a.trigger if a.trigger.startswith("- [") else f"- [ ] {a.trigger}")
    write(lines)
    print(f"marked {a.id} done" + (f"; appended trigger" if a.trigger else ""))


def cmd_gpu(a):
    lines = read(); cur = gpu_lock()
    if a.op == "status":
        print(f"GPU_LOCK={cur}  busy={gpu_busy()}"); return
    if a.op == "claim":
        if cur not in ("free", a.agent):
            print(f"REFUSED: held by {cur}"); sys.exit(1)
        newv = a.agent
    else:  # release
        if cur not in ("free", a.agent):
            print(f"REFUSED: held by {cur}, not {a.agent}"); sys.exit(1)
        newv = "free"
    found = False
    for i, l in enumerate(lines):
        if l.startswith("GPU_LOCK:"):
            lines[i] = f"GPU_LOCK: {newv}"; found = True
    if not found:
        lines.insert(0, f"GPU_LOCK: {newv}")
    write(lines); print(f"GPU_LOCK -> {newv}")


def cmd_heartbeat(a):
    key = f"HEARTBEAT_{a.agent.upper()}:"
    lines = read(); found = False
    for i, l in enumerate(lines):
        if l.startswith(key):
            lines[i] = f"{key} {now()}"; found = True
    if not found:
        lines.insert(0, f"{key} {now()}")
    write(lines); print(f"{key} {now()}")


def main():
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest="cmd", required=True)
    p = sub.add_parser("ready"); p.add_argument("--agent", required=True); p.set_defaults(fn=cmd_ready)
    p = sub.add_parser("check"); p.add_argument("cond"); p.set_defaults(fn=cmd_check)
    p = sub.add_parser("done"); p.add_argument("id"); p.add_argument("--trigger"); p.set_defaults(fn=cmd_done)
    p = sub.add_parser("gpu"); p.add_argument("op", choices=["claim", "release", "status"]); p.add_argument("--agent", default="free"); p.set_defaults(fn=cmd_gpu)
    p = sub.add_parser("heartbeat"); p.add_argument("--agent", required=True); p.set_defaults(fn=cmd_heartbeat)
    a = ap.parse_args(); a.fn(a)


if __name__ == "__main__":
    main()

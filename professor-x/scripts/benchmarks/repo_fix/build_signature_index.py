#!/usr/bin/env python3
"""Build the behavior-keyed retrieval corpus: signature_index.json.
For each reuse-family task, record its FAILURE SIGNATURE (computed on its shipped buggy state via
sig_runner.py) and a FIX HINT (the corrected content of the buggy module, reconstructed from a sibling
whose bug is elsewhere). At agent time the harness computes the current task's signature and retrieves
the nearest entry's hint (see src/agentd/fault_signature.rs). Index lives next to this script."""
import os, glob, json, subprocess
BASE = os.path.dirname(os.path.abspath(__file__))
RUN = os.path.join(BASE, "sig_runner.py")

def sig(task_dir):
    return subprocess.run(["python3", RUN], cwd=task_dir, capture_output=True, text=True).stdout.strip()

def read_mods(d):
    return {os.path.basename(f): open(f).read()
            for f in glob.glob(f"{d}/*.py") if os.path.basename(f) != "check.py"}

entries = []
for mf in sorted(glob.glob(f"{BASE}/tasks_family_*.json")):
    fam = json.load(open(mf)); name = fam["family"]; tasks = fam["tasks"]
    # correct content of each module = content seen in a task whose buggy_module != that module
    correct = {}
    for t in tasks:
        for mn, c in read_mods(f"{BASE}/{t['id']}").items():
            if mn != t.get("buggy_module"):
                correct.setdefault(mn, c)
    for t in tasks:
        bm = t.get("buggy_module")
        s = sig(f"{BASE}/{t['id']}")
        if not s or set(s) <= {"1"}:   # skip degenerate/green signatures
            continue
        fix = correct.get(bm, "")
        hint = (f"A behaviorally-matching past task was fixed by correcting module `{bm}`. "
                f"The correct `{bm}` is:\n{fix.strip()}")
        entries.append({"id": t["id"], "family": name, "signature": s,
                        "buggy_module": bm, "hint": hint})

out = os.path.join(BASE, "signature_index.json")
json.dump({"entries": entries}, open(out, "w"), indent=2)
print(f"signature_index.json: {len(entries)} entries across "
      f"{len({e['family'] for e in entries})} families -> {out}")
for e in entries[:3]:
    print(f"  {e['id']:16} sig={e['signature']}")

#!/usr/bin/env python3
"""Living-Verifier beachhead (CPU, existing data).
(1) Is the verifier already a CODE — does each fault get a UNIQUE syndrome within its family?
(2) Active-diagnosis/rateless efficiency — min # of asserts (greedy by info) needed to keep all
    faults distinguishable (vs the full battery)."""
import os, glob, json, subprocess, itertools
BASE="/home/abrar/professor-x-main-integrate/professor-x/scripts/benchmarks/repo_fix"
RUN="/tmp/sig_runner.py"
def sig(d):
    return subprocess.run(["python3",RUN],cwd=d,capture_output=True,text=True).stdout.strip()
fams={}
for mf in sorted(glob.glob(f"{BASE}/tasks_family_*.json")):
    d=json.load(open(mf)); fams[d["family"]]=[t["id"] for t in d["tasks"]]
print(f"{'family':10} {'tasks':5} {'asserts':7} {'unique?':8} {'min#checks(rateless)':20}")
tot_full=tot_min=0
for fam,ids in fams.items():
    S={i:sig(f"{BASE}/{i}") for i in ids}
    L=len(next(iter(S.values())))
    vals=list(S.values())
    unique = len(set(vals))==len(vals)
    # greedy min subset of columns that keeps all rows distinct (identifying code)
    cols=list(range(L)); chosen=[]
    def distinct(cs):
        proj=[tuple(v[c] for c in cs) for v in vals]; return len(set(proj))==len(vals)
    remaining=set(range(L))
    while not distinct(chosen) and remaining:
        # pick the column that maximizes #distinct projections when added
        best=max(remaining, key=lambda c: len(set(tuple(v[x] for x in chosen+[c]) for v in vals)))
        chosen.append(best); remaining.discard(best)
    tot_full+=L; tot_min+=len(chosen)
    print(f"{fam:10} {len(ids):5} {L:7} {str(unique):8} {len(chosen):2}/{L}  -> {len(chosen)/L:.0%} of battery")
print(f"\nOVERALL: full battery sum={tot_full}, min-decoding sum={tot_min} ({tot_min/tot_full:.0%}) "
      f"-> rateless headroom = {1-tot_min/tot_full:.0%} of checks are redundant for decoding")
print("Interpretation: unique=True per family => verifier IS a locating code (syndrome decodes fault);")
print("min#checks << battery => active-diagnosis could decode with far fewer checks (rateless efficiency).")

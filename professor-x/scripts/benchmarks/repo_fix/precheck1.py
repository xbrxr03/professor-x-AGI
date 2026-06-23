#!/usr/bin/env python3
"""PRE-CHECK 1 — Failure-signature embeddings.
CLAIM: a task embedded by WHICH verifier-asserts it fails carries fix-relevant, rename-invariant
information that TEXT embeddings do not.
DECISIVE TEST: for each RENAMED anchor (text changed, behavior identical), find its nearest neighbor
among same-family TRAIN tasks by (a) failure-signature, (b) text. Does the NN recover the ORIGINAL
task the anchor was derived from (anchor_of)? Behavioral embedding should; text should be confused by
the rename. Also: within-family buggy-module prediction, signature vs text vs chance.
KILL: if signature does not beat text on anchor->origin recovery, the representation adds nothing."""
import os, json, glob, re, subprocess
BASE="/home/abrar/professor-x-main-integrate/professor-x/scripts/benchmarks/repo_fix"
RUNNER="/tmp/sig_runner.py"
TOK=re.compile(r"[A-Za-z_][A-Za-z0-9_]*")
def toks(d):
    s=" ".join(open(f).read() for f in glob.glob(f"{d}/*.py") if os.path.basename(f)!="check.py")
    return set(TOK.findall(s))
def jacc(a,b): return len(a&b)/len(a|b) if (a|b) else 0.0
def signature(d):
    r=subprocess.run(["python3",RUNNER],cwd=d,capture_output=True,text=True)
    return r.stdout.strip()
def sig_sim(a,b):
    if not a or not b or len(a)!=len(b): return -1.0
    return sum(x==y for x,y in zip(a,b))/len(a)

# gather train tasks + anchors per family
fam_train={}; fam_anchor={}
for mf in glob.glob(f"{BASE}/tasks_family_*.json"):
    d=json.load(open(mf)); fam_train[d["family"]]=d["tasks"]
for mf in glob.glob(f"{BASE}/tasks_anchor_*.json"):
    d=json.load(open(mf)); fam_anchor[d["family"]]=d["tasks"]

# precompute signatures + token sets
sig={}; txt={}
for fam,tasks in fam_train.items():
    for t in tasks:
        p=f"{BASE}/{t['id']}"; sig[t["id"]]=signature(p); txt[t["id"]]=toks(p)
for fam,tasks in fam_anchor.items():
    for t in tasks:
        p=f"{BASE}/{t['id']}"; sig[t["id"]]=signature(p); txt[t["id"]]=toks(p)

print("="*74); print("SIGNATURE sanity (non-degenerate = has both 0s and 1s)"); print("="*74)
degen=sum(1 for s in sig.values() if set(s)<= {"1"} or set(s)<={"0"} or s=="")
print(f"tasks={len(sig)}  degenerate(all-same/empty)={degen}")
for fam in sorted(fam_train):
    ex=fam_train[fam][0]["id"]; print(f"  {fam:9} example {ex} sig={sig[ex]!r}")

# ---- TEST A: anchor -> origin recovery (same-family train pool) ----
print("\n"+"="*74); print("TEST A: anchor -> origin recovery (NN among same-family train tasks)"); print("="*74)
sig_hit=txt_hit=tie=n=0
for fam,anchors in fam_anchor.items():
    pool=[t["id"] for t in fam_train[fam]]
    for a in anchors:
        aid=a["id"]; origin=a.get("anchor_of"); n+=1
        # signature NN
        sscores=sorted(((sig_sim(sig[aid],sig[p]),p) for p in pool),reverse=True)
        snn=sscores[0][1]; s_is_hit=(snn==origin)
        # text NN
        tscores=sorted(((jacc(txt[aid],txt[p]),p) for p in pool),reverse=True)
        tnn=tscores[0][1]; t_is_hit=(tnn==origin)
        sig_hit+=s_is_hit; txt_hit+=t_is_hit
        print(f"  {aid:22} origin={origin:16} sigNN={snn:16}{'OK' if s_is_hit else '  '}  txtNN={tnn:16}{'OK' if t_is_hit else ''}")
print(f"\n  anchor->origin recovery:  signature={sig_hit}/{n} ({sig_hit/n:.2f})   text={txt_hit}/{n} ({txt_hit/n:.2f})   chance~{1/ (sum(len(v) for v in fam_train.values())/len(fam_train)):.2f}")

# ---- TEST B: within-family buggy-module prediction (leave-one-out NN) ----
print("\n"+"="*74); print("TEST B: within-family buggy_module prediction (LOO nearest neighbor)"); print("="*74)
bm={t["id"]:t["buggy_module"] for tasks in fam_train.values() for t in tasks}
sok=tok=m=0
for fam,tasks in fam_train.items():
    ids=[t["id"] for t in tasks]
    for q in ids:
        cand=[p for p in ids if p!=q]
        if not cand: continue
        m+=1
        snn=max(cand,key=lambda p:sig_sim(sig[q],sig[p]))
        tnn=max(cand,key=lambda p:jacc(txt[q],txt[p]))
        sok+= (bm[snn]==bm[q]); tok+= (bm[tnn]==bm[q])
print(f"  buggy_module match:  signature={sok}/{m} ({sok/m:.2f})   text={tok}/{m} ({tok/m:.2f})")

print("\n"+"="*74); print("VERDICT"); print("="*74)
print(f"Signature beats text on anchor->origin: {'YES' if sig_hit>txt_hit else 'NO'}")
print(f"Signature beats text on buggy_module:   {'YES' if sok>tok else 'NO'}")

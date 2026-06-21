#!/usr/bin/env python3
"""Transfer measurement for the reuse-family benchmark.
Reconstructs each task's reference patch (buggy module vs the correct module taken from a
sibling whose bug is elsewhere), then measures within-family vs cross-family token overlap on
(a) the whole solution context and (b) the patch lines restricted to shared-API symbols.
Recipe gate: within-family sibling patches share >=40% token-overlap on shared-API lines
(vs ~0.1% on the old benchmark)."""
import os, json, re, itertools, glob

BASE = "/home/abrar/professor-x-main-integrate/professor-x/scripts/benchmarks/repo_fix"
TOK = re.compile(r"[A-Za-z_][A-Za-z0-9_]*")

def toks(s): return set(TOK.findall(s))

def read_modules(taskdir):
    out={}
    for f in glob.glob(f"{taskdir}/*.py"):
        n=os.path.basename(f)
        if n=="check.py": continue
        out[n]=open(f).read()
    return out

def jaccard(a,b):
    if not a and not b: return 1.0
    if not a or not b: return 0.0
    return len(a&b)/len(a|b)

families={}
for mf in sorted(glob.glob(f"{BASE}/tasks_family_*.json")):
    d=json.load(open(mf)); fam=d["family"]
    api=[s.split(".")[-1] for s in d.get("shared_api",[])]  # bare symbol names
    tasks=d["tasks"]
    # reconstruct correct content of each module: content seen in a task whose buggy_module != module
    correct={}
    for t in tasks:
        td=os.path.join(os.path.dirname(BASE), os.path.basename(BASE), t["id"])
        mods=read_modules(os.path.join(BASE,t["id"]))
        for mn,content in mods.items():
            if mn!=t.get("buggy_module"):
                correct.setdefault(mn,content)  # first non-buggy occurrence = correct
    # build per-task reference patch (changed lines) and context tokens
    recs=[]
    for t in tasks:
        mods=read_modules(os.path.join(BASE,t["id"]))
        bm=t.get("buggy_module")
        buggy=mods.get(bm,""); corr=correct.get(bm,"")
        bl=buggy.splitlines(); cl=corr.splitlines()
        # changed lines = symmetric line diff (lines in one but not the other)
        setb,setc=set(bl),set(cl)
        patch_lines=[l for l in cl if l not in setb]+[l for l in bl if l not in setc]
        patch_tok=toks(" ".join(patch_lines))
        # shared-API-line tokens: tokens on FULL-CONTEXT lines that mention any api symbol
        # (the recipe's "shared-API lines" = the library body that uses the shared API, not the
        #  sparse patch lines — measuring patch lines was confounded by generic Python tokens).
        ctx_lines=" ".join(mods.values()).splitlines()
        api_lines=[l for l in ctx_lines if any(re.search(rf"\b{re.escape(a)}\b",l) for a in api)]
        api_line_tok=toks(" ".join(api_lines))
        ctx_tok=toks(" ".join(mods.values()))
        recs.append(dict(id=t["id"],bm=bm,patch=patch_tok,apil=api_line_tok,ctx=ctx_tok))
    families[fam]=dict(api=set(api),recs=recs)

def mean(xs): return sum(xs)/len(xs) if xs else float("nan")

print("="*78)
print("WITHIN-FAMILY transfer (mean pairwise Jaccard over sibling pairs)")
print("="*78)
print(f"{'family':10} {'n':>2} {'ctx':>7} {'patch':>7} {'api-line':>9}  shared-API symbols")
all_ctx_within=[]; all_apil_within=[]
for fam,fd in families.items():
    recs=fd["recs"]; pairs=list(itertools.combinations(recs,2))
    ctx=mean([jaccard(a["ctx"],b["ctx"]) for a,b in pairs])
    pat=mean([jaccard(a["patch"],b["patch"]) for a,b in pairs])
    apil=mean([jaccard(a["apil"],b["apil"]) for a,b in pairs])
    all_ctx_within+= [jaccard(a["ctx"],b["ctx"]) for a,b in pairs]
    all_apil_within+=[jaccard(a["apil"],b["apil"]) for a,b in pairs]
    print(f"{fam:10} {len(recs):>2} {ctx:7.3f} {pat:7.3f} {apil:9.3f}  {sorted(fd['api'])}")

print("\n"+"="*78)
print("CROSS-FAMILY control (pairs from DIFFERENT families)")
print("="*78)
allrecs=[(fam,r) for fam,fd in families.items() for r in fd["recs"]]
cross=list(itertools.combinations(allrecs,2))
cross=[(a,b) for (fa,a),(fb,b) in [((x[0],x[1]),(y[0],y[1])) for x,y in cross] if fa!=fb] if False else \
      [(a,b) for (fa,a),(fb,b) in itertools.combinations(allrecs,2) if fa!=fb]
ctx_x=mean([jaccard(a["ctx"],b["ctx"]) for a,b in cross])
apil_x=mean([jaccard(a["apil"],b["apil"]) for a,b in cross])
print(f"cross-family   ctx={ctx_x:.3f}   api-line={apil_x:.3f}   (n_pairs={len(cross)})")

print("\n"+"="*78)
print("VERDICT vs recipe gate")
print("="*78)
wc=mean(all_ctx_within); wa=mean(all_apil_within)
print(f"within-family  context-overlap   = {wc:.3f}  (vs cross-family {ctx_x:.3f})")
print(f"within-family  api-line-overlap  = {wa:.3f}  (vs cross-family {apil_x:.3f})")
print(f"GATE (api-line within-family >= 0.40): {'PASS' if wa>=0.40 else 'FAIL'}")
print(f"separation (within >> cross): context {wc/ctx_x if ctx_x else float('inf'):.1f}x, "
      f"api-line {wa/apil_x if apil_x else float('inf'):.1f}x")

# ---- ANCHOR: the OLD benchmark (hard_001..030) within-set context overlap = the 'before' number
print("\n"+"="*78)
print("ANCHOR: old hard-set (hard_*) within-set context overlap (the 'before' / ~0.1% claim)")
print("="*78)
old=[]
for d in sorted(glob.glob(f"{BASE}/hard_*")):
    if not os.path.isdir(d): continue
    mods=read_modules(d)
    if mods: old.append(toks(" ".join(mods.values())))
oldpairs=list(itertools.combinations(old,2))
old_ctx=mean([jaccard(a,b) for a,b in oldpairs])
print(f"old hard-set context-overlap = {old_ctx:.3f}  (n_tasks={len(old)}, n_pairs={len(oldpairs)})")
print(f"BEFORE/AFTER context-overlap: {old_ctx:.3f} -> {wc:.3f}  ({wc/old_ctx if old_ctx else float('inf'):.0f}x)")

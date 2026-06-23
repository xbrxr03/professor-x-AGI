#!/usr/bin/env python3
"""B2 — open-set disambiguation: are the residual aliased faults COVERAGE GAPS or TRUE DUPLICATES?

For families B1 left partial (csv 4/5, stack 5/6), find the aliased variant group(s) and run an
INTENSIVE differential search (large budget + boundary-inclusive inputs). If ANY input distinguishes
the pair -> it was a coverage gap (the minter just needed more/edge probes; here's the separator). If
NO input distinguishes after a heavy search -> the two code-bugs are BEHAVIORALLY IDENTICAL (true
duplicates), so non-separation is CORRECT — resolving the beachhead's confounded 35% open-set number.
"""
import ast, os, glob, json, random, subprocess, sys
BASE = "/home/abrar/professor-x-main-integrate/professor-x/scripts/benchmarks/repo_fix"

def harvest(cp):
    tree = ast.parse(open(cp).read()); f2m = {}
    for n in ast.walk(tree):
        if isinstance(n, ast.ImportFrom):
            for a in n.names: f2m[a.name] = n.module
    seeds = []
    for n in ast.walk(tree):
        if isinstance(n, ast.Call) and isinstance(n.func, ast.Name) and n.func.id in f2m:
            try: seeds.append((n.func.id, tuple(ast.literal_eval(a) for a in n.args)))
            except Exception: pass
    return f2m, seeds

def trand(v, rng, boundary, d=0):
    if d > 4: return v
    if isinstance(v, bool): return rng.random() < 0.5
    if isinstance(v, int): return rng.choice([0,1,-1,2]) if boundary and rng.random()<0.5 else rng.randint(-6, 20)
    if isinstance(v, float): return round(rng.uniform(-8, 25), 2)
    if isinstance(v, str): return v if rng.random()<0.25 else "".join(rng.choice("ab,;1 \t") for _ in range(rng.randint(0,8)))
    if isinstance(v, tuple): return tuple(trand(x, rng, boundary, d+1) for x in v)
    if isinstance(v, list):
        if not v: return []
        n = rng.choice([0,1,2]) if boundary and rng.random()<0.4 else rng.randint(0, 6)
        return [trand(v[0], rng, boundary, d+1) for _ in range(n)]
    return v

EVAL = r'''
import sys,json,importlib.util,os
d=json.load(sys.stdin); os.chdir(d["dir"]); sys.path.insert(0,d["dir"])
def load(m):
    s=importlib.util.spec_from_file_location(m,os.path.join(d["dir"],m+".py"))
    mod=importlib.util.module_from_spec(s); sys.modules[m]=mod; s.loader.exec_module(mod); return mod
out=[]; cache={}
for func,mod,args in d["probes"]:
    try:
        if mod not in cache: cache[mod]=load(mod)
        out.append(repr(getattr(cache[mod],func)(*args)))
    except Exception as e: out.append("ERR:"+type(e).__name__)
print(json.dumps(out))
'''
def ev(vdir, probes):
    r = subprocess.run([sys.executable,"-c",EVAL], input=json.dumps({"dir":vdir,"probes":probes}),
                       capture_output=True, text=True, timeout=60)
    try: return json.loads(r.stdout.strip())
    except Exception: return ["ERR"]*len(probes)

def syndrome_groups(variants, f2m, seeds, rng, budget, boundary):
    pool = []
    for _ in range(budget):
        fn, args = rng.choice(seeds); pool.append([fn, f2m[fn], tuple(trand(a, rng, boundary) for a in args)])
    outs = {v: ev(v, pool) for v in variants}
    # full syndrome under ALL probes (max resolution)
    syn = {}
    for v in variants:
        bits = []
        for j in range(len(pool)):
            col = [outs[u][j] for u in variants]; maj = max(set(col), key=col.count)
            bits.append(1 if outs[v][j]==maj else 0)
        syn[v] = tuple(bits)
    groups = {}
    for v in variants: groups.setdefault(syn[v], []).append(os.path.basename(v))
    return [g for g in groups.values() if len(g) > 1], pool, outs

def intensive_separate(pair_dirs, f2m, seeds, rng, budget=6000):
    """Return the first probe that gives the two variants DIFFERENT output (a separator), else None."""
    a, b = pair_dirs
    for _ in range(budget):
        fn, args = rng.choice(seeds); probe = [[fn, f2m[fn], tuple(trand(x, rng, True) for x in args)]]
        oa, ob = ev(a, probe)[0], ev(b, probe)[0]
        if oa != ob and not (oa.startswith("ERR") and ob.startswith("ERR")):
            return probe[0], oa, ob
    return None

if __name__ == "__main__":
    for fam in ["csv", "stack"]:
        variants = sorted(d for d in glob.glob(f"{BASE}/fam_{fam}_*") if os.path.isdir(d) and "anchor" not in d)
        f2m, seeds = harvest(f"{variants[0]}/check.py")
        rng = random.Random(1)
        groups, _, _ = syndrome_groups(variants, f2m, seeds, rng, 600, boundary=True)
        print(f"\n=== {fam}: aliased group(s) after heavy fuzz = {groups or 'NONE (all unique!)'} ===")
        for g in groups:
            dirs = [f"{BASE}/{x}" for x in g]
            for i in range(len(dirs)):
                for j in range(i+1, len(dirs)):
                    sep = intensive_separate([dirs[i], dirs[j]], f2m, seeds, random.Random(7), budget=6000)
                    if sep:
                        print(f"  {g[i]} vs {g[j]}: COVERAGE GAP -> separable. e.g. {sep[0][0]}{sep[0][2]} "
                              f"-> {g[i]}={sep[1]} {g[j]}={sep[2]}")
                    else:
                        print(f"  {g[i]} vs {g[j]}: TRUE BEHAVIORAL DUPLICATE (no separator in 6000 probes) "
                              f"-> correctly non-separable")

#!/usr/bin/env python3
"""B1 — generalize the differential-testing auto-minter across all families (CPU).

Claim: blind differential testing auto-constructs a LOCATING CODE (unique syndrome per fault) for any
family — not just interval. Method (family-agnostic):
  - harvest seed inputs + the func->module import map from a family's check.py (AST);
  - generate candidate checks by typed-random mutation of the seeds;
  - reference output = MAJORITY vote across task variants (each task bugs only one module, so most
    variants are correct for a given call) -> no hand-written correct impl needed;
  - greedily keep checks that increase the number of distinct syndromes until every fault is unique.
Measure per family: faults, checks minted to fully separate, probes tried (rateless cost), success.
"""
import ast, os, glob, json, random, subprocess, sys, statistics
BASE = "/home/abrar/professor-x-main-integrate/professor-x/scripts/benchmarks/repo_fix"

def harvest(check_path):
    src = open(check_path).read()
    tree = ast.parse(src)
    func2mod = {}
    for n in ast.walk(tree):
        if isinstance(n, ast.ImportFrom):
            for a in n.names:
                func2mod[a.name] = n.module
    seeds = []  # (func, args_tuple)
    for n in ast.walk(tree):
        if isinstance(n, ast.Call) and isinstance(n.func, ast.Name) and n.func.id in func2mod:
            try:
                args = tuple(ast.literal_eval(a) for a in n.args)
                seeds.append((n.func.id, args))
            except Exception:
                pass
    return func2mod, seeds

def trand(v, rng, depth=0):
    if depth > 4: return v
    if isinstance(v, bool): return rng.random() < 0.5
    if isinstance(v, int): return rng.randint(-3, 12)
    if isinstance(v, float): return round(rng.uniform(-5, 15), 2)
    if isinstance(v, str): return v if rng.random()<0.3 else "".join(rng.choice("abc,12;") for _ in range(rng.randint(0,6)))
    if isinstance(v, tuple): return tuple(trand(x, rng, depth+1) for x in v)
    if isinstance(v, list):
        if not v: return []
        proto = v[0]; n = rng.randint(0, 4)
        return [trand(proto, rng, depth+1) for _ in range(n)]
    return v

EVAL = r'''
import sys, json, importlib.util, os
d = json.load(sys.stdin); os.chdir(d["dir"]); sys.path.insert(0, d["dir"])
def load(mod):
    spec = importlib.util.spec_from_file_location(mod, os.path.join(d["dir"], mod+".py"))
    m = importlib.util.module_from_spec(spec); sys.modules[mod]=m; spec.loader.exec_module(m); return m
out=[]
cache={}
for func, mod, args in d["probes"]:
    try:
        if mod not in cache: cache[mod]=load(mod)
        f=getattr(cache[mod], func)
        out.append(repr(f(*args)))
    except Exception as e:
        out.append("ERR:"+type(e).__name__)
print(json.dumps(out))
'''

def eval_variant(vdir, probes):
    payload = {"dir": vdir, "probes": probes}
    r = subprocess.run([sys.executable, "-c", EVAL], input=json.dumps(payload),
                       capture_output=True, text=True, timeout=30)
    try: return json.loads(r.stdout.strip())
    except Exception: return ["ERR:load"]*len(probes)

def run_family(fam, budget=400):
    variants = sorted(d for d in glob.glob(f"{BASE}/fam_{fam}_*") if os.path.isdir(d) and "anchor" not in d)
    if len(variants) < 2: return None
    func2mod, seeds = harvest(f"{variants[0]}/check.py")
    if not seeds: return None
    rng = random.Random(0)
    # candidate probe pool: typed-random mutations of seeds
    pool = []
    for _ in range(budget):
        func, args = rng.choice(seeds)
        pool.append([func, func2mod[func], tuple(trand(a, rng) for a in args)])
    # evaluate the whole pool on every variant once (batch)
    outs = {v: eval_variant(v, pool) for v in variants}
    # greedily select probes that maximize distinct syndromes (majority reference per probe)
    chosen, probed = [], 0
    def syndromes(idxs):
        syn = {}
        for v in variants:
            bits = []
            for j in idxs:
                col = [outs[u][j] for u in variants]
                maj = max(set(col), key=col.count)
                bits.append(1 if outs[v][j] == maj else 0)
            syn[v] = tuple(bits)
        return syn
    cur = 1
    for j in range(len(pool)):
        probed += 1
        trial = chosen + [j]
        if len(set(syndromes(trial).values())) > cur:
            chosen.append(j); cur = len(set(syndromes(trial).values()))
            if cur == len(variants): break
    syn = syndromes(chosen)
    uniq = len(set(syn.values()))
    return {"fam": fam, "faults": len(variants), "checks": len(chosen), "probed": probed,
            "unique": uniq, "full": uniq == len(variants)}

if __name__ == "__main__":
    fams = ["csv","graph","interval","money","sm","stack","unit"]
    print(f"{'family':9} {'faults':6} {'checks':6} {'probes':6} {'unique':6} {'locating-code?'}")
    rows=[]
    for fam in fams:
        try:
            r = run_family(fam)
        except Exception as e:
            r = {"fam":fam,"err":str(e)[:40]}
        if r and "err" not in r:
            rows.append(r)
            print(f"{r['fam']:9} {r['faults']:6} {r['checks']:6} {r['probed']:6} {r['unique']:6} "
                  f"{'YES' if r['full'] else 'partial '+str(r['unique'])+'/'+str(r['faults'])}")
        else:
            print(f"{fam:9} (skip: {r.get('err','no seeds/variants') if r else 'none'})")
    if rows:
        full = sum(1 for r in rows if r['full'])
        print(f"\nfamilies auto-resolved to a FULL locating code: {full}/{len(rows)}")
        print(f"mean checks minted: {statistics.mean(r['checks'] for r in rows):.1f}  "
              f"mean probes: {statistics.mean(r['probed'] for r in rows):.0f}")

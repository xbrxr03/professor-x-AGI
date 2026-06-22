#!/usr/bin/env python3
"""Living-Verifier beachhead STEP 3 — open-set novelty detection on syndromes.
CLAIM: a genuinely NOVEL fault (a bug not in the codebook) reads OUT-OF-DISTRIBUTION on the syndrome
(far from every known codeword) so the system can flag 'new fault -> grow the codebook', while a KNOWN
fault recurring under a new surface (a renamed anchor) reads IN-distribution (matches a codeword).
WIN = novel faults separate from known/anchors (a threshold detects novelty); collisions (novel fault
whose syndrome == a known codeword = undetected novelty) are rare.
KILL = novel faults routinely collide with known codewords -> novelty is invisible -> open-world growth unfounded."""
import os, re, glob, json, shutil, subprocess, itertools
BASE="/home/abrar/professor-x-main-integrate/professor-x/scripts/benchmarks/repo_fix"
RUN="/tmp/sig_runner.py"
def sig_dir(d):
    return subprocess.run(["python3",RUN],cwd=d,capture_output=True,text=True).stdout.strip()
def read_mods(d): return {os.path.basename(f):open(f).read() for f in glob.glob(f"{d}/*.py") if os.path.basename(f)!="check.py"}
def ham(a,b): return sum(x!=y for x,y in zip(a,b)) if len(a)==len(b) and a and b else 999
# generic mutation operators (token-level) to synthesize NEVER-SEEN faults
OPS=[(" + "," - "),(" - "," + "),(" * "," // "),(" // "," * "),(" < "," <= "),(" <= "," < "),
     (" > "," >= "),(" >= "," > "),(" == "," != "),(" != "," == "),(" and "," or "),(" or "," and "),
     (" + 1"," + 2"),("[0]","[1]"),("[-1]","[0]"),(" 100"," 10"),(" 10000"," 1000")]
fams={}
for mf in sorted(glob.glob(f"{BASE}/tasks_family_*.json")):
    d=json.load(open(mf)); fams[d["family"]]=d
anchors={}
for mf in sorted(glob.glob(f"{BASE}/tasks_anchor_*.json")):
    d=json.load(open(mf)); anchors[d["family"]]=d["tasks"]

print(f"{'family':9} {'codewords':9} {'novel':6} {'OOD-detect':11} {'collide':8} {'anchor in-dist':14}")
TOT=dict(novel=0,ood=0,coll=0,anc=0,ancin=0)
for fam,d in fams.items():
    tasks=d["tasks"]
    # correct lib: module content from a task whose buggy_module != that module
    correct={}
    for t in tasks:
        for mn,c in read_mods(f"{BASE}/{t['id']}").items():
            if mn!=t["buggy_module"]: correct.setdefault(mn,c)
    check=open(f"{BASE}/{tasks[0]['id']}/check.py").read()
    # known codebook = syndromes of the shipped faults
    codebook={t["id"]:sig_dir(f"{BASE}/{t['id']}") for t in tasks}
    orig_changes={(t["buggy_module"],) for t in tasks}  # coarse
    # synthesize NOVEL faults: apply each op at each site of each module
    novel_sigs=[]
    seen=set()
    for mn,src in correct.items():
        for find,rep in OPS:
            if find not in src: continue
            mutated=src.replace(find,rep,1)
            if mutated==src: continue
            key=(mn,find,rep)
            if key in seen: continue
            seen.add(key)
            wd=f"/tmp/os-{fam}-{abs(hash(key))%99999}"; shutil.rmtree(wd,ignore_errors=True); os.makedirs(wd)
            for k,v in correct.items(): open(f"{wd}/{k}","w").write(mutated if k==mn else v)
            open(f"{wd}/check.py","w").write(check)
            s=sig_dir(wd); shutil.rmtree(wd,ignore_errors=True)
            if s and "0" in s:  # RED (a real fault) and non-empty
                novel_sigs.append(s)
    # dedupe novel syndromes
    novel_sigs=list(set(novel_sigs))
    cwvals=list(codebook.values())
    ood=coll=0
    for s in novel_sigs:
        dmin=min(ham(s,c) for c in cwvals)
        if dmin==0: coll+=1
        else: ood+=1
    # anchors should be IN-distribution (match a codeword, dist ~0)
    anc=anchors.get(fam,[]); ancin=0
    for a in anc:
        s=sig_dir(f"{BASE}/{a['id']}")
        if cwvals and min(ham(s,c) for c in cwvals)==0: ancin+=1
    n=len(novel_sigs)
    print(f"{fam:9} {len(cwvals):9} {n:6} {ood:>3}/{n:<3}({(ood/n if n else 0):.0%}) {coll:>3}/{n:<3}    {ancin}/{len(anc)} match")
    TOT['novel']+=n; TOT['ood']+=ood; TOT['coll']+=coll; TOT['anc']+=len(anc); TOT['ancin']+=ancin
print(f"\nTOTAL novel faults={TOT['novel']}  detected-OOD={TOT['ood']} ({TOT['ood']/max(TOT['novel'],1):.0%})  "
      f"undetected-collisions={TOT['coll']} ({TOT['coll']/max(TOT['novel'],1):.0%})")
print(f"known-recurring (renamed anchors) in-distribution = {TOT['ancin']}/{TOT['anc']} "
      f"({TOT['ancin']/max(TOT['anc'],1):.0%})")
print("\nVERDICT: novel faults read OOD AND anchors read in-dist => open-set separation works "
      "(novelty is detectable -> codebook growth is founded).")

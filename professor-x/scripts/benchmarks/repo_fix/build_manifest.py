#!/usr/bin/env python3
"""Recipe Step 7: consolidated tasks_families.json over all families + renamed anchors.
Per task: family_id, shared_api, split (train|anchor_renamed), anchor_of, n_changed_lines.
zpd_band is left null until the pass@k run finishes."""
import os, json, glob, re
BASE="/home/abrar/professor-x-main-integrate/professor-x/scripts/benchmarks/repo_fix"

def read_mods(d):
    return {os.path.basename(f):open(f).read() for f in glob.glob(f"{d}/*.py") if os.path.basename(f)!="check.py"}

def correct_lib(manifest_tasks):
    correct={}
    for t in manifest_tasks:
        for mn,c in read_mods(f"{BASE}/{t['id']}").items():
            if mn!=t.get("buggy_module"): correct.setdefault(mn,c)
    return correct

def n_changed(taskdir, bm, correct):
    mods=read_mods(taskdir); buggy=mods.get(bm,""); corr=correct.get(bm,"")
    setb,setc=set(buggy.splitlines()),set(corr.splitlines())
    return len([l for l in corr.splitlines() if l not in setb])+len([l for l in buggy.splitlines() if l not in setc])

out=[]
for mf in sorted(glob.glob(f"{BASE}/tasks_family_*.json")):
    d=json.load(open(mf)); fam=d["family"]; corr=correct_lib(d["tasks"])
    for t in d["tasks"]:
        out.append(dict(id=t["id"],family_id=fam,split="train",shared_api=d["shared_api"],
                        buggy_module=t["buggy_module"],anchor_of=None,
                        n_changed_lines=n_changed(f"{BASE}/{t['id']}",t["buggy_module"],corr),
                        zpd_band=None,setup=t["setup"],verify_cmd=t["verify_cmd"],expect_exit=t["expect_exit"],
                        description=t["description"]))
# anchors (their correct lib = renamed; reconstruct per anchor manifest)
for mf in sorted(glob.glob(f"{BASE}/tasks_anchor_*.json")):
    d=json.load(open(mf)); fam=d["family"]; corr=correct_lib(d["tasks"])
    for t in d["tasks"]:
        out.append(dict(id=t["id"],family_id=fam,split="anchor_renamed",shared_api=d["shared_api"],
                        buggy_module=t["buggy_module"],anchor_of=t.get("anchor_of"),
                        n_changed_lines=n_changed(f"{BASE}/{t['id']}",t["buggy_module"],corr),
                        zpd_band=None,setup=t["setup"],verify_cmd=t["verify_cmd"],expect_exit=t["expect_exit"],
                        description=t["description"]))

import statistics
ncl=[t["n_changed_lines"] for t in out]
manifest=dict(benchmark="reuse-families",n_families=len({t["family_id"] for t in out}),
              n_tasks=len(out),n_train=sum(t["split"]=="train" for t in out),
              n_anchor=sum(t["split"]=="anchor_renamed" for t in out),
              median_changed_lines=statistics.median(ncl),tasks=out)
json.dump(manifest,open(f"{BASE}/tasks_families.json","w"),indent=2)
print(f"tasks_families.json: {manifest['n_tasks']} tasks "
      f"({manifest['n_train']} train + {manifest['n_anchor']} anchor) "
      f"across {manifest['n_families']} families; median changed lines = {manifest['median_changed_lines']}")
print("changed-line dist:", sorted(ncl))

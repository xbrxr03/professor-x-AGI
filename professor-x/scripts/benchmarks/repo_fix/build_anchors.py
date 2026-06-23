#!/usr/bin/env python3
"""Sealed/renamed-anchor split (recipe Step 6, EvoEval-style).
For each family, reconstruct the correct lib + per-task bug from disk, apply a semantic rename
map (modules + shared-API symbols + key params), re-inject the SAME bug into the renamed lib, and
validate red->green. A renamed anchor that an agent fixes only by reasoning about behavior (not by
matching the original mutation operator) is evidence of REAL generalization.
Anchors: 2 tasks per family -> fam_<name>_anchor_<seq>, manifest tasks_anchor_<name>.json."""
import os, json, re, glob, shutil, subprocess
BASE="/home/abrar/professor-x-main-integrate/professor-x/scripts/benchmarks/repo_fix"

def run_check(d):
    for r,_,_ in os.walk(d):
        if r.endswith("__pycache__"): shutil.rmtree(r,ignore_errors=True)
    return subprocess.run(["python3","check.py"],cwd=d,capture_output=True).returncode

def read_mods(d):
    return {os.path.basename(f):open(f).read() for f in glob.glob(f"{d}/*.py") if os.path.basename(f)!="check.py"}

def rename_text(s, sym_map, mod_map):
    # rename module import names first (length-desc to avoid partial), then symbols
    for old,new in sorted(mod_map.items(),key=lambda kv:-len(kv[0])):
        s=re.sub(rf"\b{re.escape(old)}\b",new,s)            # `from tax import` / module refs
    for old,new in sorted(sym_map.items(),key=lambda kv:-len(kv[0])):
        s=re.sub(rf"\b{re.escape(old)}\b",new,s)
    return s

# per-family: module file renames, symbol renames (api + key params), and 2 anchor source task ids
MAPS={
 "money":(dict(zip(["tax.py","discount.py","pricing.py","cart.py"],["levy.py","markdown.py","quote.py","basket.py"])),
          {"tax":"levy","discount":"markdown","pricing":"quote","cart":"basket","add_tax":"apply_levy","apply_discount":"apply_markdown","price":"quote_one","total":"basket_total","cents":"amount","rate_bps":"bps","pct":"rate"},
          ["fam_money_01","fam_money_03"]),
 "interval":(dict(zip(["intervals.py","schedule.py"],["spans.py","planner.py"])),
          {"intervals":"spans","schedule":"planner","overlaps":"intersects","merge_pair":"fuse","merge_all":"consolidate","covered":"span_total","ivs":"items"},
          ["fam_interval_01","fam_interval_04"]),
 # NOTE: do NOT rename `pop` — it collides with the internal builtin list.pop(self._d.pop()).
 "stack":(dict(zip(["stack.py","parens.py"],["lifo.py","brackets.py"])),
          {"stack":"lifo","parens":"brackets","Stack":"Lifo","push":"add","peek":"top","is_empty":"empty","balanced":"well_formed"},
          ["fam_stack_01","fam_stack_04"]),
 "csv":(dict(zip(["row.py","records.py"],["cells.py","table.py"])),
          {"row":"cells","records":"table","parse_row":"split_cells","to_row":"join_cells","parse":"load","select":"pluck","dump":"render"},
          ["fam_csv_01","fam_csv_03"]),
 "unit":(dict(zip(["length.py","convert.py"],["meters.py","units.py"])),
          {"length":"meters","convert":"units","m_to_cm":"meter_to_centi","cm_to_m":"centi_to_meter","m_to_mm":"meter_to_milli","mm_to_m":"milli_to_meter","scale":"multiply_all"},
          ["fam_unit_01","fam_unit_03"]),
 "sm":(dict(zip(["states.py","machine.py"],["transitions.py","fsm.py"])),
          {"states":"transitions","machine":"fsm","next_state":"step","count_opens":"tally_open","run":"execute","TRANSITIONS":"TABLE","LOCKED":"SHUT","OPEN":"AJAR"},
          ["fam_sm_01","fam_sm_03"]),
 "graph":(dict(zip(["graph.py","traverse.py"],["adjacency.py","walk.py"])),
          {"graph":"adjacency","traverse":"walk","add_edge":"link","neighbors":"adjacent","degree":"valence","reachable":"component","connected":"linked","adj":"g"},
          ["fam_graph_01","fam_graph_03"]),
}

def family_correct(fam, manifest):
    tasks=manifest["tasks"]; correct={}
    for t in tasks:
        for mn,c in read_mods(f"{BASE}/{t['id']}").items():
            if mn!=t.get("buggy_module"): correct.setdefault(mn,c)
    return correct

total=0; made_all={}
for fam,(mod_map,sym_map,anchor_ids) in MAPS.items():
    man=json.load(open(f"{BASE}/tasks_family_{fam}.json"))
    by_id={t["id"]:t for t in man["tasks"]}
    correct=family_correct(fam,man)
    check=open(f"{BASE}/{man['tasks'][0]['id']}/check.py").read()
    rcheck=rename_text(check,sym_map,mod_map)
    made=[]
    for seq,aid in enumerate(anchor_ids,1):
        if aid not in by_id:
            print(f"  [{fam}] anchor src {aid} missing, skip"); continue
        t=by_id[aid]; bm=t["buggy_module"]
        buggy=read_mods(f"{BASE}/{aid}")[bm]
        rbm=rename_text(bm,sym_map,mod_map) if bm in mod_map else mod_map.get(bm,bm)
        # renamed lib (correct) with the renamed buggy module swapped in
        anchor_id=f"fam_{fam}_anchor_{seq}"
        d=f"{BASE}/{anchor_id}"; shutil.rmtree(d,ignore_errors=True); os.makedirs(d)
        for mn,c in correct.items():
            rmn=mod_map.get(mn,mn)
            content=rename_text(buggy if mn==bm else c, sym_map, mod_map)
            open(f"{d}/{rmn}","w").write(content)
        open(f"{d}/check.py","w").write(rcheck)
        buggy_rc=run_check(d)
        # fixed = swap renamed-correct module back in
        vd=f"/tmp/va-{anchor_id}"; shutil.rmtree(vd,ignore_errors=True); shutil.copytree(d,vd)
        open(f"{vd}/{mod_map.get(bm,bm)}","w").write(rename_text(correct[bm],sym_map,mod_map))
        fixed_rc=run_check(vd)
        ok=(buggy_rc==1 and fixed_rc==0)
        print(f"  {anchor_id} (from {aid}, mod {bm}->{mod_map.get(bm,bm)}): buggy={buggy_rc} fixed={fixed_rc} -> {'OK' if ok else 'REJECT'}")
        if ok: made.append((anchor_id,mod_map.get(bm,bm),aid))
        else: shutil.rmtree(d,ignore_errors=True)
    if made:
        out={"family":fam,"split":"anchor_renamed","derived_from":fam,
             "shared_api":[rename_text(s.split('.')[-1],sym_map,mod_map) for s in man.get("shared_api",[])],
             "tasks":[{"id":a,"category":"repo_fix","setup":f"scripts/benchmarks/repo_fix/{a}",
                       "description":f"check.py fails due to a bug in the shared library. Find the root cause across the files and fix it so check.py passes. Do not edit check.py.",
                       "verify_cmd":"python3 check.py","expect_exit":0,"buggy_module":m,"anchor_of":src} for a,m,src in made]}
        json.dump(out,open(f"{BASE}/tasks_anchor_{fam}.json","w"),indent=2)
    print(f"[{fam}] {len(made)} anchors validated -> tasks_anchor_{fam}.json")
    total+=len(made)
print(f"\nTOTAL renamed anchors validated: {total}")

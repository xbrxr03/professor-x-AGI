import os, shutil, subprocess, json
BASE = "/home/abrar/professor-x-main-integrate/professor-x/scripts/benchmarks/repo_fix"

# Correct shared library (the family's shared API: tax<-discount<-pricing<-cart)
LIB = {
"tax.py": "def add_tax(cents, rate_bps):\n    # rate in basis points: 10000 bps = 100%\n    return cents + cents * rate_bps // 10000\n",
"discount.py": "def apply_discount(cents, pct):\n    return cents - cents * pct // 100\n",
"pricing.py": "from tax import add_tax\nfrom discount import apply_discount\n\n\ndef price(cents, rate_bps, pct):\n    # discount first, then tax\n    return add_tax(apply_discount(cents, pct), rate_bps)\n",
"cart.py": "from pricing import price\n\n\ndef total(items, rate_bps, pct):\n    return sum(price(c, rate_bps, pct) for c in items)\n",
}
CHECK = '''import sys
from tax import add_tax
from discount import apply_discount
from pricing import price
from cart import total
try:
    assert add_tax(1000, 0) == 1000
    assert apply_discount(1000, 0) == 1000
    assert price(1000, 0, 0) == 1000
    assert apply_discount(1000, 10) == 900, ("discount", apply_discount(1000, 10))
    assert add_tax(1000, 1000) == 1100, ("tax", add_tax(1000, 1000))
    assert price(1000, 1000, 10) == 990, ("price", price(1000, 1000, 10))
    assert total([1000, 2000], 0, 0) == 3000, ("cart-sum", total([1000, 2000], 0, 0))
    assert total([1000], 1000, 10) == 990
    print("ok"); sys.exit(0)
except AssertionError as e:
    print("FAIL", e); sys.exit(1)
'''
# Each bug: (id, module, find, replace) — injected into a SHARED helper so it propagates.
BUGS = [
 ("fam_money_01","tax.py","// 10000","// 1000"),                       # 10x tax
 ("fam_money_02","discount.py","cents * pct // 100","cents * pct // 1000"),  # weak discount
 ("fam_money_03","pricing.py","add_tax(apply_discount(cents, pct), rate_bps)","apply_discount(cents, pct)"),  # forgets tax
 ("fam_money_04","tax.py","return cents + cents * rate_bps // 10000","return cents * rate_bps // 10000"),     # drops base
 ("fam_money_05","discount.py","return cents - cents * pct // 100","return cents + cents * pct // 100"),       # adds instead of subtracts
 ("fam_money_06","cart.py","return sum(price(c, rate_bps, pct) for c in items)","return price(items[0], rate_bps, pct)"),  # only first item
]

def run_check(d):
    return subprocess.run(["python3","check.py"],cwd=d,capture_output=True).returncode

# sanity: correct lib must pass
tmp="/tmp/fam_sanity"; shutil.rmtree(tmp,ignore_errors=True); os.makedirs(tmp)
for k,v in LIB.items(): open(f"{tmp}/{k}","w").write(v)
open(f"{tmp}/check.py","w").write(CHECK)
assert run_check(tmp)==0, "correct lib FAILS its own check — fix the check"
print("correct-lib sanity: PASS (exit 0)")

made=[]
for bid,mod,find,rep in BUGS:
    assert find in LIB[mod], f"{bid}: pattern not in {mod}"
    d=f"{BASE}/{bid}"; shutil.rmtree(d,ignore_errors=True); os.makedirs(d)
    for k,v in LIB.items():
        out = v.replace(find,rep) if k==mod else v
        open(f"{d}/{k}","w").write(out)
    open(f"{d}/check.py","w").write(CHECK)
    buggy=run_check(d)
    # validate fix: restore correct module -> green
    vd=f"/tmp/v-{bid}"; shutil.rmtree(vd,ignore_errors=True); shutil.copytree(d,vd)
    open(f"{vd}/{mod}","w").write(LIB[mod])
    fixed=run_check(vd)
    ok = (buggy==1 and fixed==0)
    print(f"{bid}: buggy={buggy} fixed={fixed} -> {'OK' if ok else 'REJECT'}")
    if ok: made.append((bid,mod))
    else: shutil.rmtree(d,ignore_errors=True)

# register family manifest
fam={"family":"money","shared_api":["tax.add_tax","discount.apply_discount","pricing.price","cart.total"],
     "tasks":[{"id":bid,"category":"repo_fix","setup":f"scripts/benchmarks/repo_fix/{bid}",
               "description":"check.py fails due to a bug in the shared pricing library (tax/discount/pricing/cart). Find the root cause across the files and fix it so check.py passes. Do not edit check.py.",
               "verify_cmd":"python3 check.py","expect_exit":0,"buggy_module":mod} for bid,mod in made]}
json.dump(fam,open(f"{BASE}/tasks_family_money.json","w"),indent=2)
print(f"\nFAMILY money: {len(made)}/{len(BUGS)} validated red->green, sharing 4-module API -> tasks_family_money.json")

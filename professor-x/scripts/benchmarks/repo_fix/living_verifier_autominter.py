#!/usr/bin/env python3
"""LIVING VERIFIER — automated codebook growth via DIFFERENTIAL TESTING (CPU, big-swing experiment).

Claim under test (the unproven open-world pillar): when two faults COLLIDE (alias to the same
failure-syndrome), the verifier can AUTOMATICALLY mint a new discriminating check — no hand-authored
metamorphic property — by differential testing: search random inputs for one whose correct-output
assert produces DISTINCT pass/fail across the colliding buggy implementations. That asserted input is
a new codeword that splits the collision. This is the 'code grows to track the channel' step of
self-improvement-as-channel-code-co-design, done automatically.

Decisive case: the real interval collision (fam_interval_04 sign-flip vs fam_interval_05 no-merge),
which tonight required HAND-authored checks (non-negativity + overlap-idempotence). Can a blind
differential search rediscover a separator, and how few random inputs does it need (rateless efficiency)?
"""
import random

# --- the family library (correct merge_all; the 3 covered variants from the real fixtures) ---
def overlaps(a, b): return a[0] < b[1] and b[0] < a[1]
def merge_pair(a, b): return (min(a[0], b[0]), max(a[1], b[1]))
def merge_all(ivs):
    ivs = sorted(ivs)
    if not ivs: return []
    out = [ivs[0]]
    for iv in ivs[1:]:
        if overlaps(out[-1], iv) or out[-1][1] == iv[0]:
            out[-1] = merge_pair(out[-1], iv)
        else:
            out.append(iv)
    return out

def covered_correct(ivs): return sum(e - s for s, e in merge_all(ivs))
def covered_bug04(ivs):   return sum(s - e for s, e in merge_all(ivs))   # sign flip
def covered_bug05(ivs):   return sum(e - s for s, e in ivs)              # dropped merge_all

IMPLS = {"correct": covered_correct, "bug04_sign": covered_bug04, "bug05_nomerge": covered_bug05}

def rand_input(rng, n_max=4, lo=0, hi=10):
    n = rng.randint(1, n_max)
    out = []
    for _ in range(n):
        s = rng.randint(lo, hi - 1); e = rng.randint(s + 1, hi)
        out.append((s, e))
    return out

def pass_vector(ivs):
    """For a candidate check 'covered(ivs) == correct', the pass/fail of each buggy impl."""
    target = covered_correct(ivs)
    return tuple(1 if f(ivs) == target else 0 for name, f in IMPLS.items() if name != "correct")

def auto_mint(seed=0, budget=2000):
    """Differential search: find minimal set of asserted inputs whose joint pass-vectors give every
    buggy impl a UNIQUE syndrome (i.e., separate the collision). Returns (minted_inputs, n_probed)."""
    rng = random.Random(seed)
    bugs = [n for n in IMPLS if n != "correct"]
    minted = []
    # current syndrome of each bug under the minted checks (start: empty -> all identical)
    def syndromes(checks):
        syn = {b: [] for b in bugs}
        for ivs in checks:
            target = covered_correct(ivs)
            for bi, b in enumerate(bugs):
                syn[b].append(1 if IMPLS[b](ivs) == target else 0)
        return {b: tuple(v) for b, v in syn.items()}
    probed = 0
    for _ in range(budget):
        ivs = rand_input(rng); probed += 1
        # does adding this check INCREASE the number of distinct syndromes?
        trial = minted + [ivs]
        syn = syndromes(trial)
        if len(set(syn.values())) > len(set(syndromes(minted).values())):
            minted.append(ivs)
            if len(set(syn.values())) == len(bugs):   # all unique -> collision fully resolved
                return minted, probed, syn
    return minted, probed, syndromes(minted)

if __name__ == "__main__":
    print("=== BEFORE (the collision): syndrome of each buggy impl under the ORIGINAL battery is identical ===")
    print("  fam_interval_04 and fam_interval_05 both -> 1111100 (aliased)\n")
    print("=== AUTOMATED differential minting (10 seeds) ===")
    import statistics
    n_checks, n_probed, wins = [], [], 0
    for seed in range(10):
        minted, probed, syn = auto_mint(seed=seed)
        ok = len(set(syn.values())) == 2  # 2 bugs separated
        wins += ok
        n_checks.append(len(minted)); n_probed.append(probed)
        if seed < 3:
            print(f"  seed {seed}: minted {len(minted)} check(s) in {probed} probes -> "
                  f"separated={ok}; syndromes={syn}")
            for ivs in minted:
                print(f"      new codeword: assert covered({ivs}) == {covered_correct(ivs)}  "
                      f"[04={covered_bug04(ivs)} 05={covered_bug05(ivs)} correct={covered_correct(ivs)}]")
    print(f"\n=== RESULT over 10 seeds ===")
    print(f"  collision auto-resolved: {wins}/10 seeds")
    print(f"  checks minted to separate: mean {statistics.mean(n_checks):.1f} (min {min(n_checks)}, max {max(n_checks)})")
    print(f"  random probes needed (rateless cost): mean {statistics.mean(n_probed):.0f} (min {min(n_probed)})")
    print("\n  red->green check: every minted assert holds on the CORRECT impl by construction")
    print("  (the asserted value IS covered_correct(ivs)), so it never breaks a correct solution.")

import sys
from levy import apply_levy
from markdown import apply_markdown
from quote import quote_one
from basket import basket_total
try:
    assert apply_levy(1000, 0) == 1000
    assert apply_markdown(1000, 0) == 1000
    assert quote_one(1000, 0, 0) == 1000
    assert apply_markdown(1000, 10) == 900, ("markdown", apply_markdown(1000, 10))
    assert apply_levy(1000, 1000) == 1100, ("levy", apply_levy(1000, 1000))
    assert quote_one(1000, 1000, 10) == 990, ("quote_one", quote_one(1000, 1000, 10))
    assert basket_total([1000, 2000], 0, 0) == 3000, ("basket-sum", basket_total([1000, 2000], 0, 0))
    assert basket_total([1000], 1000, 10) == 990
    print("ok"); sys.exit(0)
except AssertionError as e:
    print("FAIL", e); sys.exit(1)

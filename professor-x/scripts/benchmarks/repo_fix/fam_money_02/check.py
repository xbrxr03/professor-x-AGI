import sys
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

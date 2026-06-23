import sys
from order import Order
try:
    # gold: subtotal 100 -> 80 -> *1.10 = 88.0
    assert Order([(50,1),(25,2)], "gold").total() == 88.0
    assert Order([(10,1)], "none").total() == 11.0
    print('ok'); sys.exit(0)
except AssertionError:
    print('FAIL'); sys.exit(1)

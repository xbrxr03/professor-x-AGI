import sys
from semver import compare
try:
    assert compare("1.2.0", "1.10.0") == -1, "1.2.0 < 1.10.0 numerically"
    assert compare("1.10.0", "1.2.0") == 1
    assert compare("2.0.0", "1.99.99") == 1
    assert compare("1.0.0", "1.0.0") == 0
    print("ok"); sys.exit(0)
except AssertionError as e:
    print("FAIL", e); sys.exit(1)

import sys

from release import latest
from semver import cmp_version

try:
    assert cmp_version("1.0", "1.0.1") == -1, "1.0 < 1.0.1 (extra patch component)"
    assert cmp_version("1.0.1", "1.0") == 1, "1.0.1 > 1.0"
    assert cmp_version("1.2.0", "1.10.0") == -1, "numeric, not lexical, compare"
    assert latest(["1.0.0", "1.0.1", "1.0"]) == "1.0.1"
    print("ok")
    sys.exit(0)
except AssertionError as e:
    print("FAIL", e)
    sys.exit(1)

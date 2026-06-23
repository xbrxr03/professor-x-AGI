import sys
from auth import can_access
try:
    assert bool(can_access({"active": True, "admin": True, "owner": False})) is True
    assert bool(can_access({"active": True, "admin": False, "owner": True})) is True
    # inactive owner must be denied (this is the case the precedence bug gets wrong)
    assert bool(can_access({"active": False, "admin": False, "owner": True})) is False
    assert bool(can_access({"active": True, "admin": False, "owner": False})) is False
    print("ok"); sys.exit(0)
except AssertionError:
    print("FAIL"); sys.exit(1)

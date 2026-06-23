import sys

from config import resolve

try:
    # user sets timeout, env sets retries; verbose should fall through to the default
    r = resolve({"timeout": 10}, {"retries": 5})
    assert r.get("timeout") == 10, r
    assert r.get("retries") == 5, r
    assert r.get("verbose") is False, r  # default must survive the precedence chain
    print("ok")
    sys.exit(0)
except AssertionError as e:
    print("FAIL", e)
    sys.exit(1)

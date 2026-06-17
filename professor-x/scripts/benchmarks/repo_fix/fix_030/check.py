import sys
from dedup import dedup
try:
    assert dedup([3,1,3,2,1])==[3,1,2]
    assert dedup([])==[]
    print("ok");sys.exit(0)
except AssertionError:
    print("FAIL");sys.exit(1)

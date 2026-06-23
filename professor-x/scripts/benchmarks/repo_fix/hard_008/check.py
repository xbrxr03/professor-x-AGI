import sys
from gate import can_edit
try:
    assert bool(can_edit({'active': True, 'admin': True})) is True
    assert bool(can_edit({'active': True, 'owner': True})) is True
    assert bool(can_edit({'active': False, 'owner': True})) is False
    assert bool(can_edit({'active': True})) is False
    print('ok'); sys.exit(0)
except AssertionError:
    print('FAIL'); sys.exit(1)

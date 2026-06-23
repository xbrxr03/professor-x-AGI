import sys
from sched import next_job
try:
    assert next_job([(5,'low'),(1,'urgent'),(3,'mid')])=='urgent'
    print('ok');sys.exit(0)
except AssertionError:
    print('FAIL');sys.exit(1)

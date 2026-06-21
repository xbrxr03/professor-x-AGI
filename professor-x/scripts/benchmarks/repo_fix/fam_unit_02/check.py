import sys
from length import m_to_cm, cm_to_m
from convert import m_to_mm, mm_to_m, scale
try:
    assert m_to_cm(2) == 200
    assert cm_to_m(200) == 2
    assert m_to_mm(2) == 2000, m_to_mm(2)
    assert mm_to_m(2000) == 2, mm_to_m(2000)
    assert abs(mm_to_m(m_to_mm(3.5)) - 3.5) < 1e-9  # round-trip
    assert scale([1,2,3], 10) == [10,20,30]
    print("ok"); sys.exit(0)
except AssertionError as e:
    print("FAIL", e); sys.exit(1)

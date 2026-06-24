import sys
from meters import meter_to_centi, centi_to_meter
from units import meter_to_milli, milli_to_meter, multiply_all
try:
    assert meter_to_centi(2) == 200
    assert centi_to_meter(200) == 2
    assert meter_to_milli(2) == 2000, meter_to_milli(2)
    assert milli_to_meter(2000) == 2, milli_to_meter(2000)
    assert abs(milli_to_meter(meter_to_milli(3.5)) - 3.5) < 1e-9  # round-trip
    assert multiply_all([1,2,3], 10) == [10,20,30]
    print("ok"); sys.exit(0)
except AssertionError as e:
    print("FAIL", e); sys.exit(1)

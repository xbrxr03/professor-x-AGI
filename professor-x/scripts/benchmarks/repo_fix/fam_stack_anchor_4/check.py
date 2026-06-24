import sys
from lifo import Lifo
from brackets import well_formed
try:
    st = Lifo()
    assert st.empty() is True
    st.add(1); st.add(2)
    assert st.top() == 2, st.top()
    assert st.pop() == 2
    assert st.pop() == 1
    assert st.empty() is True
    assert well_formed("([]{})") is True
    assert well_formed("([)]") is False
    assert well_formed("(((") is False
    assert well_formed("") is True
    print("ok"); sys.exit(0)
except AssertionError as e:
    print("FAIL", e); sys.exit(1)

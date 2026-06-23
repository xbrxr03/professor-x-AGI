import sys
from stack import Stack
from parens import balanced
try:
    st = Stack()
    assert st.is_empty() is True
    st.push(1); st.push(2)
    assert st.peek() == 2, st.peek()
    assert st.pop() == 2
    assert st.pop() == 1
    assert st.is_empty() is True
    assert balanced("([]{})") is True
    assert balanced("([)]") is False
    assert balanced("(((") is False
    assert balanced("") is True
    print("ok"); sys.exit(0)
except AssertionError as e:
    print("FAIL", e); sys.exit(1)

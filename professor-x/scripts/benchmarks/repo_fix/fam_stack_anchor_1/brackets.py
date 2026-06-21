from lifo import Lifo


def well_formed(s):
    st = Lifo()
    pairs = {')': '(', ']': '[', '}': '{'}
    for c in s:
        if c in '([{':
            st.add(c)
        elif c in ')]}':
            if st.empty() or st.pop() != pairs[c]:
                return False
    return st.empty()

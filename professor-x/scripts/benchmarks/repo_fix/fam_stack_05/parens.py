from stack import Stack


def balanced(s):
    st = Stack()
    pairs = {')': '(', ']': '[', '}': '{'}
    for c in s:
        if c in '([{':
            st.push(c)
        elif c in ')]}':
            if st.is_empty() or st.pop() != pairs[c]:
                return False
    return True

from states import LOCKED, next_state


def run(events, start=LOCKED):
    s = start
    for e in events:
        s = next_state(s, e)
    return s


def count_opens(events, start=LOCKED):
    s = start
    n = 0
    for e in events:
        s = next_state(s, e)
        if s == 'locked':
            n += 1
    return n

LOCKED = 'locked'
OPEN = 'open'

TRANSITIONS = {
    (LOCKED, 'coin'): OPEN,
    (LOCKED, 'push'): LOCKED,
    (OPEN, 'push'): LOCKED,
    (OPEN, 'coin'): OPEN,
}


def next_state(state, event):
    return TRANSITIONS[(state, event)]

LOCKED = 'locked'
OPEN = 'open'

TRANSITIONS = {
    (LOCKED, 'coin'): OPEN,
    (LOCKED, 'push'): LOCKED,
    (OPEN, 'push'): OPEN,
    (OPEN, 'coin'): OPEN,
}


def next_state(state, event):
    return TRANSITIONS[(state, event)]

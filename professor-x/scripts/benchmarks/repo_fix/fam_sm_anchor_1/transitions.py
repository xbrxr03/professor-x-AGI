SHUT = 'locked'
AJAR = 'open'

TABLE = {
    (SHUT, 'coin'): SHUT,
    (SHUT, 'push'): SHUT,
    (AJAR, 'push'): SHUT,
    (AJAR, 'coin'): AJAR,
}


def step(state, event):
    return TABLE[(state, event)]

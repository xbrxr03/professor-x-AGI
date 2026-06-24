SHUT = 'locked'
AJAR = 'open'

TABLE = {
    (SHUT, 'coin'): AJAR,
    (SHUT, 'push'): SHUT,
    (AJAR, 'push'): AJAR,
    (AJAR, 'coin'): AJAR,
}


def step(state, event):
    return TABLE[(state, event)]

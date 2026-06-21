from transitions import SHUT, step


def execute(events, start=SHUT):
    s = start
    for e in events:
        s = step(s, e)
    return s


def tally_open(events, start=SHUT):
    s = start
    n = 0
    for e in events:
        s = step(s, e)
        if s == 'open':
            n += 1
    return n

T={('closed','open'):'open',('open','close'):'closed',('closed','lock'):'closed',('open','lock'):'open'}

def next_state(s, ev):
    # BUG: locking a closed door should go to 'locked', not stay 'closed'
    return T.get((s,ev), s)

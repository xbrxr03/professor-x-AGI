import transitions

class Door:
    def __init__(self): self.state='closed'
    def do(self, ev): self.state=transitions.next_state(self.state, ev)

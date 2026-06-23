class PQ:
    def __init__(self): self.items=[]
    def push(self,p,v): self.items.append((p,v))
    def pop(self):
        # BUG: returns highest priority number; lowest number = most urgent
        self.items.sort()
        return self.items[-1][1]

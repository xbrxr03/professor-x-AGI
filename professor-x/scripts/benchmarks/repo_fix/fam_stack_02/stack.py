class Stack:
    def __init__(self):
        self._d = []
    def push(self, x):
        self._d.append(x)
    def pop(self):
        return self._d.pop()
    def peek(self):
        return self._d[0]
    def is_empty(self):
        return len(self._d) == 0

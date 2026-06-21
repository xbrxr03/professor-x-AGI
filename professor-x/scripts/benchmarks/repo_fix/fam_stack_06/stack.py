class Stack:
    def __init__(self):
        self._d = []
    def push(self, x):
        self._d.insert(0, x)
    def pop(self):
        return self._d.pop()
    def peek(self):
        return self._d[-1]
    def is_empty(self):
        return len(self._d) == 0

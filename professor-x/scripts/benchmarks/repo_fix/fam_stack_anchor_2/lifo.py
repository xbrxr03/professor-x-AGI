class Lifo:
    def __init__(self):
        self._d = []
    def add(self, x):
        self._d.append(x)
    def pop(self):
        return self._d.pop()
    def top(self):
        return self._d[0]
    def empty(self):
        return len(self._d) == 0

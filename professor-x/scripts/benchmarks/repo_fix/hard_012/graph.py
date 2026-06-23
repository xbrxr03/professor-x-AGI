def bfs(g, start):
    # BUG: doesn't track visited -> revisits, returns duplicates
    out=[]; q=[start]
    while q:
        n=q.pop(0); out.append(n)
        q.extend(g.get(n, []))
    return out

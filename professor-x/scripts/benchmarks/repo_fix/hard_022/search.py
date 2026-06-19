import tokenize_mod as tk

def matches(doc, query):
    return set(tk.tokens(query)).issubset(set(tk.tokens(doc)))

def tokens(s):
    """Split text into lowercased word tokens, treating commas as separators."""
    return [t.lower() for t in s.split() if t]

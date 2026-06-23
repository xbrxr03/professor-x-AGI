def normalize(s):
    # Intended: lowercase, and turn every non-alphanumeric char into a space
    # (so the caller can split on whitespace). BUG: only lowercases.
    return s.lower()

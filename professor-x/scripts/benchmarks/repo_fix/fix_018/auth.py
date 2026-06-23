def can_access(user):
    # Rule: a user may access only if they are active AND (admin OR owner).
    # BUG: operator precedence makes this (active and admin) or owner, so an
    # inactive owner is wrongly granted access.
    return user.get("active") and user.get("admin") or user.get("owner")

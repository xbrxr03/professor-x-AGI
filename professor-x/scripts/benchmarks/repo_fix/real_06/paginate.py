def page(items, page_num, per_page):
    """Return the items on a 1-indexed page."""
    start = page_num * per_page          # BUG: 1-indexed -> should be (page_num - 1) * per_page
    return items[start:start + per_page]

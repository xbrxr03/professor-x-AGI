def page(items, page_num, per_page):
    # page_num is 1-based: page 1 is the first per_page items.
    # BUG: off by one — page 1 skips the first page instead of starting at 0.
    start = page_num * per_page
    return items[start:start + per_page]

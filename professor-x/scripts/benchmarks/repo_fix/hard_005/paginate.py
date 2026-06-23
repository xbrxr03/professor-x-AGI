from limits import PAGE_SIZE

def page(items, page_num):
    start = page_num * PAGE_SIZE
    return items[start:start + PAGE_SIZE]

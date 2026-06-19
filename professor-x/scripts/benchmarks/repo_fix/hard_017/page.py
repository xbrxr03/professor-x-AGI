def count(total, per):
    # BUG: floor division drops the partial last page
    return total // per

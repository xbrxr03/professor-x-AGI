def overlaps(a, b):
    # half-open [start,end): touching at a boundary is NOT a conflict.
    # BUG: uses <= so [1,2) and [2,3) wrongly conflict
    return a[0] <= b[1] and b[0] <= a[1]

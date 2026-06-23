def days_between(a, b):
    # a,b are (y,m,d) on the same month for simplicity; BUG: inclusive off-by-one
    return b[2]-a[2]+1

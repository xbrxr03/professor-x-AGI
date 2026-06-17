def median(xs):
    n=len(xs); m=n//2
    if n%2: return xs[m]
    return (xs[m-1]+xs[m])/2

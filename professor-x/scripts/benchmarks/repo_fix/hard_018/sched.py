import pq

def next_job(jobs):
    q=pq.PQ()
    for p,name in jobs: q.push(p,name)
    return q.pop()

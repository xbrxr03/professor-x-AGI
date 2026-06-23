import retry

def call(fn, max_attempts):
    return retry.run(fn, max_attempts)

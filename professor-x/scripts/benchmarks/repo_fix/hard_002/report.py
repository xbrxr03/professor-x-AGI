import textproc

def summarize(text):
    cleaned = textproc.clean(text)
    return {"words": textproc.wordcount(cleaned), "chars": len(cleaned)}

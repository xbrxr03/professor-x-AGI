from textutil import normalize


def slugify(title):
    # Join the normalized words with single hyphens.
    return "-".join(normalize(title).split())

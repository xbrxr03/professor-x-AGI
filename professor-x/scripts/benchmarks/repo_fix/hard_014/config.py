import ini

def get(text, key):
    return ini.parse(text).get(key)

import validators

def register(email):
    if not validators.is_email(email):
        return 'invalid'
    return 'ok'

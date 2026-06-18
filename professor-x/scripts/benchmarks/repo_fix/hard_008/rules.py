def allowed(user):
    return user.get('active') and user.get('admin') or user.get('owner')

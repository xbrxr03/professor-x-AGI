import template

def render(tpl, name):
    return template.fill(tpl, '{name}', name)

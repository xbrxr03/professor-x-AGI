from defaults import DEFAULTS
from merge import apply_overrides


def resolve(user, env):
    """Resolve config with precedence env > user > defaults."""
    step1 = apply_overrides(DEFAULTS, user)
    step2 = apply_overrides(user, env)
    return step2

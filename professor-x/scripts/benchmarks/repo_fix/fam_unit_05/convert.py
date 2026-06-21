from length import m_to_cm, cm_to_m


def m_to_mm(m):
    return m_to_cm(m) * 10


def mm_to_m(mm):
    return cm_to_m(mm / 10)


def scale(values, factor):
    return [v + factor for v in values]

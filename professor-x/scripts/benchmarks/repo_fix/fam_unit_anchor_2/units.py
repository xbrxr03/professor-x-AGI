from meters import meter_to_centi, centi_to_meter


def meter_to_milli(m):
    return meter_to_centi(m) * 10


def milli_to_meter(mm):
    return centi_to_meter(mm / 10)


def multiply_all(values, factor):
    return [v * factor for v in values]

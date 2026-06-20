from roman import to_roman


def label(nums):
    """Render each integer as a Roman numeral."""
    return [to_roman(n) for n in nums]

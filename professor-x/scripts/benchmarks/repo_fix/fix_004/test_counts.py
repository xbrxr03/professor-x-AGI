from counts import get_count

def test_get_count():
    assert get_count({"a": 5}, "a") == 5
    assert get_count({}, "missing") == 0

import sys
from slugify import slugify
try:
    assert slugify("Hello, World!") == "hello-world"
    assert slugify("  Foo   Bar  ") == "foo-bar"
    assert slugify("a.b-c") == "a-b-c"
    print("ok"); sys.exit(0)
except AssertionError:
    print("FAIL"); sys.exit(1)

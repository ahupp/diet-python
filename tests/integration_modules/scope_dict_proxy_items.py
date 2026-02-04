
def get_globals_items():
    return dict(globals().items())

# diet-python: validate

module = __import__("sys").modules[__name__]
items = module.get_globals_items()
assert "__name__" in items

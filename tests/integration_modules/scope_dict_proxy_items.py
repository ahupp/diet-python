
def get_globals_items():
    return dict(globals().items())

# diet-python: validate

def validate_module(module):
    items = module.get_globals_items()
    assert "__name__" in items


def loop_compare(a, b):
    while True:
        if a == b:
            return True
        return False

# diet-python: validate

def validate_module(module):
    assert module.loop_compare(1, 1) is True
    assert module.loop_compare(1, 2) is False


class Example:
    @property
    def value(self):
        return 42


# diet-python: validate

def validate_module(module):
    assert module.Example().value == 42

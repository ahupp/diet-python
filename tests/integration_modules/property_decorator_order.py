class Example:
    def __init__(self):
        self._value = 0

    @property
    def value(self):
        return self._value

    @value.setter
    def value(self, value):
        self._value = value

# diet-python: validate

def validate_module(module):

    instance = module.Example()
    instance.value = 7
    assert instance.value == 7

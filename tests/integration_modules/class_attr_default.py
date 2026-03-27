class Example:
    SENTINEL = object()

    def method(self, value=SENTINEL):
        return value

# diet-python: validate

def validate_module(module):

    instance = module.Example()
    assert instance.method() is module.Example.SENTINEL

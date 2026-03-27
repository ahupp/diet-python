

def make():
    sentinel = object()
    class C:
        def method(self, value=sentinel):
            return value is sentinel
    return C()


def run():
    return make().method()


# diet-python: validate

def validate_module(module):
    assert module.run() is True

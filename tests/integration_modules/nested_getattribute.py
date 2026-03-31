class Container:
    def probe(self):
        class A:
            token = 1

        class B:
            def __getattribute__(self, attr):
                a = A()
                return getattr(a, attr)

        return B().missing


# diet-python: validate

def validate_module(module):
    import pytest

    with pytest.raises(AttributeError, match="'A' object has no attribute 'missing'"):
        module.Container().probe()

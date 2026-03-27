class Base:
    @property
    def value(self):
        """base doc"""
        return 1


def copydoc(func):
    func.__doc__ = getattr(Base, func.__name__).__doc__
    return func


class Derived(Base):
    @property
    @copydoc
    def value(self):
        return 2

# diet-python: validate

def validate_module(module):

    assert module.Derived.value.__doc__ == "base doc"

class Meta(type):
    pass


class X(metaclass=Meta):
    pass


class Y(X):
    pass

# diet-python: validate

def validate_module(module):
    assert getattr(module.Meta, "__annotate__", None) is None
    assert getattr(module.X, "__annotate__", None) is None
    assert getattr(module.Y, "__annotate__", None) is None

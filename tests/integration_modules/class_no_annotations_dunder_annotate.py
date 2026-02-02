class Meta(type):
    pass


class X(metaclass=Meta):
    pass


class Y(X):
    pass

# diet-python: validate

assert getattr(Meta, "__annotate__", None) is None
assert getattr(X, "__annotate__", None) is None
assert getattr(Y, "__annotate__", None) is None

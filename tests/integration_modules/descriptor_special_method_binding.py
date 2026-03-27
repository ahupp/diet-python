class BindingDescriptor:
    def __init__(self, label):
        self.label = label

    def __get__(self, obj, owner):
        def bound(other):
            return (self.label, obj.value, getattr(other, "value", other))
        return bound

    def __call__(self, *args, **kwargs):
        raise AssertionError("unbound descriptor called")


class C:
    __add__ = BindingDescriptor("add")
    __eq__ = BindingDescriptor("eq")

    def __init__(self, value):
        self.value = value


lhs = C(10)
rhs = C(3)
add_result = lhs + 5
eq_result = lhs == rhs

# diet-python: validate

def validate_module(module):
    assert module.add_result == ("add", 10, 5)
    assert module.eq_result == ("eq", 10, 3)

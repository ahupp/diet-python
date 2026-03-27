def classcell_values():
    class C:
        def method(self):
            super()
            return __class__
        items = [(lambda: i) for i in range(5)]
        y = [x() for x in items]

    return C.y, C().method(), C

# diet-python: validate

def validate_module(module):

    values, method_class, cls = module.classcell_values()
    assert values == [4, 4, 4, 4, 4]
    assert method_class is cls

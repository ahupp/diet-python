
class Field:
    def __init__(self, init, kw_only):
        self.init = init
        self.kw_only = kw_only


def run(fields):
    return tuple(f for f in fields if f.init and not f.kw_only)


# diet-python: validate

def validate_module(module):
    fields = [module.Field(True, False), module.Field(True, True)]

    assert module.run(fields) == (fields[0],)

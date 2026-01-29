def fields_in_init_order(fields):
    return (
        tuple(f for f in fields if f.init and not f.kw_only),
        tuple(f for f in fields if f.init and f.kw_only),
    )


class Field:
    def __init__(self, init, kw_only):
        self.init = init
        self.kw_only = kw_only


# diet-python: validate

module = __import__("sys").modules[__name__]
fields = [
    module.Field(True, False),
    module.Field(True, True),
    module.Field(False, False),
]
assert module.fields_in_init_order(fields) == (
    (fields[0],),
    (fields[1],),
)

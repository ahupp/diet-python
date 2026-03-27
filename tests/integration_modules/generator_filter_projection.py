
def fields_in_init_order(fields):
    return tuple(
        field.name
        for field in fields
        if field.init and not field.kw_only
    )


class Field:
    def __init__(self, name, *, init, kw_only=False):
        self.name = name
        self.init = init
        self.kw_only = kw_only


# diet-python: validate

def validate_module(module):
    fields = [
                module.Field("field0", init=True, kw_only=False),
                module.Field("field1", init=True, kw_only=True),
                module.Field("field2", init=False, kw_only=False),
            ]

    assert module.fields_in_init_order(fields) == ("field0",)

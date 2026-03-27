class date:
    __slots__ = ()


class Example:
    slots = date.__slots__

    def date(self):
        return date()

# diet-python: validate

def validate_module(module):

    instance = module.Example()
    result = instance.date()
    assert isinstance(result, module.date)

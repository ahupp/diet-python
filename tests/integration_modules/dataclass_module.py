import dataclasses


@dataclasses.dataclass
class Example:
    value: int


# diet-python: validate

def validate_module(module):
    instance = module.Example(value=1)
    assert instance.value == 1
    assert module.Example.__annotations__["value"] is int

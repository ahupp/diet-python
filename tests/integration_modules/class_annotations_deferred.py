class ThemeSection:
    if False:
        from typing import ClassVar
    __dataclass_fields__: ClassVar[int]

# diet-python: validate

def validate_module(module):

    import pytest

    with pytest.raises(NameError):
        _ = module.ThemeSection.__annotations__

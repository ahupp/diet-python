def run():
    if NotImplemented:
        pass


# diet-python: validate

def validate_module(module):
    import pytest

    with pytest.raises(
        TypeError, match="NotImplemented should not be used in a boolean context"
    ):
        module.run()

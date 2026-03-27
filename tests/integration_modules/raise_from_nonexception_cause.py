
class ConstructMortal(BaseException):
    def __new__(*args, **kwargs):
        return ["mortal value"]


def run():
    try:
        raise IndexError from ConstructMortal
    except TypeError as exc:
        return str(exc)


# diet-python: validate

def validate_module(module):
    assert "should have returned an instance of BaseException" in module.run()

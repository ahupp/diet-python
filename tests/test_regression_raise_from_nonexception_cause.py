from tests._integration import transformed_module


def test_raise_from_nonexception_cause(tmp_path):
    source = """
class ConstructMortal(BaseException):
    def __new__(*args, **kwargs):
        return ["mortal value"]


def run():
    try:
        raise IndexError from ConstructMortal
    except TypeError as exc:
        return str(exc)
"""
    with transformed_module(tmp_path, "raise_from_nonexception_cause", source) as module:
        assert "should have returned an instance of BaseException" in module.run()

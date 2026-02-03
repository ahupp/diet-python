from tests._integration import transformed_module


def test_call_annotate_function_string_fakeglobals(tmp_path):
    source = """
import annotationlib
from annotationlib import Format


def annotate(format, /, __Format=Format, __NotImplementedError=NotImplementedError):
    if format == __Format.VALUE:
        return {'x': str}
    elif format == __Format.VALUE_WITH_FAKE_GLOBALS:
        return {'x': int}
    else:
        raise __NotImplementedError(format)


def run():
    return annotationlib.call_annotate_function(annotate, Format.STRING)
"""
    with transformed_module(tmp_path, "annotationlib_fakeglobals", source) as module:
        assert module.run() == {"x": "int"}

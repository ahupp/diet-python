
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


# diet-python: validate

def validate_module(module):
    assert module.run() == {"x": "int"}

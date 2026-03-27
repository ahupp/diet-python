
from annotationlib import Format

class C:
    value: int

def run():
    return (
        "__annotate__" in C.__dict__,
        "__annotate_func__" in C.__dict__,
        callable(getattr(C, "__annotate__", None)),
        C.__annotate__(Format.VALUE),
    )


# diet-python: validate

def validate_module(module):
    has_annotate, has_annotate_func, callable_annotate, annotations = module.run()

    assert has_annotate is False

    assert has_annotate_func is True

    assert callable_annotate is True

    assert annotations == {"value": int}

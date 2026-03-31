import annotationlib
from annotationlib import Format, ForwardRef


def run():
    class Demo:
        nonlocal sequence_b
        x: sequence_b
        y: sequence_b[int]

    fwdrefs = annotationlib.get_annotations(Demo, format=Format.FORWARDREF)
    ok_types = isinstance(fwdrefs["x"], ForwardRef) and isinstance(fwdrefs["y"], ForwardRef)
    sequence_b = list
    return ok_types, fwdrefs["x"], fwdrefs["y"]


# diet-python: validate

def validate_module(module):
    ok_types, x_val, y_val = module.run()
    assert ok_types is True
    assert isinstance(x_val, annotationlib.ForwardRef)
    assert x_val.__forward_arg__ == "sequence_b"
    assert isinstance(y_val, annotationlib.ForwardRef)
    assert y_val.__forward_arg__.startswith("sequence_b[")

import annotationlib
from annotationlib import Format


def run():
    obj = object()

    class RaisesAttributeError:
        attriberr: obj.missing

    ann = annotationlib.get_annotations(RaisesAttributeError, format=Format.FORWARDREF)
    return ann["attriberr"]


# diet-python: validate

def validate_module(module):
    value = module.run()
    assert isinstance(value, annotationlib.ForwardRef)
    assert value.__forward_arg__ == "obj.missing"

"""Exposes NameError when mutating __annotations__ in a class body."""

from typing import get_type_hints

class M(type):
    __annotations__['123'] = 123
    o: type = object

HINTS = get_type_hints(M)

# diet-python: validate

def validate_module(module):
    raise AssertionError("class_annotations_mutation should fail to import")

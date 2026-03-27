import dotted_import_alias_pkg.submodule as submodule

VALUE = submodule.SENTINEL
MODULE_NAME = submodule.__name__

# diet-python: validate

def validate_module(module):

    import pytest

    assert module.VALUE == "submodule"
    assert module.MODULE_NAME == "dotted_import_alias_pkg.submodule"

"""Reproduces the failure when importing a stdlib module with an alias."""

import sys as sys_alias

VALUE = sys_alias.version

# diet-python: validate

def validate_module(module):

    import pytest

    assert module.VALUE == __import__("sys").version

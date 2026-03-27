from __future__ import annotations


def exercise():
    errors = []
    try:
        raise ValueError("boom")
    except ValueError:
        errors.append("caught")
    else:
        errors.append("else")
    return errors

# diet-python: validate

def validate_module(module):

    assert module.exercise() == ["caught"]

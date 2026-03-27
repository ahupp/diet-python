
from string.templatelib import Template

def make():
    return t"{1}"


# diet-python: validate

def validate_module(module):
    result = module.make()

    assert isinstance(result, module.Template)

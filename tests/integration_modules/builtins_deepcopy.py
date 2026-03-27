
import builtins
import copy

def run():
    ns = {"__builtins__": builtins.__dict__}
    copy.deepcopy(ns)
    return True


# diet-python: validate

def validate_module(module):
    assert module.run() is True

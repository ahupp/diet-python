from enum import Enum


class Scope(Enum):
    Function = "function"
    Module = "module"


HIGH_SCOPES = [scope for scope in Scope if scope is Scope.Function]
FUNCTION_MEMBERS = [Scope.Function for scope in Scope if scope is Scope.Function]

# diet-python: validate

from __future__ import annotations

module = __import__("sys").modules[__name__]
assert module.HIGH_SCOPES == [module.Scope.Function]
assert module.FUNCTION_MEMBERS == [module.Scope.Function]

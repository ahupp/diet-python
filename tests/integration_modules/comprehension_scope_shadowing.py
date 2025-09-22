from enum import Enum


class Scope(Enum):
    Function = "function"
    Module = "module"


HIGH_SCOPES = [scope for scope in Scope if scope is Scope.Function]
FUNCTION_MEMBERS = [Scope.Function for scope in Scope if scope is Scope.Function]

from typing import get_type_hints

type Alias = int


class X:
    T = int
    type U = T
    type V = Alias


class Y[T]:
    type V = T
    value: T


Y_HINTS = get_type_hints(Y)

# diet-python: validate

module = __import__("sys").modules[__name__]
assert module.Alias.__value__ is int
assert module.X.U.__value__ is int
assert module.X.V.__value__ is module.Alias

type_param = module.Y.__type_params__[0]
assert module.Y.V.__type_params__ == ()
assert module.Y.V.__value__ is type_param
assert module.Y_HINTS["value"] is type_param

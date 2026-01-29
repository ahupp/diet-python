from __future__ import annotations

from typing import Generic, TypeVar, get_type_hints

T = TypeVar("T")


class Outer(Generic[T]):
    class Inner(Generic[T]):
        pass

    inner_ref: Inner


class C[T]:
    value: T


def inner_class_hint_is_inner() -> bool:
    return get_type_hints(Outer)["inner_ref"] is Outer.Inner


def pep695_generic_info():
    hints = get_type_hints(C)
    return (
        "T" in C.__dict__,
        C.__bases__,
        C.__orig_bases__,
        C.__type_params__,
        hints["value"],
    )

# diet-python: validate

from __future__ import annotations

import typing

module = __import__("sys").modules[__name__]
assert module.inner_class_hint_is_inner() is True
has_t, bases, orig_bases, type_params, value_hint = module.pep695_generic_info()
assert has_t is False
assert typing.Generic in bases
assert orig_bases[0].__origin__ is typing.Generic
assert orig_bases[0].__args__ == (type_params[0],)
assert value_hint is type_params[0]

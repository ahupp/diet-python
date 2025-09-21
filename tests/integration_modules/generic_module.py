from typing import Generic, TypeVar


T = TypeVar("T")


class Box(Generic[T]):
    pass


def make_specialization():
    class IntBox(Box[int]):
        pass

    return IntBox

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

"""Shows that PEP 695 type parameters survive the transform."""

from typing import get_type_hints

Eggs = int
Spam = str

class C[Eggs, **Spam]:
    x: Eggs
    y: Spam

HINTS = get_type_hints(C)
PARAMS = C.__type_params__

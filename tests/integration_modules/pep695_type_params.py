"""Shows that PEP 695 type parameters survive the transform."""

from typing import get_type_hints

Eggs = int
Spam = str

class C[Eggs, **Spam]:
    x: Eggs
    y: Spam

HINTS = get_type_hints(C)
PARAMS = C.__type_params__

# diet-python: validate

def validate_module(module):
    assert isinstance(module.PARAMS, tuple)
    assert len(module.PARAMS) == 2
    eggs, spam = module.PARAMS
    assert module.HINTS == {"x": eggs, "y": spam}
    assert type(eggs).__name__ == "TypeVar"
    assert eggs.__name__ == "Eggs"
    assert type(spam).__name__ == "ParamSpec"
    assert spam.__name__ == "Spam"

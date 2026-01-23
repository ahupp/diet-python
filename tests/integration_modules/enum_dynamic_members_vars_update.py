from __future__ import annotations

from enum import Enum


FOO_DEFINES = {
    "FOO_CAT": "aloof",
    "BAR_DOG": "friendly",
    "FOO_HORSE": "big",
}


class Foo(Enum):
    vars().update({
        k: v
        for k, v in FOO_DEFINES.items()
        if k.startswith("FOO_")
    })

    def upper(self):
        return self.value.upper()

# diet-python: validate

from __future__ import annotations

def validate(module):
    foo = module.Foo
    assert list(foo) == [foo.FOO_CAT, foo.FOO_HORSE]
    assert foo.FOO_CAT.value == "aloof"
    assert foo.FOO_HORSE.upper() == "BIG"

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


# diet-python: validate

def validate_module(module):
    assert [member.name for member in module.Foo] == ["FOO_CAT", "FOO_HORSE"]

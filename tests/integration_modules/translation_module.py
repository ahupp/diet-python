_ = lambda text: f"translated:{text}"


def translate_message():
    _("before try")
    try:
        raise RuntimeError("boom")
    except RuntimeError:
        return _("after except")


def call_translate():
    return translate_message()

# diet-python: validate

from __future__ import annotations

def validate(module):
    assert module.call_translate() == "translated:after except"

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

module = __import__("sys").modules[__name__]
assert module.call_translate() == "translated:after except"

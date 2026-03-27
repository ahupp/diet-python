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

def validate_module(module):

    assert module.call_translate() == "translated:after except"

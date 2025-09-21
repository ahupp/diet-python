from __future__ import annotations

from ._integration import transformed_module

MODULE_SOURCE = """
_ = lambda text: f"translated:{text}"


def translate_message():
    _("before try")
    try:
        raise RuntimeError("boom")
    except RuntimeError:
        return _("after except")


def call_translate():
    return translate_message()
"""


MODULE_WITH_PASS = """
def read_flag():
    try:
        raise OSError
    except OSError:
        pass
    return "handled"
"""


def test_bare_except_does_not_shadow_module_globals(tmp_path):
    with transformed_module(tmp_path, "translation_module", MODULE_SOURCE) as module:
        assert module.call_translate() == "translated:after except"


def test_except_block_preserves_body_statements(tmp_path):
    with transformed_module(tmp_path, "pass_in_except", MODULE_WITH_PASS) as module:
        assert module.read_flag() == "handled"

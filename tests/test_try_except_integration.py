from __future__ import annotations

import importlib
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT))

import diet_import_hook


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


def test_bare_except_does_not_shadow_module_globals(tmp_path):
    module_name = "translation_module"
    module_path = tmp_path / f"{module_name}.py"
    module_path.write_text(MODULE_SOURCE, encoding="utf-8")

    module_dir = str(module_path.parent)
    sys.path.insert(0, module_dir)
    diet_import_hook.install()

    try:
        sys.modules.pop(module_name, None)
        module = importlib.import_module(module_name)
        assert module.call_translate() == "translated:after except"
    finally:
        sys.modules.pop(module_name, None)
        if module_dir in sys.path:
            sys.path.remove(module_dir)

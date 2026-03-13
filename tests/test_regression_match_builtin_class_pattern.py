from __future__ import annotations

from pathlib import Path

from tests._integration import transformed_module


def test_builtin_str_class_pattern_binds_subject_in_transformed_runtime(
    tmp_path: Path,
) -> None:
    source = """
match "aa":
    case str(slot):
        MATCHED = slot
    case _:
        MATCHED = None
"""

    with transformed_module(tmp_path, "match_builtin_class_pattern_regression", source) as module:
        assert module.MATCHED == "aa"

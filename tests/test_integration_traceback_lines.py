from __future__ import annotations

import traceback
from pathlib import Path

import pytest

from tests._integration import split_integration_case


def test_validate_traceback_line_numbers(tmp_path: Path) -> None:
    source = (
        "def global_function():\n"
        "    return (lambda: None).__qualname__\n"
        "\n"
        "# diet-python: validate\n"
        "\n"
        "def validate(module):\n"
        "    assert False\n"
    )
    module_path = tmp_path / "case.py"
    module_path.write_text(source, encoding="utf-8")

    _, validate_source = split_integration_case(module_path)
    namespace: dict[str, object] = {
        "__name__": "tests.integration_validate.case",
        "__package__": "tests",
        "__file__": str(module_path),
    }
    exec(compile(validate_source, str(module_path), "exec"), namespace)

    validate = namespace["validate"]
    expected_line = next(
        idx
        for idx, line in enumerate(source.splitlines(), 1)
        if line.strip() == "assert False"
    )

    with pytest.raises(AssertionError) as exc_info:
        validate(None)

    last_frame = traceback.extract_tb(exc_info.value.__traceback__)[-1]
    assert last_frame.filename == str(module_path)
    assert last_frame.lineno == expected_line

from __future__ import annotations

from pathlib import Path

import pytest

from tests._integration import transformed_module


def test_chained_assignment_in_class_preserves_identity(tmp_path: Path) -> None:
    source = """
class Example:
    a = b = object()
"""

    with transformed_module(tmp_path, "chained_assignment", source) as module:
        Example = module.Example

    assert Example.a is Example.b


def test_dataclass_field_annotations_are_dropped(tmp_path: Path) -> None:
    source = """
import dataclasses

@dataclasses.dataclass
class Example:
    value: int
"""

    with transformed_module(tmp_path, "dataclass_module", source) as module:
        Example = module.Example

    with pytest.raises(TypeError, match="unexpected keyword argument 'value'"):
        Example(value=1)

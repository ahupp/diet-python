from __future__ import annotations

from pathlib import Path

from tests._integration import transformed_module


def test_chained_assignment_in_class_preserves_identity(tmp_path: Path) -> None:
    source = """
class Example:
    a = b = object()
"""

    with transformed_module(tmp_path, "chained_assignment", source) as module:
        Example = module.Example

    assert Example.a is Example.b


def test_dataclass_field_annotations_are_retained(tmp_path: Path) -> None:
    source = """
import dataclasses

@dataclasses.dataclass
class Example:
    value: int
"""

    with transformed_module(tmp_path, "dataclass_module", source) as module:
        Example = module.Example

    instance = Example(value=1)
    assert instance.value == 1
    assert Example.__annotations__["value"] is int


def test_nested_class_is_bound_to_enclosing_class(tmp_path: Path) -> None:
    source = """
class Container:
    class Member:
        pass


def get_member() -> type | None:
    return getattr(Container, "Member", None)
"""

    with transformed_module(tmp_path, "nested_class_binding", source) as module:
        Container = module.Container
        get_member = module.get_member

    assert get_member() is Container.Member

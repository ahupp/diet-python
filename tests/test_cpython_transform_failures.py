from __future__ import annotations

import sys
from pathlib import Path

from tests._integration import ROOT, transformed_module


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


def test_frozen_dataclass_attribute_initialization_succeeds(tmp_path: Path) -> None:
    source = """
import dataclasses
import importlib

dataclasses = importlib.reload(dataclasses)

@dataclasses.dataclass(frozen=True)
class Example:
    value: int
"""

    stdlib_path = ROOT / "cpython" / "Lib"
    sys.path.insert(0, str(stdlib_path))

    try:
        with transformed_module(tmp_path, "frozen_dataclass", source) as module:
            Example = module.Example
    finally:
        sys.path.remove(str(stdlib_path))

    instance = Example(value=1)
    assert instance.value == 1


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


def test_method_named_open_calls_builtin(tmp_path: Path) -> None:
    source = """
from pathlib import Path


class Wrapper:
    def __init__(self, path: Path) -> None:
        self.path = path

    def open(self, mode: str = 'r', *, encoding: str = 'utf8'):
        path = self.path
        return open(path, mode, encoding=encoding)


def write_and_read(path: Path) -> str:
    wrapper = Wrapper(path)
    with wrapper.open('w', encoding='utf8') as handle:
        handle.write('payload')
    with wrapper.open('r', encoding='utf8') as handle:
        return handle.read()
"""

    with transformed_module(tmp_path, "method_named_open", source) as module:
        target = tmp_path / "example.txt"
        result = module.write_and_read(target)

    assert result == "payload"

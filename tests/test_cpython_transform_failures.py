from __future__ import annotations

from pathlib import Path
import importlib
import sys

import pytest

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


def test_property_copydoc_uses_original_attribute_name(tmp_path: Path) -> None:
    source = r"""
class Base:
    @property
    def value(self):
        '''base doc'''
        return 1


def copydoc(func):
    func.__doc__ = getattr(Base, func.__name__).__doc__
    return func


class Derived(Base):
    @property
    @copydoc
    def value(self):
        return 2
"""

    with transformed_module(tmp_path, "property_copydoc", source) as module:
        Derived = module.Derived

    assert Derived.value.__doc__ == "base doc"
    assert Derived().value == 2


def test_nested_class_getattribute_captures_outer_bindings(tmp_path: Path) -> None:
    source = r"""
class Container:
    def probe(self):
        class A:
            token = 1

        class B:
            def __getattribute__(self, attr):
                a = A()
                return getattr(a, attr)

        return B().missing
"""

    with transformed_module(tmp_path, "nested_getattribute", source) as module:
        container = module.Container()

    with pytest.raises(AttributeError, match="'A' object has no attribute 'missing'"):
        container.probe()


def test_chained_comparisons_evaluate_side_effects_once(tmp_path: Path) -> None:
    source = r"""
calls = []


def value() -> int:
    calls.append("hit")
    return 1


def probe() -> list[str]:
    calls.clear()
    if 0 <= value() <= 2:
        return list(calls)
    return list(calls)
"""

    with transformed_module(tmp_path, "chained_comparison", source) as module:
        hits = module.probe()

    assert hits == ["hit"]


def test_class_scope_comprehension_executes(tmp_path: Path) -> None:
    source = r"""
class Example:
    values = [lc for lc in range(3)]
"""

    with transformed_module(tmp_path, "class_comprehension", source) as module:
        Example = module.Example

    assert Example.values == [0, 1, 2]




def test_nested_class_super_preserves_class_cell(tmp_path: Path) -> None:
    source = r"""
class Base:
    def probe(self):
        return "sentinel"


class Container:
    def build(self):
        class Derived(Base):
            def probe(self):
                return super().probe()

        instance = Derived()
        return instance.probe()
"""

    module_name = "nested_super"

    with transformed_module(tmp_path, module_name, source) as module:
        result = module.Container().build()

    assert result == "sentinel"

def test_nested_class_with_nonlocal_binding_executes(tmp_path: Path) -> None:
    source = r"""


class Example:
    def trigger(self):
        counter = 0

        class Token:
            def bump(self):
                nonlocal counter
                counter += 1

        token = Token()
        token.bump()
        return counter
"""

    with transformed_module(tmp_path, "nonlocal_binding", source) as module:
        Example = module.Example

    assert Example().trigger() == 1



def test_tuple_unpacking_raises_value_error(tmp_path: Path) -> None:
    source = r"""
def parse_line(line: str) -> str:
    try:
        key, value = line.split("=")
    except ValueError:
        return "handled"
    else:
        return "missing separator"
"""

    with transformed_module(tmp_path, "tuple_unpacking_module", source) as module:
        parse_line = module.parse_line

    assert parse_line("no equals here") == "handled"


def test_map_unpacking_consumes_iterator(tmp_path: Path) -> None:
    source = r"""
def summarize() -> tuple[int, int]:
    length_one, length_two = map(len, ("aa", "bbb"))
    return length_one, length_two
"""

    with transformed_module(tmp_path, "map_unpacking_module", source) as module:
        summarize = module.summarize

    assert summarize() == (2, 3)


def test_class_attribute_unpacking_binds_each_name(tmp_path: Path) -> None:
    source = r"""
class Example:
    left, right = object(), object()
"""

    with transformed_module(tmp_path, "class_attribute_unpacking", source) as module:
        Example = module.Example

    assert hasattr(Example, "left")
    assert hasattr(Example, "right")


def test_nested_class_closure_access(tmp_path: Path) -> None:
    source = r"""
class Container:
    def build(self):
        values = []

        class Recorder:
            def record(self, item):
                values.append(item)
                return list(values)

        return Recorder()


def use_container() -> list[str]:
    recorder = Container().build()
    return recorder.record("payload")
"""

    with transformed_module(tmp_path, "nested_class_closure", source) as module:
        use_container = module.use_container

    assert use_container() == ["payload"]


def test_slice_name_does_not_shadow_builtin(tmp_path: Path) -> None:
    source = r"""
def collect_segments(data: bytes) -> list[bytes]:
    pieces = []
    for start in range(len(data)):
        for end in range(start + 1, len(data) + 1):
            slice = data[start:end]
            pieces.append(slice)
    return pieces
"""

    with transformed_module(tmp_path, "slice_binding", source) as module:
        collect_segments = module.collect_segments

    assert collect_segments(b"ab") == [b"a", b"ab", b"b"]


def test_helper_bindings_are_excluded_from_all(tmp_path: Path) -> None:
    source = r"""
__all__ = ["Example"]


class Example:
    pass
"""

    with transformed_module(tmp_path, "module_all_helpers", source) as module:
        actual = set(module.__all__)
        computed = {
            name
            for name, value in vars(module).items()
            if not name.startswith("__")
            and getattr(value, "__module__", None) == module.__name__
        }

    assert computed == actual

def test_builtin_str_class_pattern_binds_subject(tmp_path: Path) -> None:
    source = """
match "aa":
    case str(slot):
        MATCHED = slot
    case _:
        MATCHED = None
"""

    with transformed_module(tmp_path, "match_builtin_class_pattern", source) as module:
        assert module.MATCHED == "aa"


def test_nested_typing_subclass_preserves_enclosing_name(tmp_path: Path) -> None:
    source = """
from typing import Any

class Container:
    def make(self) -> str:
        class Sub(Any):
            pass
        return repr(Sub)

VALUE = Container().make()
"""

    with transformed_module(tmp_path, "typing_nested_class_repr", source) as module:
        assert "Container.make.<locals>.Sub" in module.VALUE


def test_pep695_type_params_are_preserved(tmp_path: Path) -> None:
    source = """
from typing import get_type_hints

Eggs = int
Spam = str

class C[Eggs, **Spam]:
    x: Eggs
    y: Spam

HINTS = get_type_hints(C)
PARAMS = C.__type_params__
"""

    with transformed_module(tmp_path, "pep695_type_params", source) as module:
        assert isinstance(module.PARAMS, tuple)
        assert len(module.PARAMS) == 2
        eggs, spam = module.PARAMS
        assert module.HINTS == {"x": eggs, "y": spam}
        assert type(eggs).__name__ == "TypeVar"
        assert eggs.__name__ == "Eggs"
        assert type(spam).__name__ == "ParamSpec"
        assert spam.__name__ == "Spam"


def test_class_annotations_mutation_preserves_annotations(tmp_path: Path) -> None:
    source = """
from typing import get_type_hints

class M(type):
    __annotations__['123'] = 123
    o: type = object

HINTS = get_type_hints(M)
"""

    with transformed_module(tmp_path, "class_annotations_mutation", source) as module:
        M = module.M
        hints = module.HINTS

    assert M.__annotations__["123"] == 123
    assert hints["o"] is type
    assert hints["123"] == 123
    assert M.__annotations__["o"] is type


def test_typing_io_emits_multiple_deprecation_warnings(tmp_path: Path) -> None:
    source = """
import warnings

with warnings.catch_warnings(record=True) as caught:
    warnings.filterwarnings("default", category=DeprecationWarning)
    from typing.io import IO, TextIO, BinaryIO, __all__, __name__
    WARNINGS = len(caught)
    NAMES = (IO, TextIO, BinaryIO, tuple(__all__), __name__)
"""

    with transformed_module(tmp_path, "typing_io_warnings", source) as module:
        assert module.WARNINGS == 1
        io_mod, text_mod, binary_mod, exported, module_name = module.NAMES
        assert exported == ("IO", "TextIO", "BinaryIO")
        assert module_name == "typing.io"
        assert io_mod.__module__ == "typing"
        assert text_mod.__module__ == "typing"
        assert binary_mod.__module__ == "typing"

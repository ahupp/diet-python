from __future__ import annotations

import sys
from pathlib import Path

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


@pytest.mark.xfail(reason="Nested classes defined inside methods lose their __class__ binding; CPython's test_smtplib depends on this working")
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



@pytest.mark.xfail(reason="Tuple unpacking should raise ValueError; CPython's test_turtle relies on this")
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


@pytest.mark.xfail(reason="Iterable unpacking must consume the iterator; unittest's TextTestResult expects this")
def test_map_unpacking_consumes_iterator(tmp_path: Path) -> None:
    source = r"""
def summarize() -> tuple[int, int]:
    length_one, length_two = map(len, ("aa", "bbb"))
    return length_one, length_two
"""

    with transformed_module(tmp_path, "map_unpacking_module", source) as module:
        summarize = module.summarize

    assert summarize() == (2, 3)


@pytest.mark.xfail(reason="Class attribute destructuring drops bindings; fractions.Fraction loses arithmetic methods")
def test_class_attribute_unpacking_binds_each_name(tmp_path: Path) -> None:
    source = r"""
class Example:
    left, right = object(), object()
"""

    with transformed_module(tmp_path, "class_attribute_unpacking", source) as module:
        Example = module.Example

    assert hasattr(Example, "left")
    assert hasattr(Example, "right")


@pytest.mark.xfail(reason="Nested classes should capture outer scopes; failures surface in test_mmap and test_cmath")
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


@pytest.mark.xfail(reason="Target named 'slice' shadows the builtin during rewrites; mirrors CPython's test_mmap failure")
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


@pytest.mark.xfail(reason="Helper bindings generated for class definitions leak into module namespaces")
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

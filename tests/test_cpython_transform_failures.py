from __future__ import annotations

from pathlib import Path

import pytest


def test_chained_assignment_in_class_preserves_identity(run_integration_module) -> None:
    with run_integration_module("chained_assignment") as module:
        Example = module.Example

    assert Example.a is Example.b


def test_dataclass_field_annotations_are_retained(run_integration_module) -> None:
    with run_integration_module("dataclass_module") as module:
        Example = module.Example

    instance = Example(value=1)
    assert instance.value == 1
    assert Example.__annotations__["value"] is int


def test_frozen_dataclass_attribute_initialization_succeeds(
    run_integration_module,
) -> None:
    with run_integration_module("frozen_dataclass") as module:
        Example = module.Example

    instance = Example(value=1)
    assert instance.value == 1


def test_nested_class_is_bound_to_enclosing_class(run_integration_module) -> None:
    with run_integration_module("nested_class_binding") as module:
        Container = module.Container
        get_member = module.get_member

    assert get_member() is Container.Member


def test_method_named_open_calls_builtin(tmp_path: Path, run_integration_module) -> None:
    with run_integration_module("method_named_open") as module:
        target = tmp_path / "example.txt"
        result = module.write_and_read(target)

    assert result == "payload"


def test_property_copydoc_uses_original_attribute_name(run_integration_module) -> None:
    with run_integration_module("property_copydoc") as module:
        Derived = module.Derived

    assert Derived.value.__doc__ == "base doc"
    assert Derived().value == 2


def test_nested_class_getattribute_captures_outer_bindings(
    run_integration_module,
) -> None:
    with run_integration_module("nested_getattribute") as module:
        container = module.Container()

    with pytest.raises(AttributeError, match="'A' object has no attribute 'missing'"):
        container.probe()


def test_chained_comparisons_evaluate_side_effects_once(run_integration_module) -> None:
    with run_integration_module("chained_comparison") as module:
        hits = module.probe()

    assert hits == ["hit"]


def test_class_scope_comprehension_executes(run_integration_module) -> None:
    with run_integration_module("class_comprehension") as module:
        Example = module.Example

    assert Example.values == [0, 1, 2]


def test_nested_class_super_preserves_class_cell(run_integration_module) -> None:
    with run_integration_module("nested_super") as module:
        result = module.Container().build()

    assert result == "sentinel"


def test_nested_class_with_nonlocal_binding_executes(run_integration_module) -> None:
    with run_integration_module("nonlocal_binding") as module:
        Example = module.Example

    assert Example().trigger() == 1


def test_tuple_unpacking_raises_value_error(run_integration_module) -> None:
    with run_integration_module("tuple_unpacking_module") as module:
        parse_line = module.parse_line

    assert parse_line("no equals here") == "handled"


def test_map_unpacking_consumes_iterator(run_integration_module) -> None:
    with run_integration_module("map_unpacking_module") as module:
        summarize = module.summarize

    assert summarize() == (2, 3)


def test_class_attribute_unpacking_binds_each_name(run_integration_module) -> None:
    with run_integration_module("class_attribute_unpacking") as module:
        Example = module.Example

    assert hasattr(Example, "left")
    assert hasattr(Example, "right")


def test_nested_class_closure_access(run_integration_module) -> None:
    with run_integration_module("nested_class_closure") as module:
        use_container = module.use_container

    assert use_container() == ["payload"]


def test_slice_name_does_not_shadow_builtin(run_integration_module) -> None:
    with run_integration_module("slice_binding") as module:
        collect_segments = module.collect_segments

    assert collect_segments(b"ab") == [b"a", b"ab", b"b"]


def test_helper_bindings_are_excluded_from_all(run_integration_module) -> None:
    with run_integration_module("module_all_helpers") as module:
        actual = set(module.__all__)
        computed = {
            name
            for name, value in vars(module).items()
            if not name.startswith("__")
            and getattr(value, "__module__", None) == module.__name__
        }

    assert computed == actual


def test_builtin_str_class_pattern_binds_subject(run_integration_module) -> None:
    with run_integration_module("match_builtin_class_pattern") as module:
        assert module.MATCHED == "aa"


def test_nested_typing_subclass_preserves_enclosing_name(run_integration_module) -> None:
    with run_integration_module("typing_nested_class_repr") as module:
        assert "Container.make.<locals>.Sub" in module.VALUE


def test_pep695_type_params_are_preserved(run_integration_module) -> None:
    with run_integration_module("pep695_type_params") as module:
        assert isinstance(module.PARAMS, tuple)
        assert len(module.PARAMS) == 2
        eggs, spam = module.PARAMS
        assert module.HINTS == {"x": eggs, "y": spam}
        assert type(eggs).__name__ == "TypeVar"
        assert eggs.__name__ == "Eggs"
        assert type(spam).__name__ == "ParamSpec"
        assert spam.__name__ == "Spam"


def test_class_annotations_mutation_raises_nameerror(run_integration_module) -> None:
    with pytest.raises(NameError):
        with run_integration_module("class_annotations_mutation"):
            pass

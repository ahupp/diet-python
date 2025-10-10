# CPython Transform Failures

The CPython compatibility suite highlighted several regressions caused by the
current transform when it is applied to both standard library and test
modules. The following sections summarize the observed behavior, the
integration tests that capture each issue, and the suspected root causes.

## 1. Builtin class pattern destructuring

*Observed in*: `tests/integration_modules/match_builtin_class_pattern.py`

Matching against builtin classes such as `str` raises `AttributeError` when the
transformed pattern accesses a synthetic `__match_args__`. CPython falls back to
the subject value instead of consulting `__match_args__` for builtins, so the
transform needs a similar fallback path.

## 2. Nested typing subclass `__qualname__`

*Observed in*: `tests/integration_modules/typing_nested_class_repr.py`

The namespace rewriting drops the containing class name when synthesizing
nested subclasses of `typing.Generic`, causing reprs to omit intermediate
containers (`make.<locals>.Sub` instead of `Container.make.<locals>.Sub`). The
transform should propagate the enclosing `__qualname__` when rebuilding nested
class definitions.

## 3. PEP 695 type parameter metadata

*Observed in*: `tests/integration_modules/pep695_type_params.py`

Transformed generics lose their compiler-synthesized `__type_params__` objects,
leaving the runtime with `PARAMS == ()` and annotations bound to concrete
classes. The transform must preserve the original type parameter instances and
re-inject them into the desugared namespace instead of reusing the runtime
bindings.

## 4. Class body `__annotations__` mutation

*Observed in*: `tests/integration_modules/class_annotations_mutation.py`

Importing a transformed module that mutates `__annotations__` inside a class
body raises `NameError` because the helper namespace references the attribute
before CPython creates it. Initializing an empty annotations dictionary in the
synthetic namespace would match CPython’s implicit setup.

## 5. `typing.io` multi-import warnings

*Observed in*: `tests/integration_modules/typing_io_warnings.py`

A single `from typing.io import ...` statement becomes multiple helper
invocations, and each one triggers the deprecated proxy’s warning machinery.
CPython emits one warning, while the transformed module emits several. The
transform should reuse a shared import result to avoid duplicated warnings.

These failures remain open and are documented by the new integration fixtures
and regression tests added in the previous change set.

# CPython Transform Failures

The CPython compatibility suite highlighted several regressions caused by the
current transform when it is applied to both standard library and test
modules. The following sections summarize the observed behavior, the
integration tests that capture each issue, and the suspected root causes.

## 1. Builtin class pattern destructuring *(fixed)*

*Observed in*: `tests/integration_modules/match_builtin_class_pattern.py`

The transform now mirrors CPython’s fallback for builtins that do not define
`__match_args__`, so the integration module executes without raising
`AttributeError` and successfully binds the subject value.

## 2. Nested typing subclass `__qualname__` *(fixed)*

*Observed in*: `tests/integration_modules/typing_nested_class_repr.py`

Method-local class rewrites now preserve the enclosing class name, so reprs of
`typing` subclasses retain `Container.make.<locals>.Sub` just like CPython.

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

The remaining failures are still documented by the integration fixtures and
regression tests added in the previous change set.

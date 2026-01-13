from __future__ import annotations

import typing


def test_typing_generics_cases(run_integration_module):
    with run_integration_module("typing_generics_cases") as module:
        assert module.inner_class_hint_is_inner() is True
        has_t, bases, orig_bases, type_params, value_hint = module.pep695_generic_info()
        assert has_t is False
        assert typing.Generic in bases
        assert orig_bases[0].__origin__ is typing.Generic
        assert orig_bases[0].__args__ == (type_params[0],)
        assert value_hint is type_params[0]

from __future__ import annotations

import pytest


@pytest.mark.integration
def test_generic_namedtuple_fields_are_preserved(run_integration_module):
    with run_integration_module("generic_namedtuple_fields") as module:
        assert module.RESULT.out == "out"
        assert module.RESULT.err == "err"

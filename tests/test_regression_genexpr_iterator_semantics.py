import pytest

def test_genexpr_requires_iterator(run_integration_module):
    with run_integration_module("genexpr_iterator_semantics") as module:
        with pytest.raises(TypeError, match=r"object is not an iterator"):
            module.main()

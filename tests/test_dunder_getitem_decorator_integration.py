import pytest


@pytest.mark.integration
@pytest.mark.parametrize("item", [1])
def test_decorated_dunder_getitem_returns_item(run_integration_module, item):
    with run_integration_module("dunder_getitem_decorator") as module:
        example = module.Example()
        assert example[item] == item

from unittest import mock


class Context:
    __enter__ = mock.MagicMock(return_value="value")
    __exit__ = mock.MagicMock(return_value=False)


def run():
    Context.__enter__.reset_mock()
    Context.__exit__.reset_mock()
    with Context():
        pass
    return Context.__enter__.mock_calls, Context.__exit__.mock_calls

# diet-python: validate

def validate_module(module):

    import pytest

    enter_calls, exit_calls = module.run()

    assert enter_calls == [module.mock.call()]
    assert exit_calls == [module.mock.call(None, None, None)]

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

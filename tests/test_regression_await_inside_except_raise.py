import asyncio


def test_await_inside_except_preserves_bare_raise(run_integration_module):
    with run_integration_module("await_inside_except_raise") as module:
        try:
            module.run()
        except ValueError as exc:
            assert exc.args == ("boom",)
        else:  # pragma: no cover
            raise AssertionError("expected ValueError")

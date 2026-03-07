import asyncio

from tests._integration import transformed_module


def test_with_error_messages(tmp_path):
    source = """
import asyncio

class AsyncOnly:
    async def __aenter__(self):
        return self
    async def __aexit__(self, exc_type, exc, tb):
        return False

class SyncOnly:
    def __enter__(self):
        return self
    def __exit__(self, exc_type, exc, tb):
        return False


def run_sync():
    try:
        with AsyncOnly():
            pass
    except TypeError as exc:
        return str(exc)
    return None


def run_async():
    async def inner():
        try:
            async with SyncOnly():
                pass
        except TypeError as exc:
            return str(exc)
        return None
    return asyncio.run(inner())
"""
    with transformed_module(tmp_path, "with_error_messages", source) as module:
        async_only_name = f"{module.AsyncOnly.__module__}.{module.AsyncOnly.__qualname__}"
        sync_only_name = f"{module.SyncOnly.__module__}.{module.SyncOnly.__qualname__}"
        assert (
            module.run_sync()
            == f"{async_only_name!r} object does not support the context manager protocol (missed __exit__ method) but it supports the asynchronous context manager protocol. Did you mean to use 'async with'?"
        )
        assert (
            module.run_async()
            == f"{sync_only_name!r} object does not support the asynchronous context manager protocol (missed __aexit__ method) but it supports the context manager protocol. Did you mean to use 'with'?"
        )

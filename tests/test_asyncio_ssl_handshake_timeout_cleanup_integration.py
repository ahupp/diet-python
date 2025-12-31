from __future__ import annotations


def test_asyncio_ssl_handshake_timeout_cleanup(run_integration_module):
    with run_integration_module("asyncio_ssl_handshake_timeout_cleanup") as module:
        assert module.leak_check() is None

from __future__ import annotations

import gc
import importlib
import ssl
import sys
import types
import weakref
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CPYTHON_ASYNCIO = ROOT / "cpython" / "Lib" / "asyncio"
PKG_NAME = "dp_asyncio"
if PKG_NAME not in sys.modules:
    package = types.ModuleType(PKG_NAME)
    package.__path__ = [str(CPYTHON_ASYNCIO)]
    sys.modules[PKG_NAME] = package

base_events = importlib.import_module(f"{PKG_NAME}.base_events")
constants = importlib.import_module(f"{PKG_NAME}.constants")
exceptions = importlib.import_module(f"{PKG_NAME}.exceptions")
protocols = importlib.import_module(f"{PKG_NAME}.protocols")
selector_events = importlib.import_module(f"{PKG_NAME}.selector_events")
sslproto = importlib.import_module(f"{PKG_NAME}.sslproto")
tasks = importlib.import_module(f"{PKG_NAME}.tasks")


class DummySock:
    def setblocking(self, flag):
        pass


class DummyTransport:
    def __init__(self):
        self.closed = False

    def _force_close(self, exc):
        self.closed = True

    def get_extra_info(self, name, default=None):
        return default

    def write(self, data):
        pass

    def is_closing(self):
        return self.closed

    def pause_reading(self):
        pass

    def resume_reading(self):
        pass

    def set_write_buffer_limits(self, high=None, low=None):
        pass


class DummyLoop(selector_events.BaseSelectorEventLoop):
    def _make_ssl_transport(
        self,
        rawsock,
        protocol,
        sslcontext,
        waiter=None,
        *,
        server_side=False,
        server_hostname=None,
        extra=None,
        server=None,
        ssl_handshake_timeout=constants.SSL_HANDSHAKE_TIMEOUT,
        ssl_shutdown_timeout=constants.SSL_SHUTDOWN_TIMEOUT,
    ):
        ssl_protocol = sslproto.SSLProtocol(
            self,
            protocol,
            sslcontext,
            waiter,
            server_side,
            server_hostname,
            ssl_handshake_timeout=ssl_handshake_timeout,
            ssl_shutdown_timeout=ssl_shutdown_timeout,
        )
        ssl_protocol.connection_made(DummyTransport())
        return ssl_protocol._app_transport

    def _make_socket_transport(self, sock, protocol, waiter=None, *, extra=None, server=None):
        return DummyTransport()


def leak_check():
    loop = DummyLoop()
    sslctx = ssl.create_default_context()
    ref = weakref.ref(sslctx)
    sock = DummySock()

    async def run():
        await tasks.wait_for(
            loop._create_connection_transport(
                sock,
                protocols.Protocol,
                sslctx,
                "",
                ssl_handshake_timeout=10.0,
            ),
            0.01,
        )

    try:
        loop.run_until_complete(run())
    except BaseException as exc:
        if exc.__class__.__name__ not in ("TimeoutError", "CancelledError"):
            raise
    finally:
        loop.close()

    sslctx = None
    gc.collect()
    return ref()

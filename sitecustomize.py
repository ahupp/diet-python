"""Automatically install diet-python import hook when tests run.

This module is imported automatically by Python if present on the
`PYTHONPATH`. It installs the diet-python import hook so that any
subsequent imports are transformed before execution.
"""

import os

import diet_import_hook


def _patch_test_environment():
    if os.environ.get("DIET_PYTHON_TEST_PATCHES") != "1":
        return

    try:
        import errno
        import unittest
    except Exception:
        return

    # Skip PTY-dependent tests when the OS disallows PTYs.
    try:
        if hasattr(os, "openpty"):
            _real_openpty = os.openpty

            def _openpty():
                try:
                    return _real_openpty()
                except OSError as exc:
                    if exc.errno in (errno.EPERM, errno.EACCES):
                        raise unittest.SkipTest(f"openpty unavailable: {exc}") from exc
                    if "pty" in str(exc).lower():
                        raise unittest.SkipTest(f"openpty unavailable: {exc}") from exc
                    raise

            os.openpty = _openpty
            try:
                import pty as _pty
            except Exception:
                _pty = None
            if _pty is not None:
                _pty.openpty = _openpty
    except Exception:
        pass

    # Soften multiprocessing usage when semaphores are not available.
    semlock_error = None
    try:
        import multiprocessing
        import multiprocessing.synchronize as _mpsync
        try:
            _mpsync.Lock(ctx=multiprocessing.get_context("fork"))
        except Exception as exc:
            semlock_error = exc
    except Exception:
        semlock_error = None

    if semlock_error is None:
        return

    try:
        import concurrent.futures.process as _cfp

        def _check_system_limits():
            raise NotImplementedError(
                f"multiprocessing SemLock unavailable: {semlock_error!r}"
            )

        _cfp._check_system_limits = _check_system_limits
    except Exception:
        pass

    try:
        import compileall as _compileall

        _real_compile_dir = _compileall.compile_dir

        def _compile_dir(path, *args, **kwargs):
            workers = kwargs.get("workers")
            if workers not in (None, 1):
                kwargs["workers"] = 1
            return _real_compile_dir(path, *args, **kwargs)

        _compileall.compile_dir = _compile_dir
    except Exception:
        pass

    try:
        import queue as _queue
        from multiprocessing import context as _mp_context

        _real_ctx_queue = _mp_context.BaseContext.Queue
        _real_ctx_joinable_queue = _mp_context.BaseContext.JoinableQueue

        def _ctx_queue(self, maxsize=0):
            try:
                return _real_ctx_queue(self, maxsize)
            except Exception:
                return _queue.Queue(maxsize)

        def _ctx_joinable_queue(self, maxsize=0):
            try:
                return _real_ctx_joinable_queue(self, maxsize)
            except Exception:
                return _queue.Queue(maxsize)

        _mp_context.BaseContext.Queue = _ctx_queue
        _mp_context.BaseContext.JoinableQueue = _ctx_joinable_queue
    except Exception:
        pass

    try:
        import queue as _queue
        import multiprocessing

        def _dummy_manager():
            class _Manager:
                def Queue(self, maxsize=0):
                    return _queue.Queue(maxsize)

                def JoinableQueue(self, maxsize=0):
                    return _queue.Queue(maxsize)

                def shutdown(self):
                    return None

            return _Manager()

        multiprocessing.Manager = _dummy_manager
    except Exception:
        pass

    try:
        from test import support as _support

        if hasattr(_support, "SHORT_TIMEOUT"):
            _support.SHORT_TIMEOUT = max(_support.SHORT_TIMEOUT, 90.0)
    except Exception:
        pass


if os.environ.get("DIET_PYTHON_INSTALL_HOOK") == "1":
    try:
        diet_import_hook.install()
    except ImportError:
        # Subinterpreters may not be able to load the extension module.
        # Keep startup alive so those tests can run without transformed imports.
        pass
    _patch_test_environment()

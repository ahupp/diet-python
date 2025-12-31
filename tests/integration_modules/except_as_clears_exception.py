from __future__ import annotations

import gc
import types


def capture():
    try:
        raise OSError("boom")
    except OSError as err:
        return err


def count_exception_referrer_frames():
    exc = capture()
    refs = [ref for ref in gc.get_referrers(exc) if isinstance(ref, types.FrameType)]
    return len(refs)

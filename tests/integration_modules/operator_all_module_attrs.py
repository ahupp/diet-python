from __future__ import annotations

import operator


def exercise():
    bad = []
    for name, value in vars(operator).items():
        if name.startswith("__"):
            continue
        try:
            _ = value.__module__
        except Exception as exc:
            bad.append((name, type(value).__name__, type(exc).__name__))
    return bad

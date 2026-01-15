from __future__ import annotations

import dataclasses

try:
    dataclasses.make_dataclass("C", [("for", int)])
except TypeError as exc:
    ERROR = str(exc)
else:
    ERROR = None

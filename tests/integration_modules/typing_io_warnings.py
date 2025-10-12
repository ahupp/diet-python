"""Importing typing.io should emit a single deprecation warning."""

import warnings

with warnings.catch_warnings(record=True) as caught:
    warnings.filterwarnings("default", category=DeprecationWarning)
    from typing.io import IO, TextIO, BinaryIO, __all__, __name__
    WARNINGS = len(caught)
    NAMES = (IO, TextIO, BinaryIO, tuple(__all__), __name__)

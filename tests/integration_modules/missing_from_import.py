"""Exercise ``from module import name`` when ``name`` is absent."""

from missing_from_import_target import VALUE


try:
    from missing_from_import_target import MISSING
except ImportError:
    RESULT = "fallback"
else:
    RESULT = MISSING

"""Exercise ``from module import name`` when ``name`` is absent."""

from missing_from_import_target import VALUE


try:
    from missing_from_import_target import MISSING
except ImportError as exc:
    RESULT = "fallback"
    ERROR_NAME = exc.name
    ERROR_PATH = exc.path
else:
    RESULT = MISSING
    ERROR_NAME = None
    ERROR_PATH = None

"""Automatically install diet-python import hook when tests run.

This module is imported automatically by Python if present on the
`PYTHONPATH`. It installs the diet-python import hook so that any
subsequent imports are transformed before execution.
"""

import os
import sys
from pathlib import Path

PYTHON_SRC = Path(__file__).resolve().parent / "soac_py" / "src"
if str(PYTHON_SRC) not in sys.path:
    sys.path.insert(0, str(PYTHON_SRC))
import_hook = None


if os.environ.get("DIET_PYTHON_INSTALL_HOOK") == "1":
    try:
        from soac import import_hook as _import_hook

        import_hook = _import_hook
        import_hook.install()
    except ImportError:
        # Subinterpreters may not be able to load the extension module.
        # Keep startup alive so those tests can run without transformed imports.
        pass

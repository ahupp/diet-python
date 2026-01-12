"""Automatically install diet-python import hook when tests run.

This module is imported automatically by Python if present on the
`PYTHONPATH`. It installs the diet-python import hook so that any
subsequent imports are transformed before execution.
"""

import os

import diet_import_hook

if os.environ.get("DIET_PYTHON_INSTALL_HOOK") == "1":
    diet_import_hook.install()

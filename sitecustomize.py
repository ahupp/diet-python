"""Automatically install diet-python import hook when tests run.

This module is imported automatically by Python if present on the
`PYTHONPATH`. It installs the diet-python import hook so that any
subsequent imports are transformed before execution.
"""

import diet_import_hook

diet_import_hook.install()

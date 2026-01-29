"""Reproduces the failure when importing a stdlib module with an alias."""

import sys as sys_alias

VALUE = sys_alias.version

# diet-python: validate

from __future__ import annotations

import pytest

module = __import__("sys").modules[__name__]
assert module.VALUE == __import__("sys").version

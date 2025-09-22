"""Reproduces the failure when importing a stdlib module with an alias."""

import sys as sys_alias

VALUE = sys_alias.version

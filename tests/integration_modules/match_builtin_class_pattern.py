"""Replicates CPython's dataclasses slot handling using a str class pattern."""

match "aa":
    case str(slot):
        MATCHED = slot
    case _:
        MATCHED = None

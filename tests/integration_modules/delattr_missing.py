"""Ensure the transform rewrites `del` to `__dp__.delattr` correctly."""


class Example:
    pass


INSTANCE = Example()
INSTANCE.value = 1
del INSTANCE.value
ATTRIBUTE_DELETED = not hasattr(INSTANCE, "value")

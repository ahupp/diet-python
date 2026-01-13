import builtins
import sys
import types


def raise_from_with_import_patch():
    package = types.ModuleType("package")

    def _import(name, *args, **kwargs):
        sys.modules[name] = package
        return package

    original_import = builtins.__import__
    builtins.__import__ = _import
    try:
        try:
            raise ValueError("boom")
        except ValueError as exc:
            raise RuntimeError("wrapped") from exc
    except RuntimeError:
        pass
    finally:
        builtins.__import__ = original_import
    return sys.modules.get("asyncio") is package


ASYNCIO_SHADOWED = raise_from_with_import_patch()

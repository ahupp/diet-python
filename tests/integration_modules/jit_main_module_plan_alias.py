VALUE = 42

# diet-python: validate

def validate_module(module):
    assert module.VALUE == 42

    import os
    from pathlib import Path
    import runpy
    import sys
    import tempfile


    tmp = Path(tempfile.mkdtemp(prefix="dp_main_alias_"))
    pkg = tmp / "dp_main_alias_pkg"
    pkg.mkdir()
    (pkg / "__init__.py").write_text("", encoding="utf-8")
    (pkg / "__main__.py").write_text(
        "def get_value():\n"
        "    return 7\n"
        "VALUE = get_value()\n",
        encoding="utf-8",
    )

    prev_integration_only = os.environ.pop("DIET_PYTHON_INTEGRATION_ONLY", None)
    sys.path.insert(0, str(tmp))
    sys.modules.pop("__main__", None)
    try:
        namespace = runpy.run_module("dp_main_alias_pkg", run_name="__main__")
        if "VALUE" not in namespace and "_dp_module_init" in namespace:
            namespace["_dp_module_init"]()
        assert namespace["VALUE"] == 7
    finally:
        if sys.path and sys.path[0] == str(tmp):
            sys.path.pop(0)
        else:
            try:
                sys.path.remove(str(tmp))
            except ValueError:
                pass
        if prev_integration_only is None:
            os.environ.pop("DIET_PYTHON_INTEGRATION_ONLY", None)
        else:
            os.environ["DIET_PYTHON_INTEGRATION_ONLY"] = prev_integration_only

import sys
from pathlib import Path


def alias_rebind_attrs(tmp_path: Path) -> tuple[str, str]:
    package_name = "dp_alias_pkg"
    package_dir = tmp_path / package_name
    package_dir.mkdir()
    init_path = package_dir / "__init__.py"
    init_path.write_text("from .submodule import submodule\n", encoding="utf-8")
    submodule_path = package_dir / "submodule.py"
    submodule_path.write_text(
        "attr = 'submodule'\nclass submodule:\n    attr = 'rebound'\n",
        encoding="utf-8",
    )
    sys.path.insert(0, str(tmp_path))
    try:
        sys.modules.pop(package_name, None)
        sys.modules.pop(f"{package_name}.submodule", None)
        from dp_alias_pkg import submodule as from_import
        import dp_alias_pkg.submodule as direct_import
        return from_import.attr, direct_import.attr
    finally:
        sys.modules.pop(package_name, None)
        sys.modules.pop(f"{package_name}.submodule", None)
        if sys.path and sys.path[0] == str(tmp_path):
            sys.path.pop(0)

# diet-python: validate

def validate(module):
    import tempfile
    from pathlib import Path


    with tempfile.TemporaryDirectory() as tmp_dir:
        tmp_path = Path(tmp_dir)
        from_attr, direct_attr = module.alias_rebind_attrs(tmp_path)
        assert from_attr == "rebound"
        assert direct_attr == "rebound"

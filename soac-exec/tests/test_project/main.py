
import importlib.machinery
import pathlib
import subprocess
import sys

import pytest


def build_ext():
    root = pathlib.Path(__file__).resolve().parent.parent.parent
    subprocess.run(["cargo", "build"], check=True)
    build_dir = root.parent / "target" / "debug"
    lib = build_dir / "libsoac_exec.so"
    mod = build_dir / "soac_exec.so"
    if lib.exists():
        lib.replace(mod)
    sys.path.insert(0, str(build_dir))
    sys.path.insert(0, str(root))


def main():
    build_ext()
    import jit_importer

    jit_importer.install()
    project_dir = pathlib.Path(__file__).parent / "mypkg"
    sys.path.insert(0, str(project_dir))

    strict_module = __import__("strict_module")
    assert isinstance(strict_module.__spec__.loader, jit_importer.CraneLoaderExt)
    assert strict_module.__name__ == "strict_module"
    assert strict_module.expected == 1
    assert strict_module.noargs() is None
    assert strict_module.onearg(1) == 1
    assert strict_module.varargs(1, 2) == 3
    with pytest.raises(AttributeError):
        _ = strict_module.missing
    with pytest.raises(AttributeError):
        strict_module.expected = 2
    with pytest.raises(AttributeError):
        strict_module.other = 3

    with pytest.raises(NameError):
        __import__("name_error")
    sys.modules.pop("name_error", None)

    non_strict = __import__("non_strict")
    assert isinstance(
        non_strict.__spec__.loader, importlib.machinery.SourceFileLoader
    )
    assert non_strict.expected == 1
    non_strict.expected = 5
    non_strict.other = 7
    assert non_strict.expected == 5
    assert non_strict.other == 7

    flow = __import__("control_flow")
    assert flow.x == 1
    assert flow.y == 2
    assert flow.count == 2

    math_mod = __import__("complex_math")
    assert math_mod.fibonacci(10) == 55
    assert math_mod.primes_up_to(19) == [2, 3, 5, 7, 11, 13, 17, 19]
    a = math_mod.Matrix([[1, 2], [3, 4]])
    b = math_mod.Matrix([[5, 6], [7, 8]])
    assert a * b == math_mod.Matrix([[19, 22], [43, 50]])
    print("complex math ok")

    with pytest.raises(ImportError) as exc:
        __import__("bad_module")
    assert exc.value.__cause__ is not None
    sys.modules.pop("bad_module", None)

    del sys.modules["strict_module"]
    del sys.modules["non_strict"]
    del sys.modules["control_flow"]
    del sys.modules["complex_math"]
    sys.path.remove(str(project_dir))
    print("yellow, world")

if __name__ == "__main__":
    main()


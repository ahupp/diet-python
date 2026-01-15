import os
import pathlib
import subprocess
import sys


def build_ext() -> tuple[pathlib.Path, pathlib.Path]:
    root = pathlib.Path(__file__).resolve().parent.parent
    subprocess.run(["cargo", "build"], check=True)
    build_dir = root.parent / "target" / "debug"
    lib = build_dir / "libsoac_exec.so"
    mod = build_dir / "soac_exec.so"
    if lib.exists() and not mod.exists():
        lib.rename(mod)
    return build_dir, root


def run_project() -> tuple[int, str]:
    build_dir, root = build_ext()
    project_dir = pathlib.Path(__file__).parent / "test_project"
    env = os.environ.copy()
    env["PYTHONPATH"] = f"{build_dir}:{root}"
    result = subprocess.run(
        [sys.executable, "main.py"],
        cwd=project_dir,
        env=env,
        capture_output=True,
        text=True,
    )
    return result.returncode, result.stdout.strip()


def test_project_run() -> None:
    returncode, stdout = run_project()
    assert returncode == 0
    assert stdout == "complex math ok\nyellow, world"

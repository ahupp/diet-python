import subprocess
import sys
from pathlib import Path


def test_import_hook_does_not_import_threading():
    code = "from soac import import_hook; import sys; print('threading' in sys.modules)"
    result = subprocess.run(
        [sys.executable, "-c", code],
        check=True,
        text=True,
        capture_output=True,
    )
    assert result.stdout.strip() == "False"


def test_import_hook_entry_module_bootstraps_runtime():
    module_path = (
        Path(__file__).resolve().parent
        / "integration_modules"
        / "import_hook_entry_bootstrap.py"
    )

    result = subprocess.run(
        [sys.executable, "-m", "soac.import_hook", str(module_path)],
        text=True,
        capture_output=True,
    )
    assert result.returncode == 0, result.stderr
    assert result.stdout.strip() == "1"

import subprocess
import sys


def test_import_hook_does_not_import_threading():
    code = "import diet_import_hook, sys; print('threading' in sys.modules)"
    result = subprocess.run(
        [sys.executable, "-c", code],
        check=True,
        text=True,
        capture_output=True,
    )
    assert result.stdout.strip() == "False"

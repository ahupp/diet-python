from pathlib import Path
import subprocess
import pytest

ROOT = Path(__file__).resolve().parent.parent


def test_example_desugar_up_to_date():
    module = ROOT / "example_module.py"
    with pytest.raises(subprocess.CalledProcessError):
        subprocess.run(
            ["cargo", "run", "--quiet", "--", str(module)],
            cwd=ROOT,
            check=True,
            capture_output=True,
            text=True,
        )

from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parent.parent


def test_example_desugar_up_to_date():
    module = ROOT / "example_module.py"
    result = subprocess.run(
        ["cargo", "run", "--quiet", "--", str(module)],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    assert 'getattr(__dp__, "setattr")' in result.stdout

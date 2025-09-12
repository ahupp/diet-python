from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parent.parent


def test_example_desugar_up_to_date():
    module = ROOT / "example_module.py"
    desugar = ROOT / "example_desugar.py"
    result = subprocess.run(
        ["cargo", "run", "--quiet", "--", str(module)],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    expected = result.stdout
    actual = desugar.read_text()
    if expected != actual:
        desugar.write_text(expected)
        raise AssertionError("example_desugar.py was outdated and has been regenerated")

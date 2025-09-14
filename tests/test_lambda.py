from pathlib import Path
import subprocess
import textwrap

ROOT = Path(__file__).resolve().parent.parent


def test_parameterless_lambda(tmp_path):
    module = tmp_path / "lambda.py"
    module.write_text(
        textwrap.dedent(
            """
            fn = lambda: 1
            """
        )
    )
    result = subprocess.run(
        ["cargo", "run", "--quiet", "--", str(module)],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    assert "def _dp_lambda_1()" in result.stdout

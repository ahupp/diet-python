from __future__ import annotations

import subprocess
from dataclasses import dataclass
from pathlib import Path

import pytest


ROOT = Path(__file__).resolve().parent.parent
MODULES_DIR = Path(__file__).resolve().parent / "integration_modules"


@dataclass(frozen=True)
class DesugaringFixture:
    module_name: str
    module_path: Path
    fixture_path: Path
    source: str
    expected: str


def _module_path_to_slug(module_path: Path) -> str:
    relative = module_path.relative_to(MODULES_DIR).with_suffix("")
    return "__".join(relative.parts)


def _module_name_to_relative_path(module_name: str) -> Path:
    parts = module_name.split(".")
    if parts[-1] == "__init__":
        return Path(*parts[:-1], "__init__.py")
    return Path(*parts).with_suffix(".py")


def _parse_fixture(path: Path) -> tuple[str, str, str]:
    lines = path.read_text(encoding="utf-8").splitlines(keepends=True)
    if not lines:  # pragma: no cover - defensive
        raise AssertionError(f"Fixture {path} is empty")

    header = lines[0].rstrip("\n")
    prefix = "$ desugars "
    if not header.startswith(prefix):
        raise AssertionError(f"Fixture {path} is missing '$ desugars' header")
    module_name = header[len(prefix) :].strip()

    try:
        separator_index = next(i for i, line in enumerate(lines) if line.strip() == "=")
    except StopIteration as exc:  # pragma: no cover - defensive
        raise AssertionError(f"Fixture {path} is missing '=' separator") from exc

    input_lines = lines[1:separator_index]
    if input_lines and input_lines[0].strip() == "":
        input_lines = input_lines[1:]

    source = "".join(input_lines)
    expected = "".join(lines[separator_index + 1 :])

    return module_name, source, expected


def _load_desugaring_fixtures() -> list[DesugaringFixture]:
    fixtures: list[DesugaringFixture] = []
    for module_path in sorted(MODULES_DIR.rglob("*.py")):
        slug = _module_path_to_slug(module_path)
        fixture_path = MODULES_DIR / f"test_{slug}.txt"
        if not fixture_path.exists():
            raise AssertionError(f"Missing desugaring fixture for {module_path}")

        module_name, source, expected = _parse_fixture(fixture_path)

        expected_relative = _module_name_to_relative_path(module_name)
        actual_relative = module_path.relative_to(MODULES_DIR)
        if expected_relative != actual_relative:
            raise AssertionError(
                "Fixture {fixture} points to '{declared}' but should point to '{actual}'".format(
                    fixture=fixture_path,
                    declared=expected_relative,
                    actual=actual_relative,
                )
            )

        module_source = module_path.read_text(encoding="utf-8")
        if module_source != source:
            raise AssertionError(
                "Fixture {fixture} input does not match module source {module}".format(
                    fixture=fixture_path, module=module_path
                )
            )

        fixtures.append(
            DesugaringFixture(
                module_name=module_name,
                module_path=module_path,
                fixture_path=fixture_path,
                source=source,
                expected=expected,
            )
        )

    return fixtures


@pytest.mark.parametrize("fixture", _load_desugaring_fixtures())
def test_integration_desugaring_snapshots_are_current(tmp_path, fixture: DesugaringFixture):
    slug = _module_path_to_slug(fixture.module_path)
    module_file = tmp_path / f"{slug}.py"
    module_file.write_text(fixture.source, encoding="utf-8")

    result = subprocess.run(
        ["cargo", "run", "--quiet", "--", str(module_file)],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    )

    assert (
        result.stdout == fixture.expected
    ), f"Desugaring output for {fixture.fixture_path} is out of date"

from __future__ import annotations

import pytest

import diet_import_hook


SOURCE = """
async def a():
    return 3

async def no_lying():
    for i in range((await a()) + 2):
        yield i
"""

PASS_SHAPE_XFAIL = pytest.mark.xfail(
    reason=(
        "pass boundaries still lower await/yield earlier than the intended "
        "core pass sequence"
    )
)


def debug_pass_shape(pass_name: str) -> dict[str, bool]:
    transform = diet_import_hook._get_pyo3_transform()
    return transform.debug_pass_shape(SOURCE, pass_name, True)


@PASS_SHAPE_XFAIL
def test_semantic_blockpy_still_contains_await() -> None:
    assert debug_pass_shape("semantic_blockpy")["contains_await"]


@PASS_SHAPE_XFAIL
def test_semantic_blockpy_still_contains_yield() -> None:
    assert debug_pass_shape("semantic_blockpy")["contains_yield"]


@PASS_SHAPE_XFAIL
def test_core_blockpy_still_contains_await() -> None:
    assert debug_pass_shape("core_blockpy")["contains_await"]


@PASS_SHAPE_XFAIL
def test_core_blockpy_still_contains_yield() -> None:
    assert debug_pass_shape("core_blockpy")["contains_yield"]


@PASS_SHAPE_XFAIL
def test_core_blockpy_replaces_add_with_dp_add() -> None:
    assert debug_pass_shape("core_blockpy")["contains_dp_add"]


@PASS_SHAPE_XFAIL
def test_core_blockpy_without_await_removes_await() -> None:
    assert not debug_pass_shape("core_blockpy_without_await")["contains_await"]


@PASS_SHAPE_XFAIL
def test_core_blockpy_without_await_or_yield_removes_yield() -> None:
    assert not debug_pass_shape("core_blockpy_without_await_or_yield")["contains_yield"]

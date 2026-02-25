from __future__ import annotations

import __dp__

from tests._integration import integration_module


def test_eval_jit_fastpath_supports_richer_expressions(tmp_path, monkeypatch):
    monkeypatch.setenv("DIET_PYTHON_JIT", "1")
    source = """
def add5(a, b, c, d, e):
    return a + b + c + d + e

def fp_global_name(x):
    return int(x)

def fp_nested_calls(f, g, x):
    return f(g(x))

def fp_many_args(f, a, b, c, d, e):
    return f(a, b, c, d, e)

def fp_float_literal():
    return 1.5

def fp_string_literal():
    return "hello"

def fp_tuple_literal(x):
    return (x, 1, None, True)
"""
    with integration_module(tmp_path, "eval_jit_fastpath_exprs", source, mode="eval") as module:
        assert module.fp_global_name("12") == 12
        assert module.fp_nested_calls(lambda v: v + 1, int, "4") == 5
        assert module.fp_many_args(module.add5, 1, 2, 3, 4, 5) == 15
        assert module.fp_float_literal() == 1.5
        assert module.fp_string_literal() == "hello"
        assert module.fp_tuple_literal("x") == ("x", 1, None, True)


def test_eval_jit_fastpath_exprs_render_helpers(tmp_path, monkeypatch):
    monkeypatch.setenv("DIET_PYTHON_JIT", "1")
    source = """
def add5(a, b, c, d, e):
    return a + b + c + d + e

def fp_global_name(x):
    return int(x)

def fp_many_args(f, a, b, c, d, e):
    return f(a, b, c, d, e)

def fp_float_literal():
    return 1.5

def fp_string_literal():
    return "hello"

def fp_tuple_literal(x):
    return (x, 1, None, True)
"""
    with integration_module(
        tmp_path, "eval_jit_fastpath_exprs_render", source, mode="eval"
    ) as module:
        clif_global_name = __dp__.render_jit_bb(module.fp_global_name)
        clif_many_args = __dp__.render_jit_bb(module.fp_many_args)
        clif_float = __dp__.render_jit_bb(module.fp_float_literal)
        clif_string = __dp__.render_jit_bb(module.fp_string_literal)
        clif_tuple = __dp__.render_jit_bb(module.fp_tuple_literal)

    assert "dp_jit_load_name" in clif_global_name
    assert "call dp_jit_tuple_new" in clif_many_args
    assert "call dp_jit_tuple_set_item" in clif_many_args
    assert "call dp_jit_make_float" in clif_float
    assert "call dp_jit_decode_literal_bytes" in clif_string
    assert "call dp_jit_tuple_new" in clif_tuple

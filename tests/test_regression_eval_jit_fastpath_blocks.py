from __future__ import annotations

import __dp__

from tests._integration import integration_module


def test_eval_jit_fastpath_blocks_are_callable_and_jitted(tmp_path, monkeypatch):
    monkeypatch.setenv("DIET_PYTHON_JIT", "1")
    source = """
events = []

def mark(v):
    events.append(v)

def fp_return_none():
    return

def fp_jump_passthrough(x):
    if x:
        pass
    else:
        return 2
    return 1

def fp_direct_simple_ret_call(fn):
    return fn(1)

def fp_brif(x):
    if x:
        return x
    return x

def fp_expr_ret_none(sink, v):
    sink(v)
    sink(v)
    return

def fp_assign_jump(x):
    y = x
    if x:
        pass
    else:
        pass
    return y
"""
    with integration_module(
        tmp_path, "eval_jit_fastpath_blocks_callable", source, mode="eval"
    ) as module:
        assert __dp__._jit_run_bb_plan is not None
        assert __dp__._jit_render_bb_plan is not None

        original = __dp__._jit_run_bb_plan
        calls: list[object] = []

        def wrapped(plan_module, plan_qualname, globals_dict, args):
            calls.append((plan_module, plan_qualname))
            return original(plan_module, plan_qualname, globals_dict, args)

        __dp__._jit_run_bb_plan = wrapped
        try:
            assert module.fp_return_none() is None
            assert module.fp_jump_passthrough(True) == 1
            assert module.fp_jump_passthrough(False) == 2
            assert module.fp_direct_simple_ret_call(int) == 1
            assert module.fp_brif(True) is True
            assert module.fp_brif(False) is False
            assert module.fp_expr_ret_none(module.mark, 7) is None
            assert module.fp_assign_jump(11) == 11
        finally:
            __dp__._jit_run_bb_plan = original

        assert module.events == [7, 7]
        # CLIF-wrapper execution now dispatches directly from eval-frame using
        # code-extra metadata, so wrapper calls no longer flow through
        # __dp__._jit_run_bb_plan.
        assert len(calls) == 0


def test_eval_jit_fastpath_blocks_render_without_python_block_calls(
    tmp_path, monkeypatch
):
    monkeypatch.setenv("DIET_PYTHON_JIT", "1")
    source = """
events = []

def mark(v):
    events.append(v)

def fp_return_none():
    return

def fp_jump_passthrough(x):
    if x:
        pass
    else:
        return 2
    return 1

def fp_direct_simple_ret_call(fn):
    return fn(1)

def fp_brif(x):
    if x:
        return x
    return x

def fp_expr_ret_none(sink, v):
    sink(v)
    sink(v)
    return

def fp_assign_jump(x):
    y = x
    if x:
        pass
    else:
        pass
    return y
"""
    with integration_module(
        tmp_path, "eval_jit_fastpath_blocks_render", source, mode="eval"
    ) as module:
        clif_return_none = __dp__.render_jit_bb(module.fp_return_none)
        clif_jump = __dp__.render_jit_bb(module.fp_jump_passthrough)
        clif_direct_ret = __dp__.render_jit_bb(module.fp_direct_simple_ret_call)
        clif_brif = __dp__.render_jit_bb(module.fp_brif)
        clif_expr_ret_none = __dp__.render_jit_bb(module.fp_expr_ret_none)
        clif_assign_jump = __dp__.render_jit_bb(module.fp_assign_jump)

    assert "call PyObject_CallObject" not in clif_return_none

    assert "call PyObject_CallFunctionObjArgs" in clif_direct_ret
    assert "call PyObject_CallFunctionObjArgs" in clif_expr_ret_none
    assert "call dp_jit_is_true" in clif_brif
    assert "call dp_jit_term_kind" not in clif_assign_jump
    assert "call PyObject_CallObject" not in clif_assign_jump

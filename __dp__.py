# diet-python: disabled
import collections.abc as _abc
from dataclasses import dataclass
import inspect as _inspect
import os
import operator as _operator
import reprlib
import sys
import builtins
import types as _types
from typing import NamedTuple
import warnings

next = builtins.next
iter = builtins.iter
aiter = builtins.aiter
anext = builtins.anext
isinstance = builtins.isinstance
setattr = builtins.setattr
delattr = builtins.delattr
tuple = builtins.tuple
list = builtins.list
dict = builtins.dict
set = builtins.set
slice = builtins.slice
classmethod = builtins.classmethod


def __deepcopy__(memo):
    # Modules are not pickleable; keep __dp__ as a singleton during deepcopy().
    return sys.modules[__name__]


builtins.__dp__ = sys.modules[__name__]

_MISSING = object()
DELETED = object()
NO_DEFAULT = object()
_GEN_PC_DONE = -1

def load_deleted_name(name, value):
    if value is DELETED:
        raise UnboundLocalError(
            f"cannot access local variable {name!r} where it is not associated with a value"
        )
    return value


def _mro_getattr(cls, name: str):
    for base in cls.__mro__:
        try:
            return base.__dict__[name]
        except KeyError:
            continue
    return _MISSING


def _call_special_method(method, first_obj, *args):
    if first_obj is not None and hasattr(method, "__get__"):
        method = method.__get__(first_obj, type(first_obj))
        return method(*args)
    return method(first_obj, *args)


def _oper(lhs_method_name: str, rhs_method_name: str, op_symbol: str, lhs, rhs):

    lhs_type = type(lhs)
    lhs_method = _mro_getattr(lhs_type, lhs_method_name)
    lhs_rmethod = _mro_getattr(lhs_type, rhs_method_name)

    rhs_type = type(rhs)
    rhs_method = _mro_getattr(rhs_type, rhs_method_name)

    call_lhs = (lhs, lhs_method, rhs)
    call_rhs = (rhs, rhs_method, lhs)
    if (
        rhs_method is not _MISSING
        and rhs_type is not lhs_type
        and issubclass(rhs_type, lhs_type)
        and lhs_rmethod is not rhs_method
    ):
        calls = (call_rhs, call_lhs)
    elif lhs_type is not rhs_type:
        calls = (call_lhs, call_rhs)
    else:
        calls = (call_lhs,)

    for first_obj, method, second_obj in calls:
        if method is _MISSING:
            continue
        value = _call_special_method(method, first_obj, second_obj)
        if value is not NotImplemented:
            return value

    raise TypeError(
        f"unsupported operand type(s) for {op_symbol}: "
        f"'{lhs_type.__name__}' and '{rhs_type.__name__}'"
    )


def _pow_with_mod(lhs, rhs, mod):
    lhs_type = type(lhs)
    lhs_method = _mro_getattr(lhs_type, "__pow__")
    lhs_rmethod = _mro_getattr(lhs_type, "__rpow__")

    rhs_type = type(rhs)
    rhs_method = _mro_getattr(rhs_type, "__rpow__")

    call_lhs = (lhs, lhs_method, rhs, mod)
    call_rhs = (rhs, rhs_method, lhs, mod)
    if (
        rhs_method is not _MISSING
        and rhs_type is not lhs_type
        and issubclass(rhs_type, lhs_type)
        and lhs_rmethod is not rhs_method
    ):
        calls = (call_rhs, call_lhs)
    elif lhs_type is not rhs_type:
        calls = (call_lhs, call_rhs)
    else:
        calls = (call_lhs,)

    for first_obj, method, second_obj, third_obj in calls:
        if method is _MISSING:
            continue
        value = _call_special_method(method, first_obj, second_obj, third_obj)
        if value is not NotImplemented:
            return value

    raise TypeError(
        f"unsupported operand type(s) for **: "
        f"'{lhs_type.__name__}' and '{rhs_type.__name__}'"
    )


def _ioper(inplace_name: str, lhs_method_name: str, rhs_method_name: str, op_symbol: str, lhs, rhs):
    method = _mro_getattr(type(lhs), inplace_name)
    if method is not _MISSING:
        value = _call_special_method(method, lhs, rhs)
        if value is not NotImplemented:
            return value
    return _oper(lhs_method_name, rhs_method_name, op_symbol, lhs, rhs)


def add(lhs, rhs):
    return _oper("__add__", "__radd__", "+", lhs, rhs)


def sub(lhs, rhs):
    return _oper("__sub__", "__rsub__", "-", lhs, rhs)


def mul(lhs, rhs):
    return _oper("__mul__", "__rmul__", "*", lhs, rhs)


def matmul(lhs, rhs):
    return _oper("__matmul__", "__rmatmul__", "@", lhs, rhs)


def truediv(lhs, rhs):
    return _oper("__truediv__", "__rtruediv__", "/", lhs, rhs)


def floordiv(lhs, rhs):
    return _oper("__floordiv__", "__rfloordiv__", "//", lhs, rhs)


def mod(lhs, rhs):
    return _oper("__mod__", "__rmod__", "%", lhs, rhs)


def pow(lhs, rhs, mod=None):
    if mod is None:
        return _oper("__pow__", "__rpow__", "**", lhs, rhs)
    return _pow_with_mod(lhs, rhs, mod)


def lshift(lhs, rhs):
    return _oper("__lshift__", "__rlshift__", "<<", lhs, rhs)


def rshift(lhs, rhs):
    return _oper("__rshift__", "__rrshift__", ">>", lhs, rhs)


def or_(lhs, rhs):
    return _oper("__or__", "__ror__", "|", lhs, rhs)


def xor(lhs, rhs):
    return _oper("__xor__", "__rxor__", "^", lhs, rhs)


def and_(lhs, rhs):
    return _oper("__and__", "__rand__", "&", lhs, rhs)


def iadd(lhs, rhs):
    return _ioper("__iadd__", "__add__", "__radd__", "+", lhs, rhs)


def isub(lhs, rhs):
    return _ioper("__isub__", "__sub__", "__rsub__", "-", lhs, rhs)


def imul(lhs, rhs):
    return _ioper("__imul__", "__mul__", "__rmul__", "*", lhs, rhs)


def imatmul(lhs, rhs):
    return _ioper("__imatmul__", "__matmul__", "__rmatmul__", "@", lhs, rhs)


def itruediv(lhs, rhs):
    return _ioper("__itruediv__", "__truediv__", "__rtruediv__", "/", lhs, rhs)


def imod(lhs, rhs):
    return _ioper("__imod__", "__mod__", "__rmod__", "%", lhs, rhs)


def ipow(lhs, rhs, mod=None):
    if mod is not None:
        return _pow_with_mod(lhs, rhs, mod)
    return _ioper("__ipow__", "__pow__", "__rpow__", "**", lhs, rhs)


def ilshift(lhs, rhs):
    return _ioper("__ilshift__", "__lshift__", "__rlshift__", "<<", lhs, rhs)


def irshift(lhs, rhs):
    return _ioper("__irshift__", "__rshift__", "__rrshift__", ">>", lhs, rhs)


def ior(lhs, rhs):
    return _ioper("__ior__", "__or__", "__ror__", "|", lhs, rhs)


def ixor(lhs, rhs):
    return _ioper("__ixor__", "__xor__", "__rxor__", "^", lhs, rhs)


def iand(lhs, rhs):
    return _ioper("__iand__", "__and__", "__rand__", "&", lhs, rhs)


def ifloordiv(lhs, rhs):
    return _ioper("__ifloordiv__", "__floordiv__", "__rfloordiv__", "//", lhs, rhs)



def pos(value):
    return +value


def neg(value):
    return -value


def invert(value):
    return ~value


def not_(value):
    return not bool(value)


def truth(value):
    return bool(value)



class _JumpTerm(NamedTuple):
    target: object
    args: tuple


class _RetTerm(NamedTuple):
    value: object = None


class _RaiseTerm(NamedTuple):
    exc: BaseException


@dataclass(slots=True)
class BlockParam:
    args: list
    index: int

    def take(self):
        value = self.args[self.index]
        self.args[self.index] = None
        return value


def jump(target, args):
    return _JumpTerm(target=target, args=args)


def ret(value=None):
    return _RetTerm(value=value)


def raise_(exc):
    return _RaiseTerm(exc=exc)


def brif(cond, then_target, then_args, else_target, else_args):
    if truth(cond):
        return jump(then_target, then_args)
    return jump(else_target, else_args)


_BR_TABLE_NO_ARGS = object()


def br_table(index, targets, default_target, args=_BR_TABLE_NO_ARGS):
    if not isinstance(index, int):
        target = default_target
    elif index < 0 or index >= len(targets):
        target = default_target
    else:
        target = targets[index]
    if args is _BR_TABLE_NO_ARGS:
        return target
    return jump(target, args)


def _block_params(args, *, copy_args=True):
    if args is None:
        return ()
    # Most blocks do not need entry-arg preservation for exception-edge
    # dispatch; let BlockParam.take() clear the live list so values are not
    # retained across in-block operations like gc.collect().
    if copy_args:
        args = list(args)
    elif not isinstance(args, list):
        args = list(args)
    return tuple(BlockParam(args, idx) for idx in range(len(args)))


def _resolve_local_frame(owner):
    frame = getattr(owner, "gi_frame", None)
    if frame is not None:
        return frame
    return owner


def __dp_load_local(gen, name):
    frame = _resolve_local_frame(gen)
    try:
        value = frame[name]
    except KeyError as exc:
        raise UnboundLocalError(
            f"cannot access local variable {name!r} where it is not associated with a value"
        ) from exc
    return load_deleted_name(name, value)


def __dp_load_local_raw(gen, name):
    frame = _resolve_local_frame(gen)
    return frame.get(name, DELETED)


def __dp_store_local(gen, name, value):
    frame = _resolve_local_frame(gen)
    frame[name] = value
    return value


def __dp_del_local(gen, name):
    frame = _resolve_local_frame(gen)
    frame[name] = DELETED
    return DELETED


builtins.__dp_load_local = __dp_load_local
builtins.__dp_load_local_raw = __dp_load_local_raw
builtins.__dp_store_local = __dp_store_local
builtins.__dp_del_local = __dp_del_local


def _block_param_names(block):
    code = block.__code__
    return code.co_varnames[: code.co_argcount]


def raise_uncaught_generator_exception(exc):
    if isinstance(exc, StopIteration):
        raise RuntimeError("generator raised StopIteration") from exc
    raise exc


def raise_uncaught_async_generator_exception(exc):
    if isinstance(exc, StopIteration):
        raise RuntimeError("async generator raised StopIteration") from exc
    if isinstance(exc, StopAsyncIteration):
        raise RuntimeError("async generator raised StopAsyncIteration") from exc
    raise exc




def _attach_throw_context_from_state(state, exc):
    if exc.__context__ is not None:
        return
    try:
        frame = getattr(state, "gi_frame", None)
        if isinstance(frame, dict):
            for candidate in reversed(list(frame.values())):
                if isinstance(candidate, BaseException):
                    exc.__context__ = candidate
                    break
    except Exception:
        pass


def _bb_raw_args(args):
    if args is None:
        return []
    if isinstance(args, list):
        return args
    if isinstance(args, tuple):
        return list(args)
    return list(args)


def _dispatch_block_exception(block, raw_args, exc):
    # Exception edges are attached per block by the BB lowering pass.
    # Dispatch uses name-based argument reconstruction so control flow is
    # encoded in block metadata rather than a runtime try-handler stack.
    target = getattr(block, "_dp_exc_target", None)
    if target is None:
        return None
    if not callable(target):
        raise RuntimeError(f"invalid exception target on block {block!r}: {target!r}")
    source_names = _block_param_names(block)
    if len(source_names) != len(raw_args):
        raise RuntimeError(
            f"basic-block exception dispatch arity mismatch in {block!r}: "
            f"params={source_names!r}, args={raw_args!r}"
        )
    values = dict(zip(source_names, raw_args))
    exc_name = getattr(block, "_dp_exc_name", None)
    if exc_name is not None:
        values[exc_name] = exc
    # Exception-edge dispatch consumes injected resume exceptions. Once routed
    # onto a block-local exception edge, the exception should travel only via
    # the explicit exception slot (exc_name), not via _dp_resume_exc.
    if "_dp_resume_exc" in values:
        values["_dp_resume_exc"] = None
    frame_owner = values.get("_dp_self", values.get("_dp_state"))

    target_names = _block_param_names(target)
    target_args = []
    for name in target_names:
        if name in values:
            target_args.append(values[name])
            continue
        if frame_owner is not None:
            target_args.append(__dp_load_local_raw(frame_owner, name))
            continue
        raise RuntimeError(
            f"missing exception-edge argument {name!r} when dispatching from "
            f"{block!r} to {target!r}"
        )
    return jump(target, target_args)


def _route_region_jump(target, next_args, region_targets):
    if region_targets is None or target in region_targets:
        return target, _bb_raw_args(next_args)
    return None


_jit_run_bb = None
_jit_render_bb = None


def _run_bb_interpreted(entry, args):
    term_obj = run_bb_term(entry, args, None)
    if isinstance(term_obj, _RetTerm):
        return term_obj.value
    if isinstance(term_obj, _RaiseTerm):
        raise term_obj.exc
    if isinstance(term_obj, _JumpTerm):
        raise RuntimeError(f"unexpected out-of-region jump in run_bb: {term_obj!r}")
    raise RuntimeError(f"invalid basic-block terminator: {term_obj!r}")


def _run_bb_step(block, args):
    raw_args = _bb_raw_args(args)
    try:
        preserve_entry_args = getattr(block, "_dp_exc_target", None) is not None
        block_params = _block_params(raw_args, copy_args=preserve_entry_args)
        term_obj = block(*block_params)
    except BaseException as exc:
        term_obj = _dispatch_block_exception(block, raw_args, exc)
        if term_obj is None:
            return raise_(exc)

    while isinstance(term_obj, _RaiseTerm):
        dispatched = _dispatch_block_exception(block, raw_args, term_obj.exc)
        if dispatched is None:
            return term_obj
        term_obj = dispatched
    return term_obj


def _bb_term_kind(term):
    if isinstance(term, _JumpTerm):
        return 0
    if isinstance(term, _RetTerm):
        return 1
    if isinstance(term, _RaiseTerm):
        return 2
    return -1


def _bb_term_jump_target(term):
    return term.target


def _bb_term_jump_args(term):
    return term.args


def _bb_term_ret_value(term):
    return term.value


def _bb_term_raise(term):
    raise term.exc


def _bb_term_invalid(term):
    raise RuntimeError(f"invalid basic-block terminator: {term!r}")


def _bb_resolve_blocks(entry, labels):
    if not isinstance(labels, tuple):
        labels = tuple(labels)
    if not labels:
        return ()

    cached_labels = getattr(entry, "_dp_jit_labels", None)
    cached_blocks = getattr(entry, "_dp_jit_blocks", None)
    if cached_labels == labels and isinstance(cached_blocks, tuple) and len(cached_blocks) == len(labels):
        return cached_blocks

    wanted = set(labels)
    found = dict(())
    seen_ids = set()
    stack = [entry]
    while stack:
        fn = stack.pop()
        obj_id = id(fn)
        if obj_id in seen_ids:
            continue
        seen_ids.add(obj_id)
        name = getattr(fn, "__name__", None)
        if isinstance(name, str) and name in wanted and callable(fn):
            found[name] = fn
            if len(found) == len(wanted):
                break
        closure = getattr(fn, "__closure__", None)
        if closure:
            for cell in closure:
                try:
                    cell_value = cell.cell_contents
                except ValueError:
                    continue
                if callable(cell_value):
                    stack.append(cell_value)
        defaults = getattr(fn, "__defaults__", None)
        if defaults:
            for value in defaults:
                if callable(value):
                    stack.append(value)
        kwdefaults = getattr(fn, "__kwdefaults__", None)
        if isinstance(kwdefaults, dict):
            for value in kwdefaults.values():
                if callable(value):
                    stack.append(value)

    if len(found) != len(wanted):
        globals_dict = getattr(entry, "__globals__", None)
        if isinstance(globals_dict, dict):
            for name in labels:
                if name in found:
                    continue
                value = globals_dict.get(name, None)
                if callable(value):
                    found[name] = value

    missing = [name for name in labels if name not in found]
    if missing:
        raise RuntimeError(
            f"failed to resolve basic blocks for {entry!r}: missing {missing!r}"
        )

    resolved = tuple(found[name] for name in labels)
    entry._dp_jit_labels = labels
    entry._dp_jit_blocks = resolved
    return resolved


def run_bb(entry, args):
    if _jit_run_bb is not None:
        return _jit_run_bb(entry, args)
    return _run_bb_interpreted(entry, args)


def render_jit_bb(entry):
    if _jit_render_bb is None:
        raise RuntimeError("JIT CLIF renderer is unavailable")
    return _jit_render_bb(entry)


async def run_coro_bb(entry, args):
    term_obj = await run_coro_bb_term(entry, args, None)
    if isinstance(term_obj, _RetTerm):
        return term_obj.value
    if isinstance(term_obj, _RaiseTerm):
        raise term_obj.exc
    if isinstance(term_obj, _JumpTerm):
        raise RuntimeError(f"unexpected out-of-region jump in run_coro_bb: {term_obj!r}")
    raise RuntimeError(f"invalid basic-block terminator: {term_obj!r}")


async def run_coro_bb_term(entry, args, region_targets):
    block = entry
    raw_args = _bb_raw_args(args)

    while True:
        assert callable(block), f"invalid basic-block target: {block!r}"

        term_obj = None
        try:
            preserve_entry_args = getattr(block, "_dp_exc_target", None) is not None
            block_params = _block_params(raw_args, copy_args=preserve_entry_args)
            term_obj = await block(*block_params)
        except BaseException as exc:
            # BB regions execute as independent async coroutine functions.
            # If a block leaks StopIteration/StopAsyncIteration across that
            # async boundary, CPython converts it to RuntimeError with the
            # original stop exception in __cause__. Recover the original
            # exception so region-level try/except matching sees the same
            # semantics as in-function execution.
            if isinstance(exc, RuntimeError) and isinstance(exc.__cause__, (StopIteration, StopAsyncIteration)):
                term_obj = raise_(exc.__cause__)
            else:
                term_obj = _dispatch_block_exception(block, raw_args, exc)
                if term_obj is None:
                    term_obj = raise_(exc)
        while True:
            if isinstance(term_obj, _JumpTerm):
                routed = _route_region_jump(term_obj.target, term_obj.args, region_targets)
                if routed is not None:
                    block, raw_args = routed
                    break
                return term_obj
            if isinstance(term_obj, _RaiseTerm):
                raise_term = term_obj
                term_obj = _dispatch_block_exception(block, raw_args, raise_term.exc)
                if term_obj is not None:
                    continue
                return raise_term
            if isinstance(term_obj, _RetTerm):
                return term_obj
            raise RuntimeError(f"invalid basic-block terminator: {term_obj!r}")


def run_bb_term(entry, args, region_targets):
    block = entry
    raw_args = _bb_raw_args(args)

    while True:
        assert callable(block), f"invalid basic-block target: {block!r}"

        term_obj = None
        try:
            preserve_entry_args = getattr(block, "_dp_exc_target", None) is not None
            block_params = _block_params(raw_args, copy_args=preserve_entry_args)
            term_obj = block(*block_params)
        except BaseException as exc:
            term_obj = _dispatch_block_exception(block, raw_args, exc)
            if term_obj is None:
                return raise_(exc)
        while True:
            if isinstance(term_obj, _JumpTerm):
                routed = _route_region_jump(term_obj.target, term_obj.args, region_targets)
                if routed is not None:
                    block, raw_args = routed
                    break
                return term_obj
            if isinstance(term_obj, _RaiseTerm):
                raise_term = term_obj
                term_obj = _dispatch_block_exception(block, raw_args, raise_term.exc)
                if term_obj is not None:
                    continue
                return raise_term
            if isinstance(term_obj, _RetTerm):
                return term_obj
            raise RuntimeError(f"invalid basic-block terminator: {term_obj!r}")




class _DpGenerator:
    __slots__ = (
        "_dp_resume",
        "_pc",
        "__name__",
        "__qualname__",
        "gi_frame",
        "gi_yieldfrom",
        "gi_code",
    )

    def __init__(
        self,
        *,
        resume,
        pc,
        gi_frame,
        name,
        qualname,
        code,
    ):
        self._dp_resume = resume
        self._pc = pc
        self.gi_frame = gi_frame
        self.gi_yieldfrom = None
        self.__name__ = name
        self.__qualname__ = qualname
        self.gi_code = code

    def __iter__(self):
        return self

    def __next__(self):
        return self.send(None)

    def _resume(self, value, resume_exc):
        return run_bb(self._dp_resume, (self, value, resume_exc))

    def send(self, value):
        return self._resume(value, None)

    def throw(self, typ=None, val=None, tb=None):
        if val is not None or tb is not None:
            raise TypeError("DpGen.throw() does not support value/traceback in this mode")
        exc = raise_from(typ, None)
        _attach_throw_context_from_state(self, exc)
        return self._resume(None, exc)

    def close(self):
        try:
            self.throw(GeneratorExit)
        except (GeneratorExit, StopIteration):
            return None
        raise RuntimeError("generator ignored GeneratorExit")


class _DpAsyncGenerator:
    __slots__ = (
        "_dp_resume",
        "_pc",
        "_dp_transport_sent",
        "__name__",
        "__qualname__",
        "gi_frame",
        "gi_yieldfrom",
        "ag_code",
    )

    def __init__(
        self,
        *,
        resume,
        pc,
        gi_frame,
        name,
        qualname,
        code,
    ):
        self._dp_resume = resume
        self._pc = pc
        self._dp_transport_sent = None
        self.__name__ = name
        self.__qualname__ = qualname
        self.gi_frame = gi_frame
        self.gi_yieldfrom = None
        self.ag_code = code

    def __aiter__(self):
        return self

    def __anext__(self):
        return self.asend(None)

    def __getattr__(self, name):
        if name == "ag_running":
            return False
        if name == "ag_frame":
            return self.gi_frame
        if name == "ag_await":
            return None
        raise AttributeError(name)

    async def _resume(self, value, resume_exc, transport_sent):
        self._dp_resume._dp_transport_sent = transport_sent
        return await run_coro_bb(self._dp_resume, (self, value, resume_exc))

    def asend(self, value):
        return _DpAsyncGenSend(self, value)

    async def athrow(self, typ=None, val=None, tb=None):
        if val is not None or tb is not None:
            raise TypeError("DpAsyncGen.athrow() does not support value/traceback in this mode")
        exc = raise_from(typ, None)
        _attach_throw_context_from_state(self, exc)
        return await self._resume(None, exc, None)

    async def aclose(self):
        try:
            await self.athrow(GeneratorExit)
        except (GeneratorExit, StopAsyncIteration):
            return None
        raise RuntimeError("async generator ignored GeneratorExit")


class _DpAsyncGenSend:
    __slots__ = ("_dp_gen", "_dp_value", "_dp_coro")

    def __init__(self, gen, value):
        self._dp_gen = gen
        self._dp_value = value
        self._dp_coro = None

    def __iter__(self):
        return self

    def __await__(self):
        return self

    def __next__(self):
        return self.send(None)

    def send(self, value):
        if self._dp_coro is None:
            initial_send = self._dp_value
            if (
                initial_send is None
                and value is not None
                and getattr(self._dp_gen, "_pc", 0) == 0
            ):
                initial_send = value
            self._dp_coro = self._dp_gen._resume(
                initial_send,
                None,
                value,
            )
            return self._dp_coro.send(None)
        return self._dp_coro.send(value)

    def throw(self, typ, val=None, tb=None):
        if self._dp_coro is None:
            self._dp_coro = self._dp_gen._resume(self._dp_value, None, None)
        return self._dp_coro.throw(typ, val, tb)

    def close(self):
        if self._dp_coro is not None:
            return self._dp_coro.close()
        return None


def _rich_compare(lhs_method_name: str, rhs_method_name: str, lhs, rhs):
    lhs_type = type(lhs)
    lhs_method = _mro_getattr(lhs_type, lhs_method_name)
    lhs_rmethod = _mro_getattr(lhs_type, rhs_method_name)

    rhs_type = type(rhs)
    rhs_method = _mro_getattr(rhs_type, rhs_method_name)

    call_lhs = (lhs, lhs_method, rhs)
    call_rhs = (rhs, rhs_method, lhs)
    if (
        rhs_method is not _MISSING
        and rhs_type is not lhs_type
        and issubclass(rhs_type, lhs_type)
        and lhs_rmethod is not rhs_method
    ):
        calls = (call_rhs, call_lhs)
    elif lhs_type is not rhs_type:
        calls = (call_lhs, call_rhs)
    else:
        calls = (call_lhs,)

    for first_obj, method, second_obj in calls:
        if method is _MISSING:
            continue
        value = _call_special_method(method, first_obj, second_obj)
        if value is not NotImplemented:
            return value

    return NotImplemented


def _rich_compare_error(lhs_method_name: str, rhs_method_name: str, lhs, rhs):
    cmp = _rich_compare(lhs_method_name, rhs_method_name, lhs, rhs)
    if cmp is NotImplemented:
        raise TypeError(
            f"'{type(lhs).__name__}' not supported between instances of "
            f"'{type(lhs).__name__}' and '{type(rhs).__name__}'"
        )

    return cmp


def eq(lhs, rhs):
    cmp = _rich_compare("__eq__", "__eq__", lhs, rhs)
    if cmp is NotImplemented:
        return False
    return cmp


def ne(lhs, rhs):
    cmp = _rich_compare("__ne__", "__ne__", lhs, rhs)
    if cmp is NotImplemented:
        return True
    return cmp


def lt(lhs, rhs):
    return _rich_compare_error("__lt__", "__gt__", lhs, rhs)


def le(lhs, rhs):
    return _rich_compare_error("__le__", "__ge__", lhs, rhs)


def gt(lhs, rhs):
    return _rich_compare_error("__gt__", "__lt__", lhs, rhs)


def ge(lhs, rhs):
    return _rich_compare_error("__ge__", "__le__", lhs, rhs)


is_ = _operator.is_


is_not = _operator.is_not


def contains(container, item):
    return item in container


def getitem(obj, key):
    return obj[key]


def setitem(obj, key, value):
    if obj is DELETED:
        raise builtins.UnboundLocalError(
            "cannot access local variable before assignment"
        )
    obj[key] = value


def delitem(obj, key):
    del obj[key]


def delitem_quietly(obj, key):
    # Used for `except ... as name` cleanup where CPython `del name` must be
    # silent if the binding was already removed in handler code.
    try:
        del obj[key]
    except (NameError, KeyError):
        pass


# TODO: very questionable
def float_from_literal(literal):
    # Preserve CPython's literal parsing for values that Rust rounds differently.
    return float(literal.replace("_", ""))



_DP_CELL_PREFIX = "_dp_cell_"



def class_lookup_cell(class_ns, name, cell):
    try:
        return class_ns[name]
    except KeyError:
        pass
    try:
        return load_cell(cell)
    except UnboundLocalError as exc:
        raise NameError(
            f"cannot access free variable {name!r} where it is not associated with a value in enclosing scope"
        ) from exc


def class_lookup_global(class_ns, name, globals_dict):
    try:
        return class_ns[name]
    except KeyError:
        try:
            return globals_dict[name]
        except KeyError:
            try:
                return builtins.__dict__[name]
            except KeyError as exc:
                raise NameError(f"name {name!r} is not defined") from exc


def _validate_exception_type(exc_type):
    if isinstance(exc_type, tuple):
        for entry in exc_type:
            _validate_exception_type(entry)
        return
    if isinstance(exc_type, type) and issubclass(exc_type, BaseException):
        return
    raise TypeError("catching classes that do not inherit from BaseException is not allowed")


def exception_matches(exc, exc_type):
    if isinstance(exc, RecursionError):
        return isinstance(exc, exc_type)
    _validate_exception_type(exc_type)
    return isinstance(exc, exc_type)

def exceptiongroup_split(exc, exc_type):
    _validate_exception_type(exc_type)
    if isinstance(exc, BaseExceptionGroup):
        match, rest = exc.split(exc_type)
        return match, rest
    if isinstance(exc, exc_type):
        return exc, None
    return None, exc


def unpack(iterable, spec):
    try:
        iterator = iter(iterable)
    except TypeError as exc:
        raise TypeError(
            f"cannot unpack non-iterable {type(iterable).__name__} object"
        ) from exc

    result = []
    star_index = None

    for idx, flag in enumerate(spec):
        if flag:
            try:
                result.append(next(iterator))
            except StopIteration as exc:
                raise ValueError from exc
        else:
            if star_index is not None:
                raise ValueError("only one starred target is supported")
            star_index = idx
            break

    if star_index is None:
        try:
            next(iterator)
        except StopIteration:
            return tuple(result)
        raise ValueError

    suffix_flags = list(spec[star_index + 1 :])
    if not all(suffix_flags):
        raise ValueError("only one starred target is supported")

    remainder = list(iterator)
    suffix_count = len(suffix_flags)

    if len(remainder) < suffix_count:
        raise ValueError

    if suffix_count:
        tail = remainder[-suffix_count:]
        remainder = remainder[:-suffix_count]
    else:
        tail = []

    result.append(remainder)
    result.extend(tail)
    return tuple(result)



def make_cell(value=_MISSING):
    cell = _types.CellType()
    if value is not _MISSING:
        cell.cell_contents = value
    return cell

def load_cell(cell):
    if not isinstance(cell, _types.CellType):
        raise TypeError("expected cell")
    try:
        return cell.cell_contents
    except ValueError as exc:
        raise UnboundLocalError("local variable referenced before assignment") from exc

class LocalsProxy:
    def __init__(self, frame):
        self.frame = frame

    def __getitem__(self, name):
        return self.frame.f_locals[name]

    def __setitem__(self, name, value):
        self.frame.f_locals[name] = value

def _normalize_mapping(values):
    result = {}
    cell_overrides = {}
    for name, value in values.items():
        if not isinstance(name, str):
            result[name] = value
            continue
        if name.startswith(_DP_CELL_PREFIX):
            base = name[len(_DP_CELL_PREFIX):]
            if isinstance(value, _types.CellType):
                try:
                    cell_overrides[base] = value.cell_contents
                except ValueError:
                    pass
            continue
        if name.startswith("_dp_"):
            continue
        if isinstance(value, _types.CellType):
            try:
                result[name] = value.cell_contents
            except ValueError:
                continue
        else:
            result[name] = value
    for name, value in cell_overrides.items():
        result[name] = value
    return result


def _lookup_normalized_name(mapping, name, *, hide_internal=False):
    if hide_internal and isinstance(name, str) and name.startswith("_dp_"):
        raise KeyError(name)
    if isinstance(name, str):
        if not name.startswith(_DP_CELL_PREFIX):
            cell_name = _DP_CELL_PREFIX + name
            if cell_name in mapping:
                value = mapping[cell_name]
                if isinstance(value, _types.CellType):
                    try:
                        return value.cell_contents
                    except ValueError:
                        raise KeyError(name)
                return value
        if name in mapping:
            value = mapping[name]
            if isinstance(value, _types.CellType):
                try:
                    return value.cell_contents
                except ValueError:
                    raise KeyError(name)
            return value
        raise KeyError(name)
    return mapping[name]


class _NormalizedMappingProxy(_abc.MutableMapping):
    def _mapping(self):
        raise NotImplementedError

    def _hide_internal_name(self, name):
        return False

    def _snapshot(self):
        return _normalize_mapping(self._mapping())

    def __getitem__(self, name):
        return _lookup_normalized_name(
            self._mapping(),
            name,
            hide_internal=self._hide_internal_name(name),
        )

    def __iter__(self):
        return iter(self._snapshot())

    def __len__(self):
        return len(self._snapshot())

    def __contains__(self, name):
        try:
            self[name]
        except KeyError:
            return False
        return True

    def keys(self):
        return self._snapshot().keys()

    def values(self):
        return self._snapshot().values()

    def items(self):
        return self._snapshot().items()


class FrameLocalsProxy(_NormalizedMappingProxy):
    def __init__(self, frame_or_mapping, /):
        if isinstance(frame_or_mapping, _types.FrameType):
            self.frame = frame_or_mapping
            self._mapping_override = None
            return
        if isinstance(frame_or_mapping, dict):
            self.frame = None
            self._mapping_override = frame_or_mapping
            return
        raise TypeError("expected a frame object")

    def _mapping(self):
        if self._mapping_override is not None:
            return self._mapping_override
        return self.frame.f_locals

    def _is_local_name(self, name):
        if not isinstance(name, str):
            return False
        if self.frame is None:
            return False
        code = self.frame.f_code
        if (
            name in code.co_varnames
            or name in code.co_cellvars
            or name in code.co_freevars
        ):
            return True
        cell_name = _DP_CELL_PREFIX + name
        return (
            cell_name in code.co_varnames
            or cell_name in code.co_cellvars
            or cell_name in code.co_freevars
        )

    def _snapshot(self):
        return _normalize_mapping(self._mapping())

    def __setitem__(self, name, value):
        locals_map = self._mapping()
        if isinstance(name, str) and not name.startswith(_DP_CELL_PREFIX):
            cell_name = _DP_CELL_PREFIX + name
            if cell_name in locals_map and isinstance(locals_map[cell_name], _types.CellType):
                locals_map[cell_name].cell_contents = value
                return
        locals_map[name] = value

    def __delitem__(self, name):
        if self._is_local_name(name):
            raise ValueError("cannot remove local variables from FrameLocalsProxy")
        locals_map = self._mapping()
        if isinstance(name, str) and not name.startswith(_DP_CELL_PREFIX):
            cell_name = _DP_CELL_PREFIX + name
            if cell_name in locals_map:
                value = locals_map[cell_name]
                if isinstance(value, _types.CellType):
                    try:
                        del value.cell_contents
                        return
                    except ValueError:
                        pass
        del locals_map[name]

    @reprlib.recursive_repr("{...}")
    def __repr__(self):
        return repr(self._snapshot())

    def __reversed__(self):
        return list(reversed(self._snapshot().keys()))

    def keys(self):
        return self._snapshot().keys()

    def values(self):
        return self._snapshot().values()

    def items(self):
        return self._snapshot().items()

    def clear(self):
        raise AttributeError("FrameLocalsProxy object has no attribute 'clear'")

    def __or__(self, other):
        if isinstance(other, _NormalizedMappingProxy):
            other = other._snapshot()
        if not isinstance(other, dict):
            try:
                other = dict(other)
            except Exception:
                return NotImplemented
        result = self._snapshot()
        result.update(other)
        return result

    def __ror__(self, other):
        if not isinstance(other, dict):
            try:
                other = dict(other)
            except Exception:
                return NotImplemented
        result = dict(other)
        result.update(self._snapshot())
        return result

    def __ior__(self, other):
        self.update(other)
        return self

    def copy(self):
        return dict(self._snapshot())

class GlobalsProxy(_NormalizedMappingProxy):
    def __init__(self, globals_dict):
        self._globals = globals_dict

    def _mapping(self):
        return self._globals

    def _hide_internal_name(self, name):
        return isinstance(name, str) and name.startswith("_dp_")

    __getitem__ = _NormalizedMappingProxy.__getitem__
    __iter__ = _NormalizedMappingProxy.__iter__
    __len__ = _NormalizedMappingProxy.__len__
    __contains__ = _NormalizedMappingProxy.__contains__
    keys = _NormalizedMappingProxy.keys
    values = _NormalizedMappingProxy.values
    items = _NormalizedMappingProxy.items

    def __setitem__(self, name, value):
        self._globals[name] = value

    def __delitem__(self, name):
        del self._globals[name]

def locals():
    frame = sys._getframe(1)
    return _normalize_mapping(frame.f_locals)

def frame_locals(frame):
    if frame is not None:
        locals_map = frame.f_locals
        class_ns = None
        try:
            if "_dp_class_ns" in locals_map:
                class_ns = locals_map["_dp_class_ns"]
        except Exception:
            class_ns = None
        if isinstance(class_ns, dict):
            return GlobalsProxy(class_ns)
        if (
            frame.f_back is not None
            and frame.f_code.co_name in {"<listcomp>", "<setcomp>", "<dictcomp>", "<genexpr>"}
        ):
            frame = frame.f_back
    return FrameLocalsProxy(frame)

def globals():
    frame = sys._getframe(1)
    return frame.f_globals

def dir_(*args):
    if args:
        return builtins.dir(*args)
    frame = sys._getframe(1)
    names = _normalize_mapping(frame.f_locals).keys()
    return sorted(name for name in names if not name.startswith("_dp_"))

def eval_(source, globals=None, locals=None):
    if globals is None or locals is None:
        frame = sys._getframe(1)
        if globals is None:
            globals = frame.f_globals
        if locals is None:
            locals = _normalize_mapping(frame.f_locals)
    return builtins.eval(source, globals, locals)

def _normalize_exec_closure(closure):
    mutated = []
    try:
        iterator = iter(closure)
    except TypeError:
        return mutated
    for cell in iterator:
        if not isinstance(cell, _types.CellType):
            continue
        try:
            contents = cell.cell_contents
        except ValueError:
            inner = make_cell()
            cell.cell_contents = inner
            mutated.append((cell, inner))
            continue
        if isinstance(contents, _types.CellType):
            continue
        inner = make_cell(contents)
        cell.cell_contents = inner
        mutated.append((cell, inner))
    return mutated


def _restore_exec_closure(mutated):
    for cell, inner in mutated:
        try:
            cell.cell_contents = inner.cell_contents
        except ValueError:
            try:
                del cell.cell_contents
            except ValueError:
                pass


def exec_(source, globals=None, locals=None, *, closure=None):
    mutated = []
    if closure is not None:
        mutated = _normalize_exec_closure(closure)
    if globals is None:
        try:
            frame = sys._getframe(1)
            globals = frame.f_globals
            if locals is None:
                locals = _normalize_mapping(frame.f_locals)
            if closure is None:
                return builtins.exec(source, globals, locals)
            return builtins.exec(source, globals, locals, closure=closure)
        finally:
            _restore_exec_closure(mutated)
    if isinstance(globals, GlobalsProxy):
        globals = globals._globals
    if locals is None:
        try:
            if closure is None:
                return builtins.exec(source, globals)
            return builtins.exec(source, globals, closure=closure)
        finally:
            _restore_exec_closure(mutated)
    try:
        if closure is None:
            return builtins.exec(source, globals, locals)
        return builtins.exec(source, globals, locals, closure=closure)
    finally:
        _restore_exec_closure(mutated)



def store_cell(cell, value):
    cell.cell_contents = value
    return value


def load_global(globals_dict, name):
    try:
        return globals_dict[name]
    except KeyError:
        try:
            return builtins.__dict__[name]
        except KeyError as exc:
            raise NameError(f"name {name!r} is not defined") from exc


def store_global(globals_dict, name, value):
    globals_dict[name] = value
    return value


def del_deref(cell):
    try:
        del cell.cell_contents
    except ValueError as exc:
        raise UnboundLocalError("local variable referenced before assignment") from exc


def call_super(super_fn, cls, instance_or_cls):
    if super_fn is builtins.super:
        if isinstance(cls, _types.CellType):
            try:
                cls_value = cls.cell_contents
            except ValueError:
                raise RuntimeError("super(): empty __class__ cell")
            return builtins.super(cls_value, instance_or_cls)
        return builtins.super(cls, instance_or_cls)
    return super_fn()


def call_super_noargs(super_fn):
    if super_fn is builtins.super:
        raise RuntimeError("super(): no arguments")
    return super_fn()


def _match_class_validate_arity(cls, match_args, total):
    allowed = 1 if match_args is None else len(match_args)
    if total > allowed:
        plural_allowed = "" if allowed == 1 else "s"
        raise TypeError(
            f"{cls.__name__}() accepts {allowed} positional sub-pattern"
            f"{plural_allowed} ({total} given)"
        )
    return allowed


def match_class_attr_exists(cls, subject, idx, total):
    match_args = getattr(cls, "__match_args__", None)
    _match_class_validate_arity(cls, match_args, total)

    if match_args is None:
        return True

    name = match_args[idx]
    return hasattr(subject, name)


def match_class_attr_value(cls, subject, idx, total):
    match_args = getattr(cls, "__match_args__", None)
    _match_class_validate_arity(cls, match_args, total)

    if match_args is None:
        return subject

    name = match_args[idx]
    return getattr(subject, name)



def update_fn(func, qualname, name):
    try:
        func.__qualname__ = qualname
    except (AttributeError, TypeError):
        pass        
    try:
        func.__name__ = name
    except (AttributeError, TypeError):
        pass
    if isinstance(func, _types.FunctionType):
        # In eval mode, soac-created functions carry execution metadata in
        # code extras; code.replace(...) drops those extras and can make calls
        # fall back to the stub bytecode object.
        if os.environ.get("DIET_PYTHON_MODE") == "eval":
            return func
        try:
            func.__code__ = func.__code__.replace(
                co_name=name,
                co_qualname=qualname,
            )
        except (AttributeError, ValueError):
            pass
    return func


def _bb_param_kind_and_name(raw_name):
    if raw_name.startswith("**"):
        return (_inspect.Parameter.VAR_KEYWORD, raw_name[2:])
    if raw_name.startswith("*"):
        return (_inspect.Parameter.VAR_POSITIONAL, raw_name[1:])
    if raw_name.startswith("kw:"):
        return (_inspect.Parameter.KEYWORD_ONLY, raw_name[3:])
    if raw_name.startswith("/"):
        return (_inspect.Parameter.POSITIONAL_ONLY, raw_name[1:])
    return (_inspect.Parameter.POSITIONAL_OR_KEYWORD, raw_name)


def _build_bb_signature(params):
    sig_params = []
    state_order = []
    for param in params:
        if len(param) >= 3:
            raw_name, _, default = param[:3]
        elif len(param) == 2:
            raw_name, _ = param
            default = NO_DEFAULT
        else:
            raise RuntimeError(f"invalid bb param spec: {param!r}")

        kind, name = _bb_param_kind_and_name(raw_name)
        state_order.append(name)
        param_default = _inspect._empty if default is NO_DEFAULT else default
        sig_params.append(
            _inspect.Parameter(name, kind, default=param_default)
        )

    return (_inspect.Signature(sig_params), tuple(state_order))


def _bb_state_order(default_order, closure):
    if not isinstance(closure, tuple):
        return (default_order, {})
    state_order = []
    closure_values = {}
    for item in closure:
        if isinstance(item, str):
            state_order.append(item)
            continue
        if (
            isinstance(item, tuple)
            and len(item) == 2
            and isinstance(item[0], str)
        ):
            state_order.append(item[0])
            closure_values[item[0]] = item[1]
            continue
        return (default_order, {})
    # An explicit closure tuple (including empty tuple) defines the exact BB
    # state order expected by lowered blocks.
    return (tuple(state_order), closure_values)


def _bb_wrap_with_closure(entry, closure_values):
    if not closure_values:
        return entry
    if getattr(entry, "__closure__", None):
        return entry
    captured_names = tuple(closure_values.keys())
    captured_values = tuple(closure_values[name] for name in captured_names)

    assigns = "\n".join(
        f"    {name} = __dp_values[{idx}]"
        for idx, name in enumerate(captured_names)
    )
    refs = "\n".join(f"        {name}" for name in captured_names)
    if not assigns:
        assigns = "    pass"
    if not refs:
        refs = "        pass"

    # Build a wrapper whose closure freevar names match captured_names.
    if _inspect.iscoroutinefunction(entry):
        src = (
            "def __dp_make(__dp_entry, __dp_values):\n"
            f"{assigns}\n"
            "    async def wrapped(*args, __dp_entry=__dp_entry, **kwargs):\n"
            f"{refs}\n"
            "        return await __dp_entry(*args, **kwargs)\n"
            "    return wrapped\n"
        )
    else:
        src = (
            "def __dp_make(__dp_entry, __dp_values):\n"
            f"{assigns}\n"
            "    def wrapped(*args, __dp_entry=__dp_entry, **kwargs):\n"
            f"{refs}\n"
            "        return __dp_entry(*args, **kwargs)\n"
            "    return wrapped\n"
        )
    ns = {}
    exec(src, {}, ns)
    return ns["__dp_make"](entry, captured_values)


def _bb_build_entry_args(bound_arguments, state_order, closure_values):
    return tuple(
        bound_arguments[name]
        if name in bound_arguments
        else closure_values.get(name, DELETED)
        for name in state_order
    )


def apply_fn_metadata(fn_obj, doc, annotate_fn):
    if doc is not None:
        fn_obj.__doc__ = doc
    if annotate_fn is not None:
        fn_obj.__annotate__ = annotate_fn
    return fn_obj


def def_fn(entry_bb, name, qualname, closure, params, module_name=None):
    # BB mode passes a lowered entry block, and def_fn builds the callable
    # wrapper so we don't need an extra transformed outer function call layer.
    signature, default_state_order = _build_bb_signature(params)
    state_order, closure_values = _bb_state_order(default_state_order, closure)

    def entry(
        *args,
        __dp_sig=signature,
        __dp_state_order=state_order,
        __dp_closure=closure_values,
        __dp_entry_bb=entry_bb,
        __dp_run_bb=run_bb,
        __dp_build_entry_args=_bb_build_entry_args,
        **kwargs,
    ):
        bound = __dp_sig.bind(*args, **kwargs)
        bound.apply_defaults()
        bb_args = __dp_build_entry_args(bound.arguments, __dp_state_order, __dp_closure)
        return __dp_run_bb(__dp_entry_bb, bb_args)

    entry = _bb_wrap_with_closure(entry, closure_values)
    entry.__signature__ = signature
    entry = update_fn(entry, qualname, name)
    if module_name is not None:
        entry.__module__ = module_name
    return entry


def def_coro(entry_bb, name, qualname, closure, params, module_name=None):
    signature, default_state_order = _build_bb_signature(params)
    state_order, closure_values = _bb_state_order(default_state_order, closure)

    async def entry(
        *args,
        __dp_sig=signature,
        __dp_state_order=state_order,
        __dp_closure=closure_values,
        __dp_entry_bb=entry_bb,
        __dp_run_coro_bb=run_coro_bb,
        __dp_build_entry_args=_bb_build_entry_args,
        **kwargs,
    ):
        bound = __dp_sig.bind(*args, **kwargs)
        bound.apply_defaults()
        bb_args = __dp_build_entry_args(bound.arguments, __dp_state_order, __dp_closure)
        return await __dp_run_coro_bb(__dp_entry_bb, bb_args)

    entry = _bb_wrap_with_closure(entry, closure_values)
    entry.__signature__ = signature
    entry = update_fn(entry, qualname, name)
    if module_name is not None:
        entry.__module__ = module_name
    return entry


_DP_GEN_CODE_TEMPLATE = None


def _dp_make_gen_code(name, qualname):
    global _DP_GEN_CODE_TEMPLATE
    if _DP_GEN_CODE_TEMPLATE is None:
        ns = {}
        exec(
            "def _dp_gen_code_template(_it):\n"
            "    while True:\n"
            "        yield next(_it)\n",
            {},
            ns,
        )
        _DP_GEN_CODE_TEMPLATE = ns["_dp_gen_code_template"].__code__

    code = _DP_GEN_CODE_TEMPLATE
    return code.replace(co_name=name, co_qualname=qualname)


_DP_ASYNC_GEN_CODE_TEMPLATE = None


def _dp_make_async_gen_code(name, qualname):
    global _DP_ASYNC_GEN_CODE_TEMPLATE
    if _DP_ASYNC_GEN_CODE_TEMPLATE is None:
        ns = {}
        exec(
            "async def _dp_async_gen_code_template():\n"
            "    if False:\n"
            "        yield None\n",
            {},
            ns,
        )
        _DP_ASYNC_GEN_CODE_TEMPLATE = ns["_dp_async_gen_code_template"].__code__

    code = _DP_ASYNC_GEN_CODE_TEMPLATE
    return code.replace(co_name=name, co_qualname=qualname)

class DefGenConst(NamedTuple):
    name: str
    qualname: str
    code: _types.CodeType
    module_name: str
    targets: tuple
    resume: object


def def_gen(
    resume,
    name,
    qualname,
    closure,
    params,
    module_name,
):
    signature, default_state_order = _build_bb_signature(params)
    state_order, closure_values = _bb_state_order(default_state_order, closure)
    gen_code = _dp_make_gen_code(name, qualname)

    def entry(
        *args,
        __dp_sig=signature,
        __dp_state_order=state_order,
        __dp_closure=closure_values,
        __dp_resume=resume,
        __dp_name=name,
        __dp_qualname=qualname,
        __dp_code=gen_code,
        **kwargs,
    ):
        bound = __dp_sig.bind(*args, **kwargs)
        bound.apply_defaults()
        state_args = tuple(
            bound.arguments[param]
            if param in bound.arguments
            else __dp_closure.get(param, DELETED)
            for param in __dp_state_order
        )
        _dp_frame = dict(())
        for _dp_state_name, _dp_state_value in zip(__dp_state_order, state_args):
            _dp_frame[_dp_state_name] = _dp_state_value

        _dp_gen = _DpGenerator(
            resume=__dp_resume,
            pc=0,
            gi_frame=_dp_frame,
            name=__dp_name,
            qualname=__dp_qualname,
            code=__dp_code,
        )
        return _dp_gen

    entry = _bb_wrap_with_closure(entry, closure_values)
    entry.__signature__ = signature
    entry = update_fn(entry, qualname, name)
    if module_name is not None:
        entry.__module__ = module_name
    return entry


def def_async_gen(
    resume,
    name,
    qualname,
    closure,
    params,
    module_name,
):
    signature, default_state_order = _build_bb_signature(params)
    state_order, closure_values = _bb_state_order(default_state_order, closure)
    ag_code = _dp_make_async_gen_code(name, qualname)

    def entry(
        *args,
        __dp_sig=signature,
        __dp_state_order=state_order,
        __dp_closure=closure_values,
        __dp_resume=resume,
        __dp_name=name,
        __dp_qualname=qualname,
        __dp_code=ag_code,
        **kwargs,
    ):
        bound = __dp_sig.bind(*args, **kwargs)
        bound.apply_defaults()
        state_args = tuple(
            bound.arguments[param]
            if param in bound.arguments
            else __dp_closure.get(param, DELETED)
            for param in __dp_state_order
        )
        _dp_frame = dict(())
        for _dp_state_name, _dp_state_value in zip(__dp_state_order, state_args):
            _dp_frame[_dp_state_name] = _dp_state_value

        _dp_gen = _DpAsyncGenerator(
            resume=__dp_resume,
            pc=0,
            gi_frame=_dp_frame,
            name=__dp_name,
            qualname=__dp_qualname,
            code=__dp_code,
        )
        _dp_gen._dp_transport_sent = None
        return _dp_gen

    entry = _bb_wrap_with_closure(entry, closure_values)
    entry.__signature__ = signature
    entry = update_fn(entry, qualname, name)
    if module_name is not None:
        entry.__module__ = module_name
    return entry


async def _dp_run_async_gen_resume(
    resume_block,
    gen,
    value=None,
    resume_exc=None,
    transport_sent=None,
):
    gen._dp_transport_sent = transport_sent
    return await run_coro_bb(resume_block, (gen, value, resume_exc))


# TODO: gross
def decode_surrogate_literal(src):
    try:
        import ast
        return ast.literal_eval(src)
    except Exception:
        return src




def create_class(
    name,
    namespace_fn,
    bases,
    kwds,
    requires_class_cell,
    firstlineno=None,
    static_attributes=(),
):
    resolved_bases = _types.resolve_bases(bases)
    meta, ns, meta_kwds = _types.prepare_class(name, resolved_bases, kwds)

    class_cell = ns.get("__classcell__", None)
    if requires_class_cell and class_cell is None:
        class_cell = make_cell()
        ns["__classcell__"] = class_cell

    namespace_fn(ns, class_cell)
    if "__firstlineno__" not in ns and firstlineno is not None:
        ns["__firstlineno__"] = firstlineno
    if "__static_attributes__" not in ns:
        ns["__static_attributes__"] = static_attributes

    if resolved_bases is not bases and "__orig_bases__" not in ns:
        ns["__orig_bases__"] = bases
    cls = meta(name, resolved_bases, ns, **meta_kwds)

    if cls is not None:
        ns.pop("__classcell__", None)

        if class_cell is not None:
            if isinstance(class_cell, _types.CellType):
                class_cell.cell_contents = cls
            else:
                raise TypeError("__classcell__ must be a cell")

    return cls

def exc_info():
    exc = current_exception()
    return exc_info_from_exception(exc)


def exc_info_from_exception(exc):
    if exc is None:
        return None
    return (type(exc), exc, exc.__traceback__)


def current_exception():
    return sys.exception()


def aiter(obj):
    try:
        aiter_fn = obj.__aiter__
    except AttributeError:
        obj_type = type(obj).__name__
        obj = None
        raise TypeError(
            f"'async for' requires an object with __aiter__ method, got {obj_type}"
        ) from None
    iterator = aiter_fn()
    if not hasattr(iterator, "__anext__"):
        iter_type = type(iterator).__name__
        iterator = None
        raise TypeError(
            "'async for' received an object from __aiter__ that does not implement __anext__"
            f": {iter_type}"
        ) from None
    return iterator


class _AwaitIterWrapper:
    __slots__ = ("_it",)

    def __init__(self, iterator):
        self._it = iterator

    def __await__(self):
        return self._it


def _get_awaitable_iter(awaitable):
    try:
        iterator = awaitable.__await__()
    except AttributeError:
        awaitable_type = type(awaitable).__name__
        awaitable = None
        raise TypeError(
            "'async for' received an invalid object from __anext__"
            f": {awaitable_type}"
        ) from None
    except Exception as exc:
        awaitable_type = type(awaitable).__name__
        awaitable = None
        raise TypeError(
            "'async for' received an invalid object from __anext__"
            f": {awaitable_type}"
        ) from exc
    if not hasattr(iterator, "__next__"):
        awaitable_type = type(awaitable).__name__
        awaitable = None
        raise TypeError(
            "'async for' received an invalid object from __anext__"
            f": {awaitable_type}"
        ) from None
    return iterator


ITER_COMPLETE = object()

async def anext_or_sentinel(iterator):
    try:
        awaitable = iterator.__anext__()
    except AttributeError:
        iter_type = type(iterator).__name__
        iterator = None
        raise TypeError(
            "'async for' received an object from __aiter__ that does not implement __anext__"
            f": {iter_type}"
        ) from None
    try:
        await_iter = _get_awaitable_iter(awaitable)
    except Exception:
        iterator = None
        awaitable = None
        raise
    try:
        return await _AwaitIterWrapper(await_iter)
    except StopAsyncIteration:
        return ITER_COMPLETE


def next_or_sentinel(iterator):
    try:
        return iterator.__next__()
    except AttributeError:
        iter_type = type(iterator).__name__
        iterator = None
        raise TypeError(
            "'for' received an object from __iter__ that does not implement __next__"
            f": {iter_type}"
        ) from None
    except StopIteration:
        return ITER_COMPLETE

def raise_from(exc, cause):
    CancelledError = None
    asyncio_mod = sys.modules.get("asyncio")
    if asyncio_mod is not None:
        CancelledError = getattr(asyncio_mod, "CancelledError", None)
    if exc is None:
        raise TypeError("exceptions must derive from BaseException")
    if isinstance(exc, type):
        if issubclass(exc, BaseException):
            exc = _call_exception_class(exc)
        else:
            raise TypeError("exceptions must derive from BaseException")
    elif not isinstance(exc, BaseException):
        raise TypeError("exceptions must derive from BaseException")
    if cause is None:
        exc.__cause__ = None
        exc.__suppress_context__ = True
    else:
        if isinstance(cause, type):
            if issubclass(cause, BaseException):
                cause = _call_exception_class(cause)
            else:
                raise TypeError("exception causes must derive from BaseException")
        elif not isinstance(cause, BaseException):
            raise TypeError("exception causes must derive from BaseException")
        if CancelledError is not None and type(cause) is CancelledError:
            cause = cause.with_traceback(None)
        exc.__cause__ = cause
        exc.__suppress_context__ = True
    return exc


def _call_exception_class(exc_type):
    inst = exc_type()
    if not isinstance(inst, BaseException):
        raise TypeError(
            f"calling {exc_type!r} should have returned an instance of BaseException, "
            f"not {type(inst)!r}"
        )
    return inst


def _patch_annotationlib(module):
    try:
        stringifier_dict = module._StringifierDict
    except AttributeError:
        return
    try:
        stringifier_cls = module._Stringifier
    except AttributeError:
        stringifier_cls = None
    try:
        build_closure = module._build_closure
    except AttributeError:
        build_closure = None
    if getattr(stringifier_dict, "_dp_patched", False):
        return
    original_init = stringifier_dict.__init__

    def __init__(self, namespace, *, globals=None, owner=None, is_class=False, format):
        original_init(
            self,
            namespace,
            globals=globals,
            owner=owner,
            is_class=is_class,
            format=format,
        )
        try:
            self["__dp__"] = builtins.__dp__
        except Exception:
            pass

    stringifier_dict.__init__ = __init__
    stringifier_dict._dp_patched = True
    if stringifier_cls is not None and not getattr(stringifier_cls, "_dp_patched", False):
        original_stringifier_init = stringifier_cls.__init__

        def __init__(
            self,
            node,
            globals=None,
            owner=None,
            is_class=False,
            cell=None,
            *,
            stringifier_dict,
            extra_names=None,
        ):
            if isinstance(node, str) and node.startswith(_DP_CELL_PREFIX):
                node = node[len(_DP_CELL_PREFIX):]
            original_stringifier_init(
                self,
                node,
                globals=globals,
                owner=owner,
                is_class=is_class,
                cell=cell,
                stringifier_dict=stringifier_dict,
                extra_names=extra_names,
            )

        stringifier_cls.__init__ = __init__
        stringifier_cls._dp_patched = True
    if build_closure is not None and not getattr(build_closure, "_dp_patched", False):
        def _build_closure_patched(annotate, owner, is_class, stringifier_dict, *, allow_evaluation):
            new_closure, cell_dict = build_closure(
                annotate,
                owner,
                is_class,
                stringifier_dict,
                allow_evaluation=allow_evaluation,
            )
            try:
                stringifiers = list(stringifier_dict.stringifiers)
            except Exception:
                stringifiers = []
            for obj in stringifiers:
                cell = getattr(obj, "__cell__", None)
                if isinstance(cell, _types.CellType):
                    try:
                        inner = cell.cell_contents
                    except ValueError:
                        inner = None
                    if isinstance(inner, _types.CellType):
                        obj.__cell__ = None
            if not cell_dict:
                return new_closure, cell_dict
            normalized = {}
            for name, cell in cell_dict.items():
                base_name = name
                if isinstance(name, str) and name.startswith(_DP_CELL_PREFIX):
                    base_name = name[len(_DP_CELL_PREFIX):]
                if isinstance(cell, _types.CellType):
                    try:
                        inner = cell.cell_contents
                    except ValueError:
                        inner = None
                    else:
                        if isinstance(inner, _types.CellType):
                            cell = inner
                normalized[base_name] = cell
            return new_closure, normalized

        _build_closure_patched._dp_patched = True
        module._build_closure = _build_closure_patched


def _annotationlib_patch_enabled():
    # Annotation callables now run without annotationlib monkeypatching.
    return False


def _ensure_annotationlib_import_hook():
    try:
        import importlib.abc
        import importlib.machinery
    except Exception:
        return

    class _AnnotationlibFinder(importlib.abc.MetaPathFinder):
        _dp_annotationlib_finder = True

        def find_spec(self, fullname, path, target=None):
            if fullname != "annotationlib":
                return None
            spec = importlib.machinery.PathFinder.find_spec(fullname, path)
            if spec is None or spec.loader is None:
                return spec
            if getattr(spec.loader, "_dp_annotationlib_wrapped", False):
                return spec
            exec_module = getattr(spec.loader, "exec_module", None)
            if exec_module is None:
                return spec

            def exec_module_wrapped(module):
                exec_module(module)
                _patch_annotationlib(module)

            spec.loader.exec_module = exec_module_wrapped
            spec.loader._dp_annotationlib_wrapped = True
            return spec

    for finder in sys.meta_path:
        if getattr(finder, "_dp_annotationlib_finder", False):
            return
    sys.meta_path.insert(0, _AnnotationlibFinder())


if _annotationlib_patch_enabled():
    import annotationlib
    if "annotationlib" in sys.modules:
        _patch_annotationlib(sys.modules["annotationlib"])
    else:
        _ensure_annotationlib_import_hook()


def import_(name, spec, fromlist=None, level=0):
    if fromlist is None:
        fromlist = []
    globals_dict = {"__spec__": spec}
    if spec is not None:
        package = spec.parent
        if not package and getattr(spec, "submodule_search_locations", None):
            package = spec.name
        globals_dict["__package__"] = package
        globals_dict["__name__"] = spec.name
    try:
        module = builtins.__import__(name, globals_dict, {}, fromlist, level)
        if name == "annotationlib" and _annotationlib_patch_enabled():
            _patch_annotationlib(module)
        return module
    except Exception as exc:
        raise exc from None


def import_attr(module, attr):
    try:
        return getattr(module, attr)
    except AttributeError as exc:
        module_name = getattr(module, "__name__", None)
        if module_name:
            submodule = sys.modules.get(f"{module_name}.{attr}")
            if submodule is not None:
                try:
                    setattr(module, attr, submodule)
                except Exception:
                    warnings.warn(
                        f"cannot set attribute {attr!r} on {module_name!r}",
                        ImportWarning,
                        stacklevel=2,
                    )
                return submodule
        module_spec = getattr(module, "__spec__", None)
        if (
            module_name
            and module_spec is not None
            and getattr(module_spec, "_initializing", False)
        ):
            message = (
                f"cannot import name {attr!r} from partially initialized module "
                f"{module_name!r} (most likely due to a circular import)"
            )
            import_error = ImportError(message, name=module_name)
            raise import_error.with_traceback(exc.__traceback__) from None
        module_name = module_name or "<unknown module name>"
        module_file = getattr(module, "__file__", None)
        message = f"cannot import name {attr!r} from {module_name!r}"
        if module_file is not None:
            message = f"{message} ({module_file})"
        else:
            message = f"{message} (unknown location)"
        import_error = ImportError(message, name=module_name, path=module_file)

        raise import_error.with_traceback(exc.__traceback__) from None


def import_star(name, spec, globals_dict, level=0):
    module = import_(name, spec, ["*"], level)
    try:
        names = getattr(module, "__all__", None)
    except Exception:
        names = None
    if names is None:
        names = [name for name in dir(module) if not name.startswith("_")]
    for name in names:
        globals_dict[name] = getattr(module, name)
    return module


def _lookup_special_method(obj, name: str):
    cls = type(obj)
    for base in cls.__mro__:
        try:
            descr = base.__dict__[name]
        except KeyError:
            continue
        if hasattr(descr, "__get__"):
            return descr.__get__(obj, cls)
        return descr
    raise AttributeError(name)

def _has_special_method(obj, name: str) -> bool:
    cls = type(obj)
    for base in cls.__mro__:
        if name in base.__dict__:
            return True
    return False


def contextmanager_enter(ctx):
    try:
        enter = _lookup_special_method(ctx, "__enter__")
    except AttributeError as exc:
        message = "object does not support the context manager protocol (missed __enter__ method)"
        if _has_special_method(ctx, "__aenter__") or _has_special_method(ctx, "__aexit__"):
            message += (
                " but it supports the asynchronous context manager protocol. "
                "Did you mean to use 'async with'?"
            )
        raise TypeError(message) from exc
    return enter()

def contextmanager_get_exit(cm):
    try:
        return _lookup_special_method(cm, "__exit__")
    except AttributeError as exc:
        message = "object does not support the context manager protocol (missed __exit__ method)"
        if _has_special_method(cm, "__aenter__") or _has_special_method(cm, "__aexit__"):
            message += (
                " but it supports the asynchronous context manager protocol. "
                "Did you mean to use 'async with'?"
            )
        raise TypeError(message) from exc


def contextmanager_exit(exit_fn, exc_info: tuple | None):
    if exc_info is not None:
        exc_type, exc, tb = exc_info
        try:
            suppress = exit_fn(*exc_info)
            if suppress:
                exc.__traceback__ = None
                return
            raise exc
        finally:
            # Clear the reference for GC in long-lived frames.
            exc_info = None
            exc_type = None
            exc = None
            tb = None
    else:
        exit_fn(None, None, None)


def _ensure_awaitable(
    awaitable, method_name: str, *, suppress_context: bool = True
):
    try:
        iterator = awaitable.__await__()
    except AttributeError as exc:
        if suppress_context:
            awaitable_type = type(awaitable).__name__
            awaitable = None
            raise TypeError(
                f"'async with' received an object from {method_name} that does not implement __await__: {awaitable_type}"
            ) from None
        iterator = None
    except Exception as exc:
        if suppress_context:
            awaitable_type = type(awaitable).__name__
            awaitable = None
            raise TypeError(
                f"'async with' received an object from {method_name} that does not implement __await__: {awaitable_type}"
            ) from exc
        iterator = None
    if iterator is None:
        awaitable_type = type(awaitable).__name__
        awaitable = None
        raise TypeError(
            f"'async with' received an object from {method_name} that does not implement __await__: {awaitable_type}"
        )
    if not hasattr(iterator, "__next__"):
        awaitable_type = type(awaitable).__name__
        awaitable = None
        raise TypeError(
            f"'async with' received an object from {method_name} that does not implement __await__: {awaitable_type}"
        ) from None
    return iterator


async def asynccontextmanager_aenter(ctx):
    try:
        aenter = _lookup_special_method(ctx, "__aenter__")
    except AttributeError as exc:
        message = "object does not support the asynchronous context manager protocol (missed __aenter__ method)"
        if _has_special_method(ctx, "__enter__") or _has_special_method(ctx, "__exit__"):
            message += " but it supports the context manager protocol. Did you mean to use 'with'?"
        raise TypeError(message) from exc
    await_iter = _ensure_awaitable(aenter(), "__aenter__")
    return await _AwaitIterWrapper(await_iter)

def asynccontextmanager_get_aexit(acm):
    try:
        return _lookup_special_method(acm, "__aexit__")
    except AttributeError as exc:
        message = "object does not support the asynchronous context manager protocol (missed __aexit__ method)"
        if _has_special_method(acm, "__enter__") or _has_special_method(acm, "__exit__"):
            message += " but it supports the context manager protocol. Did you mean to use 'with'?"
        raise TypeError(message) from exc


async def asynccontextmanager_aexit(exit_fn, exc_info: tuple | None):
    if exc_info is not None:
        exc_type, exc, tb = exc_info
        try:
            await_iter = _ensure_awaitable(
                exit_fn(*exc_info), "__aexit__", suppress_context=False
            )
            suppress = await _AwaitIterWrapper(await_iter)
            if suppress:
                exc.__traceback__ = None
            return suppress
        finally:
            exc_info = None
            exc_type = None
            exc = None
            tb = None
    else:
        await_iter = _ensure_awaitable(exit_fn(None, None, None), "__aexit__")
        await _AwaitIterWrapper(await_iter)
        return False

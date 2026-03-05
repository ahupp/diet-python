# diet-python: disabled
import collections.abc as _abc
import inspect as _inspect
import os
import operator as _operator
import reprlib
import sys
import builtins
import threading as _threading
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
ascii = builtins.ascii
repr = builtins.repr
str = builtins.str
format = builtins.format
AssertionError = builtins.AssertionError


def _dp_tuple_helper(*values):
    # __dp_tuple is strict variadic tuple construction for transformed code.
    return builtins.tuple(values)


def _dp_tuple_from_iter_helper(value):
    return builtins.tuple(value)


def __deepcopy__(memo):
    # Modules are not pickleable; keep __dp__ as a singleton during deepcopy().
    return sys.modules[__name__]


builtins.__dp__ = sys.modules[__name__]
builtins.__dp_getattr = builtins.getattr

_MISSING = object()
DELETED = object()
NO_DEFAULT = object()
_GEN_PC_DONE = -1
builtins.__dp_DELETED = DELETED
builtins.__dp_Ellipsis = Ellipsis
builtins.__dp_TRUE = True
builtins.__dp_FALSE = False
builtins.__dp_NONE = None


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


def _ioper(
    inplace_name: str,
    lhs_method_name: str,
    rhs_method_name: str,
    op_symbol: str,
    lhs,
    rhs,
):
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
        if name.startswith("_dp_yield_from_iter_"):
            # Yield-from lowering tracks the active delegated iterator in
            # `gi_yieldfrom`. If temp names differ across transformed resume
            # blocks, fall back to the canonical generator slot.
            return load_deleted_name(
                name,
                getattr(gen, "gi_yieldfrom", DELETED),
            )
        raise UnboundLocalError(
            f"cannot access local variable {name!r} where it is not associated with a value"
        ) from exc
    return load_deleted_name(name, value)


def __dp_load_local_raw(gen, name):
    frame = _resolve_local_frame(gen)
    if name.startswith("_dp_yield_from_iter_") and name not in frame:
        return getattr(gen, "gi_yieldfrom", DELETED)
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


def _resume_block_param_names(resume_block):
    names = _block_param_names(resume_block)
    if names:
        return names
    if _jit_block_param_names is None:
        return names
    plan_module = getattr(resume_block, "__dp_plan_module", None)
    plan_qualname = getattr(resume_block, "__dp_plan_qualname", None)
    entry_ref = getattr(resume_block, "__dp_entry_ref", None)
    if not (
        isinstance(plan_module, str)
        and isinstance(plan_qualname, str)
        and isinstance(entry_ref, str)
    ):
        return names
    try:
        resolved = _jit_block_param_names(plan_module, plan_qualname, entry_ref)
    except Exception:
        return names
    if isinstance(resolved, (tuple, list)):
        return tuple(resolved)
    return names


def _bb_build_resume_args(resume_block, gen, send_value, resume_exc, transport_sent=None):
    # Generator/async-generator BB lowering still emits resume blocks that may
    # carry additional frame locals as block parameters. Reconstruct arguments
    # by declared parameter names so JIT and interpreted BB execution agree.
    frame = getattr(gen, "gi_frame", None)
    args = []
    for name in _resume_block_param_names(resume_block):
        if name in ("_dp_self", "_dp_state"):
            args.append(gen)
        elif name == "_dp_send_value":
            args.append(send_value)
        elif name == "_dp_resume_exc":
            args.append(resume_exc)
        elif name == "_dp_transport_sent":
            args.append(transport_sent)
        elif isinstance(frame, dict):
            args.append(frame.get(name, DELETED))
        else:
            args.append(DELETED)
    return tuple(args)


_jit_run_bb_plan = None
_jit_render_bb_plan = None
_jit_has_bb_plan = None
_jit_block_param_names = None
_register_clif_wrapper = None
_jit_implicit_locals_stack = []
_jit_run_bb_depth = _threading.local()
_JIT_RECURSION_SOFT_LIMIT = 200


def _run_bb_plan_from_entry(entry, args):
    if _jit_run_bb_plan is None:
        raise RuntimeError("JIT basic-block plan runner is unavailable")
    plan_module = getattr(entry, "__dp_plan_module", None)
    plan_qualname = getattr(entry, "__dp_plan_qualname", None)
    if not isinstance(plan_module, str) or not isinstance(plan_qualname, str):
        raise RuntimeError(
            f"JIT basic-block execution requires plan metadata on entry: {entry!r}"
        )
    if _jit_has_bb_plan is not None and not _jit_has_bb_plan(plan_module, plan_qualname):
        raise RuntimeError(
            "JIT basic-block execution requires a registered plan, "
            f"but none is available for {plan_module}.{plan_qualname}"
        )
    globals_dict = getattr(entry, "__dp_plan_globals", None)
    if not isinstance(globals_dict, dict):
        globals_dict = getattr(entry, "__globals__", None)
    return _jit_run_bb_plan(plan_module, plan_qualname, globals_dict, args)


def run_bb(entry, args):
    return _run_bb_plan_from_entry(entry, args)


def render_jit_bb(entry):
    if _jit_render_bb_plan is None:
        raise RuntimeError("JIT CLIF renderer is unavailable")
    plan_module = getattr(entry, "__dp_plan_module", None)
    plan_qualname = getattr(entry, "__dp_plan_qualname", None)
    if not isinstance(plan_module, str) or not isinstance(plan_qualname, str):
        raise RuntimeError("entry is missing JIT plan metadata")
    return _jit_render_bb_plan(plan_module, plan_qualname)


def _dp_resume_generator(gen, value, resume_exc):
    bb_args = _bb_build_resume_args(gen._dp_resume, gen, value, resume_exc)
    return run_bb(gen._dp_resume, bb_args)


async def _dp_resume_async_generator(gen, value, resume_exc, transport_sent):
    send_value = value
    pending_exc = resume_exc
    pending_transport = transport_sent
    while True:
        # When delegating through `gi_yieldfrom` (for lowered `await` paths in
        # async generators), resume values/exceptions come from the awaiter
        # transport channel rather than user-level `asend(...)`.
        step_send_value = (
            pending_transport if getattr(gen, "gi_yieldfrom", None) is not None else send_value
        )
        bb_args = _bb_build_resume_args(
            gen._dp_resume, gen, step_send_value, pending_exc, pending_transport
        )
        result = run_bb(gen._dp_resume, bb_args)
        if getattr(gen, "gi_yieldfrom", None) is None:
            return result
        try:
            pending_transport = await _AsyncGenTransportStep(result)
            pending_exc = None
        except BaseException as exc:
            pending_transport = None
            pending_exc = exc


class _AsyncGenTransportStep:
    __slots__ = ("_value",)

    def __init__(self, value):
        self._value = value

    def __await__(self):
        sent = yield self._value
        return sent


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

    def send(self, value):
        return _dp_resume_generator(self, value, None)

    def throw(self, typ=None, val=None, tb=None):
        if val is not None or tb is not None:
            raise TypeError(
                "DpGen.throw() does not support value/traceback in this mode"
            )
        exc = raise_from(typ, None)
        _attach_throw_context_from_state(self, exc)
        return _dp_resume_generator(self, None, exc)

    def close(self):
        try:
            self.throw(GeneratorExit)
        except (GeneratorExit, StopIteration):
            return None
        raise RuntimeError("generator ignored GeneratorExit")


class _DpCoroutine(_abc.Coroutine):
    __slots__ = ("_dp_gen",)

    def __init__(self, gen):
        self._dp_gen = gen

    def __await__(self):
        return self

    def __iter__(self):
        return self

    def __next__(self):
        return self.send(None)

    def send(self, value):
        return self._dp_gen.send(value)

    def throw(self, typ, val=None, tb=None):
        if val is not None or tb is not None:
            raise TypeError(
                "DpCoroutine.throw() does not support value/traceback in this mode"
            )
        return self._dp_gen.throw(typ)

    def close(self):
        return self._dp_gen.close()

    @property
    def cr_frame(self):
        return self._dp_gen.gi_frame

    @property
    def cr_running(self):
        return False

    @property
    def cr_code(self):
        return self._dp_gen.gi_code

    @property
    def cr_await(self):
        return self._dp_gen.gi_yieldfrom


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

    def asend(self, value):
        return _DpAsyncGenSend(self, value)

    async def athrow(self, typ=None, val=None, tb=None):
        if val is not None or tb is not None:
            raise TypeError(
                "DpAsyncGen.athrow() does not support value/traceback in this mode"
            )
        exc = raise_from(typ, None)
        _attach_throw_context_from_state(self, exc)
        return await _dp_resume_async_generator(self, None, exc, None)

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
                raise TypeError(
                    "can't send non-None value to a just-started async generator"
                )
            self._dp_coro = _dp_resume_async_generator(
                self._dp_gen,
                initial_send,
                None,
                value,
            )
            return self._dp_coro.send(None)
        return self._dp_coro.send(value)

    def throw(self, typ, val=None, tb=None):
        if self._dp_coro is None:
            self._dp_coro = _dp_resume_async_generator(
                self._dp_gen, self._dp_value, None, None
            )
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
    raise TypeError(
        "catching classes that do not inherit from BaseException is not allowed"
    )


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
            base = name[len(_DP_CELL_PREFIX) :]
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


def normalize_mapping(values):
    return _normalize_mapping(values)


def dir_from_locals_mapping(values):
    names = _normalize_mapping(values).keys()
    filtered = []
    for name in names:
        if not name.startswith("_dp_"):
            filtered.append(name)
    return sorted(filtered)


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
            if cell_name in locals_map and isinstance(
                locals_map[cell_name], _types.CellType
            ):
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
        if frame.f_back is not None and frame.f_code.co_name in {
            "<listcomp>",
            "<setcomp>",
            "<dictcomp>",
            "<genexpr>",
        }:
            frame = frame.f_back
    return FrameLocalsProxy(frame)


def globals():
    frame = sys._getframe(1)
    return frame.f_globals


builtins.__dp_globals = globals


def dir_(*args):
    if args:
        return builtins.dir(*args)
    frame = sys._getframe(1)
    names = _normalize_mapping(frame.f_locals).keys()
    filtered = []
    for name in names:
        if not name.startswith("_dp_"):
            filtered.append(name)
    return sorted(filtered)


def eval_(source, globals=None, locals=None):
    if globals is None or locals is None:
        frame = sys._getframe(1)
        if globals is None:
            globals = frame.f_globals
        if locals is None:
            locals = _normalize_mapping(frame.f_locals)
            closure_values = None
            if _jit_implicit_locals_stack:
                closure_values = _jit_implicit_locals_stack[-1]
            if closure_values is None:
                probe = frame
                while probe is not None and closure_values is None:
                    closure_values = probe.f_locals.get("__dp_closure")
                    probe = probe.f_back
            if isinstance(closure_values, dict) and closure_values:
                # JIT BB wrappers execute through synthetic `entry(...)` frames.
                # Merge captured lexical bindings so implicit eval() resolves
                # closure names the same way as regular Python frames.
                merged = {}
                for name, value in closure_values.items():
                    merged[name] = value
                    if isinstance(name, str) and name.startswith("_dp_cell_"):
                        try:
                            merged[name[len("_dp_cell_"):]] = load_cell(value)
                        except UnboundLocalError:
                            pass
                merged.update(locals)
                locals = merged
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
        sig_params.append(_inspect.Parameter(name, kind, default=param_default))

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
        if isinstance(item, tuple) and len(item) == 2 and isinstance(item[0], str):
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
    captured_values_list = []
    for name in captured_names:
        captured_values_list.append(closure_values[name])
    captured_values = tuple(captured_values_list)

    assign_lines = []
    for idx, name in enumerate(captured_names):
        assign_lines.append(f"    {name} = __dp_values[{idx}]")
    assigns = "\n".join(assign_lines)

    ref_lines = []
    for name in captured_names:
        ref_lines.append(f"        {name}")
    refs = "\n".join(ref_lines)
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


_DP_ENTRY_TEMPLATE_CODE = None
_DP_ASYNC_ENTRY_TEMPLATE_CODE = None


def _bb_entry_template_code(async_entry):
    global _DP_ENTRY_TEMPLATE_CODE
    global _DP_ASYNC_ENTRY_TEMPLATE_CODE
    if async_entry:
        if _DP_ASYNC_ENTRY_TEMPLATE_CODE is None:
            ns = {}
            exec(
                "async def _dp_entry_template(\n"
                "    *args,\n"
                "    **kwargs,\n"
                "):\n"
                "    raise RuntimeError(\n"
                "        \"CLIF wrapper coroutine entry executed without eval-frame interception\"\n"
                "    )\n",
                {},
                ns,
            )
            _DP_ASYNC_ENTRY_TEMPLATE_CODE = ns["_dp_entry_template"].__code__
        return _DP_ASYNC_ENTRY_TEMPLATE_CODE
    if _DP_ENTRY_TEMPLATE_CODE is None:
        ns = {}
        exec(
            "def _dp_entry_template(\n"
            "    *args,\n"
            "    **kwargs,\n"
            "):\n"
            "    raise RuntimeError(\n"
            "        \"CLIF wrapper entry executed without eval-frame interception\"\n"
            "    )\n",
            {},
            ns,
        )
        _DP_ENTRY_TEMPLATE_CODE = ns["_dp_entry_template"].__code__
    return _DP_ENTRY_TEMPLATE_CODE




def _bb_make_lazy_clif_entry(
    *,
    async_entry,
    function_name,
    module_globals,
):
    entry = _types.FunctionType(
        _bb_entry_template_code(async_entry),
        module_globals,
        name=function_name,
    )
    return entry


def _bb_build_entry_args(bound_arguments, state_order, closure_values):
    values = []
    for name in state_order:
        if name in bound_arguments:
            values.append(bound_arguments[name])
        else:
            values.append(closure_values.get(name, DELETED))
    return tuple(values)


def _bb_rebind_function_globals(func, module_globals):
    if module_globals is None:
        return func
    if not isinstance(module_globals, dict):
        raise TypeError("module_globals must be a dict")
    if func.__globals__ is module_globals:
        return func
    rebound = _types.FunctionType(
        func.__code__,
        module_globals,
        name=func.__name__,
        argdefs=func.__defaults__,
        closure=func.__closure__,
    )
    # Preserve runtime metadata carried in keyword-only defaults.
    rebound.__kwdefaults__ = func.__kwdefaults__
    rebound.__dict__.update(func.__dict__)
    return rebound


def _bb_set_plan_metadata(
    func,
    module_name,
    qualname,
    module_globals=None,
    entry_ref=None,
):
    if callable(func):
        setattr(func, "__dp_plan_module", module_name)
        setattr(func, "__dp_plan_qualname", qualname)
        if isinstance(entry_ref, str):
            setattr(func, "__dp_entry_ref", entry_ref)
        if isinstance(module_globals, dict):
            setattr(func, "__dp_plan_globals", module_globals)


def _bb_enable_lazy_clif_wrapper(
    entry,
    module_name,
    plan_qualname,
    signature,
    state_order,
    closure_values,
    build_entry_args,
):
    # Python functions already use vectorcall by default. Registering CLIF
    # wrapper code-extra keeps that call convention and routes frame execution
    # through the PyUnstable eval-frame hook.
    if _register_clif_wrapper is None:
        raise RuntimeError(
            "JIT basic-block wrapper registration helper is unavailable for "
            f"{module_name}.{plan_qualname}"
        )
    try:
        _register_clif_wrapper(
            entry,
            module_name,
            plan_qualname,
            signature,
            state_order,
            closure_values,
            build_entry_args,
        )
    except Exception as exc:
        raise RuntimeError(
            "failed to register lazy CLIF wrapper for "
            f"{module_name}.{plan_qualname}: {exc}"
        ) from exc


def _bb_plan_lookup_qualname(qualname, entry_ref):
    if isinstance(entry_ref, str):
        return f"{qualname}::{entry_ref}"
    return qualname


def _bb_resolve_entry_ref(entry_ref, module_globals, module_name, qualname):
    if callable(entry_ref):
        resolved = entry_ref
    elif isinstance(entry_ref, str):
        if not entry_ref.startswith("_dp_bb_"):
            raise TypeError(
                f"unexpected non-BB string entry reference: {entry_ref!r}"
            )
        # BB labels in this runtime are JIT-plan-only. If execution reaches this
        # callable it means no JIT plan was registered for this function/module.
        # Raise a clear error rather than returning a non-terminator value.
        def _bb_entry_unavailable(*_args, __dp_entry_ref=entry_ref, **_kwargs):
            raise RuntimeError(
                "basic-block string entry requires a registered JIT plan, "
                f"but no plan is available for {module_name}.{qualname} "
                f"(entry={__dp_entry_ref!r})"
            )

        resolved = _bb_entry_unavailable
    else:
        raise TypeError(
            f"basic-block entry reference must be callable or str, got {type(entry_ref)!r}"
        )
    if module_name is not None:
        lookup_qualname = _bb_plan_lookup_qualname(
            qualname, entry_ref if isinstance(entry_ref, str) else None
        )
        _bb_set_plan_metadata(
            resolved,
            module_name,
            lookup_qualname,
            module_globals,
            entry_ref=entry_ref if isinstance(entry_ref, str) else None,
        )
    return resolved


def run_bb_plan(plan_module, plan_qualname, globals_dict, args, implicit_locals=None):
    if _jit_run_bb_plan is None:
        raise RuntimeError(
            f"basic-block plan execution requires JIT: {plan_module}.{plan_qualname}"
        )
    depth = getattr(_jit_run_bb_depth, "value", 0) + 1
    _jit_run_bb_depth.value = depth
    effective_limit = min(sys.getrecursionlimit(), _JIT_RECURSION_SOFT_LIMIT)
    if depth > effective_limit:
        _jit_run_bb_depth.value = depth - 1
        raise RecursionError("maximum recursion depth exceeded")
    if implicit_locals is None:
        try:
            return _jit_run_bb_plan(plan_module, plan_qualname, globals_dict, args)
        finally:
            _jit_run_bb_depth.value = depth - 1
    _jit_implicit_locals_stack.append(implicit_locals)
    try:
        return _jit_run_bb_plan(plan_module, plan_qualname, globals_dict, args)
    finally:
        _jit_implicit_locals_stack.pop()
        _jit_run_bb_depth.value = depth - 1


def jit_bb_plan_enabled():
    return _jit_run_bb_plan is not None


def def_fn(
    entry_bb,
    name,
    qualname,
    closure,
    params,
    module_globals=None,
    module_name=None,
    doc=None,
    annotate_fn=None,
):
    # BB mode passes a lowered entry block, and def_fn builds the callable
    # wrapper so we don't need an extra transformed outer function call layer.
    signature, default_state_order = _build_bb_signature(params)
    state_order, closure_values = _bb_state_order(default_state_order, closure)
    entry_ref = entry_bb if isinstance(entry_bb, str) else None
    plan_qualname = _bb_plan_lookup_qualname(qualname, entry_ref)
    _bb_resolve_entry_ref(entry_bb, module_globals, module_name, qualname)
    if not (
        jit_bb_plan_enabled()
        and isinstance(module_name, str)
        and isinstance(plan_qualname, str)
        and _jit_has_bb_plan is not None
        and _jit_has_bb_plan(module_name, plan_qualname)
    ):
        raise RuntimeError(
            "JIT basic-block function definition requires a registered plan, "
            f"but none is available for {module_name}.{plan_qualname}"
        )

    entry = _bb_make_lazy_clif_entry(
        async_entry=False,
        function_name=name,
        module_globals=module_globals,
    )
    entry.__signature__ = signature
    entry = update_fn(entry, qualname, name)
    if module_name is not None:
        entry.__module__ = module_name
    if doc is not None:
        entry.__doc__ = doc
    if annotate_fn is not None:
        entry.__annotate__ = annotate_fn
    _bb_enable_lazy_clif_wrapper(
        entry,
        module_name,
        plan_qualname,
        signature,
        state_order,
        closure_values,
        _bb_build_entry_args,
    )
    return entry


def def_coro(
    entry_bb,
    name,
    qualname,
    closure,
    params,
    module_globals=None,
    module_name=None,
    doc=None,
    annotate_fn=None,
):
    signature, default_state_order = _build_bb_signature(params)
    state_order, closure_values = _bb_state_order(default_state_order, closure)
    entry_ref = entry_bb if isinstance(entry_bb, str) else None
    plan_qualname = _bb_plan_lookup_qualname(qualname, entry_ref)
    _bb_resolve_entry_ref(entry_bb, module_globals, module_name, qualname)
    if not (
        jit_bb_plan_enabled()
        and isinstance(module_name, str)
        and isinstance(plan_qualname, str)
        and _jit_has_bb_plan is not None
        and _jit_has_bb_plan(module_name, plan_qualname)
    ):
        raise RuntimeError(
            "JIT basic-block coroutine definition requires a registered plan, "
            f"but none is available for {module_name}.{plan_qualname}"
        )

    entry = _bb_make_lazy_clif_entry(
        async_entry=True,
        function_name=name,
        module_globals=module_globals,
    )
    entry.__signature__ = signature
    entry = update_fn(entry, qualname, name)
    if module_name is not None:
        entry.__module__ = module_name
    if doc is not None:
        entry.__doc__ = doc
    if annotate_fn is not None:
        entry.__annotate__ = annotate_fn
    _bb_enable_lazy_clif_wrapper(
        entry,
        module_name,
        plan_qualname,
        signature,
        state_order,
        closure_values,
        _bb_build_entry_args,
    )
    return entry


def _dp_mark_coroutine_function(func):
    module = sys.modules.get("asyncio.coroutines")
    if module is None:
        try:
            import asyncio.coroutines as module
        except Exception:
            module = None
    marker = getattr(module, "_is_coroutine", None) if module is not None else None
    if marker is not None:
        try:
            func._is_coroutine = marker
        except Exception:
            pass
    return func


def def_coro_from_gen(
    resume,
    name,
    qualname,
    closure,
    params,
    module_globals,
    module_name,
    doc=None,
    annotate_fn=None,
):
    gen_entry = def_gen(
        resume,
        name,
        qualname,
        closure,
        params,
        module_globals,
        module_name,
        doc,
        annotate_fn,
    )

    def entry(
        *args,
        __dp_gen_entry=gen_entry,
        __dp_coro_type=_DpCoroutine,
        **kwargs,
    ):
        return __dp_coro_type(__dp_gen_entry(*args, **kwargs))

    entry = _bb_rebind_function_globals(entry, module_globals)
    if hasattr(gen_entry, "__signature__"):
        entry.__signature__ = gen_entry.__signature__
    entry = update_fn(entry, qualname, name)
    if module_name is not None:
        entry.__module__ = module_name
        _bb_set_plan_metadata(entry, module_name, qualname, module_globals)
    if doc is not None:
        entry.__doc__ = doc
    if annotate_fn is not None:
        entry.__annotate__ = annotate_fn
    return _dp_mark_coroutine_function(entry)


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
    module_globals,
    module_name,
    doc=None,
    annotate_fn=None,
):
    signature, default_state_order = _build_bb_signature(params)
    state_order, closure_values = _bb_state_order(default_state_order, closure)
    gen_code = _dp_make_gen_code(name, qualname)
    _dp_deleted = DELETED
    _dp_gen_type = _DpGenerator
    resolved_resume = _bb_resolve_entry_ref(
        resume, module_globals, module_name, qualname
    )

    def entry(*args, **kwargs):
        bound = signature.bind(*args, **kwargs)
        bound.apply_defaults()
        state_arg_items = []
        for param in state_order:
            if param in bound.arguments:
                state_arg_items.append(bound.arguments[param])
            else:
                state_arg_items.append(closure_values.get(param, _dp_deleted))
        state_args = tuple(state_arg_items)
        _dp_frame = dict(())
        for _dp_state_name, _dp_state_value in zip(state_order, state_args):
            _dp_frame[_dp_state_name] = _dp_state_value

        _dp_gen = _dp_gen_type(
            resume=resolved_resume,
            pc=0,
            gi_frame=_dp_frame,
            name=name,
            qualname=qualname,
            code=gen_code,
        )
        return _dp_gen

    entry = _bb_wrap_with_closure(entry, closure_values)
    entry = _bb_rebind_function_globals(entry, module_globals)
    entry.__signature__ = signature
    entry = update_fn(entry, qualname, name)
    if module_name is not None:
        entry.__module__ = module_name
        _bb_set_plan_metadata(entry, module_name, qualname, module_globals)
    if doc is not None:
        entry.__doc__ = doc
    if annotate_fn is not None:
        entry.__annotate__ = annotate_fn
    return entry


def def_async_gen(
    resume,
    name,
    qualname,
    closure,
    params,
    module_globals,
    module_name,
    doc=None,
    annotate_fn=None,
):
    signature, default_state_order = _build_bb_signature(params)
    state_order, closure_values = _bb_state_order(default_state_order, closure)
    ag_code = _dp_make_async_gen_code(name, qualname)
    _dp_deleted = DELETED
    _dp_async_gen_type = _DpAsyncGenerator
    resolved_resume = _bb_resolve_entry_ref(
        resume, module_globals, module_name, qualname
    )

    def entry(*args, **kwargs):
        bound = signature.bind(*args, **kwargs)
        bound.apply_defaults()
        state_arg_items = []
        for param in state_order:
            if param in bound.arguments:
                state_arg_items.append(bound.arguments[param])
            else:
                state_arg_items.append(closure_values.get(param, _dp_deleted))
        state_args = tuple(state_arg_items)
        _dp_frame = dict(())
        for _dp_state_name, _dp_state_value in zip(state_order, state_args):
            _dp_frame[_dp_state_name] = _dp_state_value

        _dp_gen = _dp_async_gen_type(
            resume=resolved_resume,
            pc=0,
            gi_frame=_dp_frame,
            name=name,
            qualname=qualname,
            code=ag_code,
        )
        _dp_gen._dp_transport_sent = None
        return _dp_gen

    entry = _bb_wrap_with_closure(entry, closure_values)
    entry = _bb_rebind_function_globals(entry, module_globals)
    entry.__signature__ = signature
    entry = update_fn(entry, qualname, name)
    if module_name is not None:
        entry.__module__ = module_name
        _bb_set_plan_metadata(entry, module_name, qualname, module_globals)
    if doc is not None:
        entry.__doc__ = doc
    if annotate_fn is not None:
        entry.__annotate__ = annotate_fn
    return entry



def decode_literal_bytes(value):
    return value.decode("utf-8", "surrogatepass")


# TODO: questionable
def decode_literal_source_bytes(src_bytes):
    try:
        import ast

        src = src_bytes.decode("utf-8", "surrogatepass")
        value = ast.literal_eval(src)
        if isinstance(value, str):
            return decode_literal_bytes(value.encode("utf-8", "surrogatepass"))
        return value
    except Exception:
        return decode_literal_bytes(src_bytes)


builtins.__dp_decode_literal_bytes = decode_literal_bytes
builtins.__dp_decode_literal_source_bytes = decode_literal_source_bytes


def exec_function_def_source(source, globals_dict, captures, name):
    namespace = dict(captures)
    builtins.exec(source, globals_dict, namespace)
    return namespace[name]


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


builtins.__dp_exc_info_from_exception = exc_info_from_exception


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
            f"'async for' received an invalid object from __anext__: {awaitable_type}"
        ) from None
    except Exception as exc:
        awaitable_type = type(awaitable).__name__
        awaitable = None
        raise TypeError(
            f"'async for' received an invalid object from __anext__: {awaitable_type}"
        ) from exc
    if not hasattr(iterator, "__next__"):
        awaitable_type = type(awaitable).__name__
        awaitable = None
        raise TypeError(
            f"'async for' received an invalid object from __anext__: {awaitable_type}"
        ) from None
    return iterator


def await_iter(awaitable):
    try:
        iterator = awaitable.__await__()
    except AttributeError:
        awaitable_type = type(awaitable).__name__
        awaitable = None
        raise TypeError(
            f"object {awaitable_type!r} can't be used in 'await' expression"
        ) from None
    except Exception as exc:
        awaitable_type = type(awaitable).__name__
        awaitable = None
        raise TypeError(
            f"object {awaitable_type!r} can't be used in 'await' expression"
        ) from exc
    if not hasattr(iterator, "__next__"):
        awaitable_type = type(awaitable).__name__
        awaitable = None
        raise TypeError(
            f"object {awaitable_type!r} can't be used in 'await' expression"
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
        return builtins.__import__(name, globals_dict, {}, fromlist, level)
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
        if _has_special_method(ctx, "__aenter__") or _has_special_method(
            ctx, "__aexit__"
        ):
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
        if _has_special_method(cm, "__aenter__") or _has_special_method(
            cm, "__aexit__"
        ):
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


def _ensure_awaitable(awaitable, method_name: str, *, suppress_context: bool = True):
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
        if _has_special_method(ctx, "__enter__") or _has_special_method(
            ctx, "__exit__"
        ):
            message += " but it supports the context manager protocol. Did you mean to use 'with'?"
        raise TypeError(message) from exc
    await_iter = _ensure_awaitable(aenter(), "__aenter__")
    return await _AwaitIterWrapper(await_iter)


def asynccontextmanager_get_aexit(acm):
    try:
        return _lookup_special_method(acm, "__aexit__")
    except AttributeError as exc:
        message = "object does not support the asynchronous context manager protocol (missed __aexit__ method)"
        if _has_special_method(acm, "__enter__") or _has_special_method(
            acm, "__exit__"
        ):
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


def _inject_builtin_helper_aliases():
    module_dict = globals()
    for name, value in tuple(module_dict.items()):
        if name.startswith("_"):
            continue
        if name == "tuple":
            setattr(builtins, "__dp_tuple", _dp_tuple_helper)
            setattr(builtins, "__dp_tuple_from_iter", _dp_tuple_from_iter_helper)
            continue
        if callable(value):
            setattr(builtins, f"__dp_{name}", value)


_inject_builtin_helper_aliases()

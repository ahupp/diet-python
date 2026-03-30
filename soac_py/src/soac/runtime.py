# diet-python: disabled
from asyncio import coroutines
import collections.abc as _abc
import keyword as _keyword
import os
import reprlib
import sys
import builtins
import threading as _threading
import types as _types
from typing import NamedTuple
import warnings

try:
    from diet_python import (
        make_bb_function as _jit_make_bb_function,
        make_bb_generator as _jit_make_bb_generator,
    )
except Exception:
    _jit_make_bb_function = None
    _jit_make_bb_generator = None

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
    # Modules are not pickleable; keep runtime as a singleton during deepcopy().
    return sys.modules[__name__]


builtins.runtime = sys.modules[__name__]
builtins.__dp_getattr = builtins.getattr
_dp_typing = builtins.__import__("typing")
_dp_templatelib = builtins.__import__(
    "string.templatelib", globals(), {}, ("Template", "Interpolation"), 0
)
builtins.__dp_typing_Generic = _dp_typing.Generic
builtins.__dp_typing_TypeVar = _dp_typing.TypeVar
builtins.__dp_typing_TypeVarTuple = _dp_typing.TypeVarTuple
builtins.__dp_typing_ParamSpec = _dp_typing.ParamSpec
builtins.__dp_typing_TypeAliasType = _dp_typing.TypeAliasType
builtins.__dp_typing_Unpack = _dp_typing.Unpack
builtins.__dp_templatelib_Template = _dp_templatelib.Template
builtins.__dp_templatelib_Interpolation = _dp_templatelib.Interpolation

_MISSING = object()
DELETED = object()
NO_DEFAULT = object()

builtins.__dp_DELETED = DELETED
builtins.__dp_NO_DEFAULT = NO_DEFAULT
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


def bb_trace_enter(function_qualname, block_label, params=None):
    if params:
        pieces = []
        for name, value in params:
            try:
                rendered = reprlib.repr(value)
            except Exception as err:
                rendered = f"<repr failed: {type(err).__name__}>"
            pieces.append(f"{name}={rendered}")
        message = f"[bb] {function_qualname}::{block_label} " + ", ".join(pieces)
    else:
        message = f"[bb] {function_qualname}::{block_label}"
    print(message, file=sys.stderr, flush=True)


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
    return None


def _resume_closure_value(owner, name):
    resume = getattr(owner, "_dp_resume", None)
    closure = getattr(resume, "__closure__", None)
    code = getattr(resume, "__code__", None)
    if closure is None or code is None:
        return _MISSING
    target_name = (
        name
        if name == "_dp_classcell" or name.startswith("_dp_cell_")
        else f"_dp_cell_{name}"
    )
    for freevar, cell in zip(code.co_freevars, closure):
        if freevar != target_name and freevar != name:
            continue
        try:
            value = cell.cell_contents
        except ValueError:
            return DELETED
        if freevar == name and target_name != name:
            if isinstance(value, _types.CellType):
                try:
                    return value.cell_contents
                except ValueError:
                    return DELETED
            return value
        if target_name == name:
            if isinstance(value, _types.CellType):
                return value
            return cell
        if isinstance(value, _types.CellType):
            try:
                return value.cell_contents
            except ValueError:
                return DELETED
        return value
    return _MISSING


def _resume_closure_contents(cell):
    try:
        value = cell.cell_contents
    except ValueError:
        return _MISSING
    if isinstance(value, _types.CellType):
        try:
            return value.cell_contents
        except ValueError:
            return DELETED
    if type(cell) is _types.CellType:
        return value
    return value


def _current_yieldfrom(owner):
    value = __dp_load_local_raw(owner, "_dp_yieldfrom")
    if value is DELETED:
        return None
    return value


def __dp_load_local_raw(gen, name):
    frame = _resolve_local_frame(gen)
    if isinstance(frame, dict):
        if name.startswith("_dp_yield_from_iter_") and name not in frame:
            return getattr(gen, "gi_yieldfrom", DELETED)
        return frame.get(name, DELETED)
    value = _resume_closure_value(gen, name)
    if value is _MISSING and name.startswith("_dp_yield_from_iter_"):
        return getattr(gen, "gi_yieldfrom", DELETED)
    if value is _MISSING:
        return DELETED
    return value


builtins.__dp_load_local_raw = __dp_load_local_raw


class _DpAsyncGenComplete(Exception):
    pass


builtins.__dp_AsyncGenComplete = _DpAsyncGenComplete


def raise_uncaught_async_generator_exception(exc):
    if isinstance(exc, StopIteration):
        raise RuntimeError("async generator raised StopIteration") from exc
    if isinstance(exc, StopAsyncIteration):
        raise RuntimeError("async generator raised StopAsyncIteration") from exc
    raise exc


def _dp_closed_generator_resume(_gen, _send_value, _resume_exc):
    raise StopIteration


def _dp_closed_async_generator_resume(_gen, _send_value, _resume_exc, _transport_sent):
    raise _DpAsyncGenComplete


def _dp_clear_owner_state(owner, *, async_gen):
    if hasattr(owner, "_pc"):
        owner._pc = 0
    if hasattr(owner, "gi_frame"):
        owner.gi_frame = None
    owner._dp_resume = (
        _dp_closed_async_generator_resume if async_gen else _dp_closed_generator_resume
    )


def _dp_is_cancelled_error(exc):
    asyncio_mod = sys.modules.get("asyncio")
    if asyncio_mod is None:
        return False
    cancelled_error = getattr(asyncio_mod, "CancelledError", None)
    return cancelled_error is not None and isinstance(exc, cancelled_error)


def _dp_reraise_control_flow(exc):
    if isinstance(exc, GeneratorExit) or _dp_is_cancelled_error(exc):
        raise exc.with_traceback(None)
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
        if exc.__context__ is None:
            resume = getattr(state, "_dp_resume", None)
            closure = getattr(resume, "__closure__", None)
            if closure is not None:
                for cell in reversed(tuple(closure)):
                    candidate = _resume_closure_contents(cell)
                    if candidate is _MISSING or candidate is DELETED:
                        continue
                    if isinstance(candidate, BaseException):
                        exc.__context__ = candidate
                        break
    except Exception:
        pass

class _DpGenerator:
    __slots__ = (
        "_dp_resume",
        "_pc",
        "__name__",
        "__qualname__",
        "gi_frame",
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
        self.__name__ = name
        self.__qualname__ = qualname
        self.gi_code = code

    def __iter__(self):
        return self

    def __next__(self):
        return self.send(None)

    def send(self, value):
        try:
            return self._dp_resume(self, value, NO_DEFAULT)
        except BaseException as exc:
            _dp_clear_owner_state(self, async_gen=False)
            _dp_reraise_control_flow(exc)

    def throw(self, typ=None, val=None, tb=None):
        if val is not None or tb is not None:
            raise TypeError(
                "DpGen.throw() does not support value/traceback in this mode"
            )
        exc = raise_from(typ, None)
        _attach_throw_context_from_state(self, exc)
        try:
            return self._dp_resume(self, NO_DEFAULT, exc)
        except BaseException as exc:
            _dp_clear_owner_state(self, async_gen=False)
            _dp_reraise_control_flow(exc)

    def close(self):
        try:
            self.throw(GeneratorExit)
        except (GeneratorExit, StopIteration):
            return None
        raise RuntimeError("generator ignored GeneratorExit")

    @property
    def gi_yieldfrom(self):
        return _current_yieldfrom(self)


class _DpClosureGenerator:
    __slots__ = (
        "_dp_resume",
        "__name__",
        "__qualname__",
        "gi_code",
    )

    def __init__(
        self,
        *,
        resume,
        name,
        qualname,
        code,
    ):
        self._dp_resume = resume
        self.__name__ = name
        self.__qualname__ = qualname
        self.gi_code = code

    def __iter__(self):
        return self

    def __next__(self):
        return self.send(None)

    def send(self, value):
        try:
            return self._dp_resume(self, value, NO_DEFAULT)
        except BaseException as exc:
            _dp_clear_owner_state(self, async_gen=False)
            _dp_reraise_control_flow(exc)

    def throw(self, typ=None, val=None, tb=None):
        if val is not None or tb is not None:
            raise TypeError(
                "DpGen.throw() does not support value/traceback in this mode"
            )
        exc = raise_from(typ, None)
        _attach_throw_context_from_state(self, exc)
        try:
            return self._dp_resume(self, NO_DEFAULT, exc)
        except BaseException as exc:
            _dp_clear_owner_state(self, async_gen=False)
            _dp_reraise_control_flow(exc)

    def close(self):
        try:
            self.throw(GeneratorExit)
        except (GeneratorExit, StopIteration):
            return None
        raise RuntimeError("generator ignored GeneratorExit")

    @property
    def gi_yieldfrom(self):
        return _current_yieldfrom(self)


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
        return getattr(self._dp_gen, "gi_frame", None)

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
        "__name__",
        "__qualname__",
        "gi_frame",
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
        self.__name__ = name
        self.__qualname__ = qualname
        self.gi_frame = gi_frame
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
            return self.gi_yieldfrom
        raise AttributeError(name)

    @property
    def gi_yieldfrom(self):
        return _current_yieldfrom(self)

    def asend(self, value):
        return _DpAsyncGenSend(self, value, NO_DEFAULT)

    def athrow(self, typ=None, val=None, tb=None):
        if val is not None or tb is not None:
            raise TypeError(
                "DpAsyncGen.athrow() does not support value/traceback in this mode"
            )
        exc = raise_from(typ, None)
        _attach_throw_context_from_state(self, exc)
        return _DpAsyncGenSend(self, NO_DEFAULT, exc)

    async def aclose(self):
        try:
            await self.athrow(GeneratorExit)
        except (GeneratorExit, StopAsyncIteration):
            return None
        raise RuntimeError("async generator ignored GeneratorExit")


class _DpClosureAsyncGenerator:
    __slots__ = (
        "_dp_resume",
        "__name__",
        "__qualname__",
        "ag_code",
    )

    def __init__(
        self,
        *,
        resume,
        name,
        qualname,
        code,
    ):
        self._dp_resume = resume
        self.__name__ = name
        self.__qualname__ = qualname
        self.ag_code = code

    def __aiter__(self):
        return self

    def __anext__(self):
        return self.asend(None)

    def __getattr__(self, name):
        if name == "ag_running":
            return False
        if name == "ag_frame":
            return None
        if name == "ag_await":
            return self.gi_yieldfrom
        raise AttributeError(name)

    @property
    def gi_yieldfrom(self):
        return _current_yieldfrom(self)

    def asend(self, value):
        return _DpAsyncGenSend(self, value, NO_DEFAULT)

    def athrow(self, typ=None, val=None, tb=None):
        if val is not None or tb is not None:
            raise TypeError(
                "DpAsyncGen.athrow() does not support value/traceback in this mode"
            )
        exc = raise_from(typ, None)
        _attach_throw_context_from_state(self, exc)
        return _DpAsyncGenSend(self, NO_DEFAULT, exc)

    async def aclose(self):
        try:
            await self.athrow(GeneratorExit)
        except (GeneratorExit, StopAsyncIteration):
            return None
        raise RuntimeError("async generator ignored GeneratorExit")


def _normalize_throw_args(typ, val=None, tb=None):
    if val is not None or tb is not None:
        raise TypeError(
            "DpAsyncGenSend.throw() does not support value/traceback in this mode"
        )
    return raise_from(typ, None)


class _DpAsyncGenSend:
    __slots__ = ("_dp_gen", "_dp_value", "_dp_resume_exc", "_dp_done")

    def __init__(self, gen, value, resume_exc):
        self._dp_gen = gen
        self._dp_value = value
        self._dp_resume_exc = resume_exc
        self._dp_done = False

    def __iter__(self):
        return self

    def __await__(self):
        return self

    def __next__(self):
        return self.send(None)

    def _dp_step(self, transport_sent):
        step_send_value = (
            transport_sent
            if _current_yieldfrom(self._dp_gen) is not None
            else self._dp_value
        )
        try:
            result = self._dp_gen._dp_resume(
                self._dp_gen,
                step_send_value,
                self._dp_resume_exc,
                transport_sent,
            )
        except _DpAsyncGenComplete:
            self._dp_done = True
            self._dp_resume_exc = NO_DEFAULT
            _dp_clear_owner_state(self._dp_gen, async_gen=True)
            raise StopAsyncIteration
        except BaseException as exc:
            self._dp_done = True
            self._dp_resume_exc = NO_DEFAULT
            _dp_clear_owner_state(self._dp_gen, async_gen=True)
            if _dp_is_cancelled_error(exc) or isinstance(exc, GeneratorExit):
                _dp_reraise_control_flow(exc)
            raise_uncaught_async_generator_exception(exc)
        self._dp_resume_exc = NO_DEFAULT
        if _current_yieldfrom(self._dp_gen) is None:
            self._dp_done = True
            raise StopIteration(result)
        return result

    def send(self, value):
        if self._dp_done:
            raise StopIteration
        if (
            value is not None
            and self._dp_value is None
            and self._dp_resume_exc is NO_DEFAULT
            and _current_yieldfrom(self._dp_gen) is None
        ):
            raise TypeError(
                "can't send non-None value to a just-started async generator"
            )
        return self._dp_step(value)

    def throw(self, typ, val=None, tb=None):
        if self._dp_done:
            raise _normalize_throw_args(typ, val, tb)
        self._dp_resume_exc = _normalize_throw_args(typ, val, tb)
        return self._dp_step(None)

    def close(self):
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


# TODO: very questionable
def float_from_literal(literal):
    # Preserve CPython's literal parsing for values that Rust rounds differently.
    return float(literal.replace("_", ""))


def class_lookup_cell(class_ns, name, cell):
    try:
        return class_ns[name]
    except KeyError:
        pass
    try:
        value = cell.cell_contents
    except ValueError as exc:
        raise NameError(
            f"cannot access free variable {name!r} where it is not associated with a value in enclosing scope"
        ) from exc
    if value is DELETED:
        raise NameError(
            f"cannot access free variable {name!r} where it is not associated with a value in enclosing scope"
        )
    return value


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


def globals():
    frame = sys._getframe(1)
    return frame.f_globals


builtins.__dp_globals = globals


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


_DP_CODE_WITH_FREEVARS_CACHE = {}


def code_with_freevars(names, is_async, is_generator):
    names = tuple(names)
    is_async = bool(is_async)
    is_generator = bool(is_generator)
    cache_key = (names, is_async, is_generator)
    cached = _DP_CODE_WITH_FREEVARS_CACHE.get(cache_key)
    if cached is not None:
        return cached
    for name in names:
        if not isinstance(name, str):
            raise TypeError(f"freevar names must be str, got {type(name)!r}")
        if not name.isidentifier() or _keyword.iskeyword(name):
            raise ValueError(f"invalid freevar name: {name!r}")
    if len(set(names)) != len(names):
        raise ValueError("freevar names must be unique")

    outer_lines = ["def __dp_make_code():"]
    for name in names:
        outer_lines.append(f"    {name} = None")
    if is_async:
        outer_lines.append("    async def wrapped(*args, __dp_entry=None, **kwargs):")
    else:
        outer_lines.append("    def wrapped(*args, __dp_entry=None, **kwargs):")
    if names:
        outer_lines.append("        if False:")
        for name in names:
            outer_lines.append(f"            {name}")
    if is_async and is_generator:
        outer_lines.append(
            "        async for __dp_item in __dp_entry(*args, **kwargs):"
        )
        outer_lines.append("            yield __dp_item")
    elif is_async:
        outer_lines.append("        return await __dp_entry(*args, **kwargs)")
    elif is_generator:
        outer_lines.append("        yield from __dp_entry(*args, **kwargs)")
    else:
        outer_lines.append("        return __dp_entry(*args, **kwargs)")
    outer_lines.append("    return wrapped.__code__")

    ns = {}
    exec("\n".join(outer_lines), {}, ns)
    code = ns["__dp_make_code"]()
    _DP_CODE_WITH_FREEVARS_CACHE[cache_key] = code
    return code


def _dp_entry_template(*args, **kwargs):
    raise RuntimeError("CLIF entry executed without vectorcall interception")


def _dp_gen_code_template(_it):
    while True:
        yield next(_it)


async def _dp_async_gen_code_template():
    if False:
        yield None


def make_function(
    function_id,
    kind,
    captures,
    param_defaults,
    module_globals=None,
    annotate_fn=None,
):
    if _jit_make_bb_function is None:
        raise RuntimeError(
            "JIT basic-block function instantiation requires a registered Rust constructor"
        )
    func = _jit_make_bb_function(
        function_id,
        captures,
        param_defaults,
        module_globals,
        annotate_fn,
    )
    if kind == "coroutine":
        func._is_coroutine = coroutines._is_coroutine

    return func


def make_closure_generator(function_id, resume, module_globals=None):
    if _jit_make_bb_generator is None:
        raise RuntimeError(
            "JIT basic-block generator construction requires a registered Rust constructor"
        )
    return _jit_make_bb_generator(function_id, resume, module_globals, async_gen=False)


def make_coroutine_from_generator(gen):
    return _DpCoroutine(gen)


def make_closure_async_generator(function_id, resume, module_globals=None):
    if _jit_make_bb_generator is None:
        raise RuntimeError(
            "JIT basic-block async generator construction requires a registered Rust constructor"
        )
    return _jit_make_bb_generator(function_id, resume, module_globals, async_gen=True)


def decode_literal_bytes(value):
    return value.decode("utf-8", "surrogatepass")


builtins.__dp_decode_literal_bytes = decode_literal_bytes


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
        class_cell = _types.CellType()
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


def _missing_context_protocol_message(
    obj,
    protocol: str,
    missing_method: str,
    alt_method_names: tuple[str, str],
    hint: str,
):
    cls = type(obj)
    module = getattr(cls, "__module__", None)
    qualname = getattr(cls, "__qualname__", cls.__name__)
    if module and module != "builtins":
        type_name = f"{module}.{qualname}"
    else:
        type_name = qualname
    message = (
        f"{type_name!r} object does not support the {protocol} protocol "
        f"(missed {missing_method} method)"
    )
    if _has_special_method(obj, alt_method_names[0]) or _has_special_method(
        obj, alt_method_names[1]
    ):
        message += hint
    return message


def contextmanager_enter(ctx):
    try:
        enter = _lookup_special_method(ctx, "__enter__")
    except AttributeError as exc:
        message = _missing_context_protocol_message(
            ctx,
            "context manager",
            "__enter__",
            ("__aenter__", "__aexit__"),
            " but it supports the asynchronous context manager protocol. Did you mean to use 'async with'?",
        )
        raise TypeError(message) from exc
    return enter()


def contextmanager_get_exit(cm):
    try:
        return _lookup_special_method(cm, "__exit__")
    except AttributeError as exc:
        message = _missing_context_protocol_message(
            cm,
            "context manager",
            "__exit__",
            ("__aenter__", "__aexit__"),
            " but it supports the asynchronous context manager protocol. Did you mean to use 'async with'?",
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
        message = _missing_context_protocol_message(
            ctx,
            "asynchronous context manager",
            "__aenter__",
            ("__enter__", "__exit__"),
            " but it supports the context manager protocol. Did you mean to use 'with'?",
        )
        raise TypeError(message) from exc
    await_iter = _ensure_awaitable(aenter(), "__aenter__")
    return await _AwaitIterWrapper(await_iter)


def asynccontextmanager_get_aexit(acm):
    try:
        return _lookup_special_method(acm, "__aexit__")
    except AttributeError as exc:
        message = _missing_context_protocol_message(
            acm,
            "asynchronous context manager",
            "__aexit__",
            ("__enter__", "__exit__"),
            " but it supports the context manager protocol. Did you mean to use 'with'?",
        )
        raise TypeError(message) from exc


async def asynccontextmanager_exit(exit_fn, exc_info: tuple | None):
    if exc_info is not None:
        exc_type, exc, tb = exc_info
        try:
            await_iter = _ensure_awaitable(
                exit_fn(*exc_info), "__aexit__", suppress_context=False
            )
            suppress = await _AwaitIterWrapper(await_iter)
            if suppress:
                exc.__traceback__ = None
                return None
            return exc
        finally:
            exc_info = None
            exc_type = None
            exc = None
            tb = None
    else:
        await_iter = _ensure_awaitable(exit_fn(None, None, None), "__aexit__")
        await _AwaitIterWrapper(await_iter)
        return None


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

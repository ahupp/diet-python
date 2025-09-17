# diet-python: disabled
import operator as _operator
import sys
import builtins
import types as _types
from typing import Any, Iterator, Optional, Tuple, Union, Literal, TypeVar, Callable, Awaitable

operator = _operator
add = _operator.add
sub = _operator.sub
mul = _operator.mul
matmul = _operator.matmul
truediv = _operator.truediv
floordiv = _operator.floordiv
mod = _operator.mod
pow = _operator.pow
lshift = _operator.lshift
rshift = _operator.rshift
or_ = _operator.or_
xor = _operator.xor
and_ = _operator.and_
getitem = _operator.getitem
setitem = _operator.setitem
delitem = _operator.delitem
iadd = _operator.iadd
isub = _operator.isub
imul = _operator.imul
imatmul = _operator.imatmul
itruediv = _operator.itruediv
imod = _operator.imod
ipow = _operator.ipow
ilshift = _operator.ilshift
irshift = _operator.irshift
ior = _operator.ior
ixor = _operator.ixor
iand = _operator.iand
ifloordiv = _operator.ifloordiv
pos = _operator.pos
neg = _operator.neg
invert = _operator.invert
not_ = _operator.not_
truth = _operator.truth
eq = _operator.eq
ne = _operator.ne
lt = _operator.lt
le = _operator.le
gt = _operator.gt
ge = _operator.ge
is_ = _operator.is_
is_not = _operator.is_not
contains = _operator.contains
next = builtins.next
iter = builtins.iter
aiter = builtins.aiter
anext = builtins.anext
isinstance = builtins.isinstance
setattr = builtins.setattr

def resolve_bases(bases):
    return _types.resolve_bases(bases)

def prepare_class(name, bases, kwds=None):
    if kwds is None:
        return _types.prepare_class(name, bases)
    return _types.prepare_class(name, bases, kwds)

def exc_info():
    return sys.exc_info()


def current_exception():
    return sys.exc_info()[1]


def check_stopiteration():
    if not isinstance(current_exception(), StopIteration):
        raise


def acheck_stopiteration():
    if not isinstance(current_exception(), StopAsyncIteration):
        raise


def raise_from(exc, cause):
    if exc is None:
        raise TypeError("exceptions must derive from BaseException")
    if isinstance(exc, type):
        if issubclass(exc, BaseException):
            exc = exc()
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
                cause = cause()
            else:
                raise TypeError("exception causes must derive from BaseException")
        elif not isinstance(cause, BaseException):
            raise TypeError("exception causes must derive from BaseException")
        exc.__cause__ = cause
    return exc


def import_(name, spec, fromlist=None, level=0):
    if fromlist is None:
        fromlist = []
    globals_dict = {"__spec__": spec}
    if spec is not None:
        globals_dict["__package__"] = spec.parent
        globals_dict["__name__"] = spec.name
    return builtins.__import__(name, globals_dict, {}, fromlist, level)


# Tags as ints for yield from state machine
RUNNING = 0
RETURN = 1

# Discriminated union for state
YFRunning = Tuple[Literal[RUNNING], Any, Optional[Any], Iterator[Any]]
YFReturn = Tuple[Literal[RETURN], Optional[Any], None, None]
YFState = Union[YFRunning, YFReturn]


def yield_from_init(iterable) -> YFState:
    it = iter(iterable)
    try:
        y = next(it)  # prime
    except StopIteration as e:
        return (RETURN, getattr(e, "value", None), None, None)
    else:
        return (RUNNING, y, None, it)


def yield_from_next(state: YFRunning, sent: Optional[Any]) -> YFState:
    """Advance one step given the value just sent into the outer generator.
       Must be called only while RUNNING."""
    tag, _y, _to_send, it = state
    assert tag == RUNNING and it is not None, "yield_from_next requires RUNNING state"

    try:
        if sent is None:
            y = next(it)
        else:
            send = getattr(it, "send", None)
            y = next(it) if send is None else send(sent)
    except StopIteration as e:
        return (RETURN, getattr(e, "value", None), None, None)
    else:
        return (RUNNING, y, None, it)


def yield_from_except(state: YFState, exc: BaseException) -> YFState:
    """Forward exceptions immediately to the subgenerator."""
    # Unpack first, then assert as requested
    tag, _y, _to_send, it = state
    assert tag == RUNNING and it is not None, "Invalid state for exception forwarding"

    if isinstance(exc, GeneratorExit):
        close = getattr(it, "close", None)
        if close is not None:
            try:
                close()
            finally:
                raise exc
        raise exc

    throw = getattr(it, "throw", None)
    if throw is None:
        raise exc

    try:
        y = throw(exc)
    except StopIteration as e:
        return (RETURN, getattr(e, "value", None), None, None)
    else:
        return (RUNNING, y, None, it)


T = TypeVar("T")
AWith = Tuple[T, Callable[[T, Any, Any, Any], Awaitable[bool]]]
With = Tuple[T, Callable[[T, Any, Any, Any], bool]]


async def with_aenter(ctx) -> AWith:
    enter = type(ctx).__aenter__
    exit = type(ctx).__aexit__
    var = await enter(ctx)
    return (var, exit)


async def with_aexit(state: AWith, exc_info: tuple | None):
    ctx, aexit = state
    if exc_info is not None:
        if not await aexit(ctx, *exc_info):
            raise
    else:
        await aexit(ctx, None, None, None)


def with_enter(ctx) -> With:
    enter = type(ctx).__enter__
    exit = type(ctx).__exit__
    var = enter(ctx)
    return (var, exit)


def with_exit(state: With, exc_info: tuple | None):
    ctx, aexit = state
    if exc_info is not None:
        if not aexit(ctx, *exc_info):
            raise
    else:
        aexit(ctx, None, None, None)


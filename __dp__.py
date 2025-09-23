# diet-python: disabled
import operator as _operator
import sys
import builtins
import types as _types

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
delattr = builtins.delattr
tuple = builtins.tuple
list = builtins.list
dict = builtins.dict
set = builtins.set
slice = builtins.slice


def unpack(arr, idx):
    try:
        return arr[idx]
    except IndexError as exc:
        raise ValueError from exc


def resolve_bases(bases):
    return _types.resolve_bases(bases)

def prepare_class(name, bases, kwds=None):
    if kwds is None:
        return _types.prepare_class(name, bases)
    return _types.prepare_class(name, bases, kwds)


def create_class(name, namespace_fn, bases, kwds=None):
    orig_bases = bases
    bases = resolve_bases(orig_bases)
    meta, ns, meta_kwds = prepare_class(name, bases, kwds)
    temp_ns = dict()

    def add_binding(binding_name: str, value):
        setitem(temp_ns, binding_name, value)
        setitem(ns, binding_name, value)
        return value

    namespace_fn(ns, add_binding)
    if orig_bases is not bases and "__orig_bases__" not in ns:
        ns["__orig_bases__"] = orig_bases
    return meta(name, bases, ns, **meta_kwds)

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
    module = builtins.__import__(name, globals_dict, {}, fromlist, level)
    if fromlist:
        module_name = getattr(module, "__name__", name)
        module_file = getattr(module, "__file__", None)
        for attr in fromlist:
            if attr == "*":
                continue
            if (
                module_name == name
                and "." in module_name
                and module_name.rsplit(".", 1)[1] == attr
            ):
                continue
            try:
                getattr(module, attr)
            except AttributeError as exc:
                if module_name:
                    submodule = sys.modules.get(f"{module_name}.{attr}")
                    if submodule is not None:
                        setattr(module, attr, submodule)
                        continue
                message = f"cannot import name {attr!r} from {module_name!r}"
                if module_file is not None:
                    message = f"{message} ({module_file})"
                raise ImportError(message, name=module_name, path=module_file) from exc
    return module


# Tags as ints for yield from state machine
RUNNING = 0
RETURN = 1

# Discriminated union for state
def yield_from_init(iterable):
    it = iter(iterable)
    try:
        y = next(it)  # prime
    except StopIteration as e:
        return (RETURN, getattr(e, "value", None), None, None)
    else:
        return (RUNNING, y, None, it)


def yield_from_next(state, sent):
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


def yield_from_except(state, exc: BaseException):
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


async def with_aenter(ctx):
    enter = type(ctx).__aenter__
    exit = type(ctx).__aexit__
    var = await enter(ctx)
    return (var, (ctx, exit))


async def with_aexit(state, exc_info: tuple | None):
    ctx, aexit = state
    if exc_info is not None:
        if not await aexit(ctx, *exc_info):
            raise
    else:
        await aexit(ctx, None, None, None)


def with_enter(ctx):
    enter = type(ctx).__enter__
    exit = type(ctx).__exit__
    var = enter(ctx)
    return (var, (ctx, exit))


def with_exit(state, exc_info: tuple | None):
    ctx, aexit = state
    if exc_info is not None:
        if not aexit(ctx, *exc_info):
            raise
    else:
        aexit(ctx, None, None, None)


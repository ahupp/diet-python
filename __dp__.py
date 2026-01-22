# diet-python: disabled
import operator as _operator
import sys
import builtins
import types as _types
import warnings

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


# TODO: very questionable
def float_from_literal(literal):
    # Preserve CPython's literal parsing for values that Rust rounds differently.
    return float(literal.replace("_", ""))



_MISSING = object()


def class_lookup(class_ns, name, lookup_fn):
    try:
        return class_ns[name]
    except KeyError:
        return lookup_fn()


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


def resolve_bases(bases):
    return _types.resolve_bases(bases)

def prepare_class(name, bases, kwds=None):
    if kwds is None:
        return _types.prepare_class(name, bases)
    return _types.prepare_class(name, bases, kwds)



_MISSING_CLASSCELL = object()
_EMPTY_CLASSCELL = object()



def make_classcell(value=_MISSING_CLASSCELL):
    if value is _MISSING_CLASSCELL:
        return _types.CellType()
    def inner():
        return value
    return inner.__closure__[0]


def empty_classcell():
    return _EMPTY_CLASSCELL


def super_(super_fn, class_namespace, instance_or_cls):
    """Return a super() proxy using the defining class, falling back to cls during class creation."""
    defining = None
    if class_namespace is _EMPTY_CLASSCELL:
        raise RuntimeError("empty __class__ cell")
    if isinstance(class_namespace, _types.CellType):
        try:
            defining = class_namespace.cell_contents
        except ValueError:
            raise RuntimeError("empty __class__ cell") from None
        if defining is _EMPTY_CLASSCELL:
            raise RuntimeError("empty __class__ cell")
    else:
        try:
            locals_dict = object.__getattribute__(class_namespace, "_locals")
            defining = locals_dict.get("__dp_class")
        except Exception:
            defining = None
    if defining is None and isinstance(class_namespace, type):
        defining = class_namespace
    if defining is None:
        defining = instance_or_cls
    return super(defining, instance_or_cls)


def call_super(super_fn, cls, instance_or_cls):
    if super_fn is builtins.super:
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



def update_fn(func, scope, name):
    if scope is None:
        qualname = name
    else:
        qualname = f"{scope}.{name}"
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


typing = None
templatelib = None

def init_lazy_imports():
    global typing, templatelib
    typing = builtins.__import__("typing")
    templatelib = builtins.__import__(
            "string.templatelib", fromlist=["templatelib"]
    )


def create_class(name, namespace_fn, bases, kwds, requires_class_cell):
    resolved_bases = resolve_bases(bases)
    meta, ns, meta_kwds = prepare_class(name, bases, kwds)

    class_cell = ns.get("__classcell__", None)
    if requires_class_cell and class_cell is None:
        class_cell = make_classcell()
        ns["__classcell__"] = class_cell

    namespace_fn(ns, class_cell)

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
    exc = sys.exception()
    if exc is None:
        return None
    return (type(exc), exc, exc.__traceback__)


def current_exception():
    exc = sys.exception()
    if exc is None:
        return None
    if isinstance(exc, RecursionError):
        return exc
    tb = _strip_dp_frames(exc.__traceback__)
    if tb is not exc.__traceback__:
        exc = exc.with_traceback(tb)
    return exc


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


@_types.coroutine
def _await_from_iter(iterator):
    return (yield from iterator)


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


async def anext(iterator):
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
    return await _await_from_iter(await_iter)




def raise_from(exc, cause):
    CancelledError = None
    asyncio_mod = sys.modules.get("asyncio")
    if asyncio_mod is not None:
        CancelledError = getattr(asyncio_mod, "CancelledError", None)
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
        if CancelledError is not None and type(cause) is CancelledError:
            cause = cause.with_traceback(None)
        exc.__cause__ = cause
        exc.__suppress_context__ = True
    return exc


def import_(name, spec, fromlist=None, level=0):
    if fromlist is None:
        fromlist = []
    globals_dict = {"__spec__": spec}
    if spec is not None:
        globals_dict["__package__"] = spec.parent
        globals_dict["__name__"] = spec.name
    try:
        return builtins.__import__(name, globals_dict, {}, fromlist, level)
    except Exception as exc:
        tb = _strip_dp_frames(exc.__traceback__)
        raise exc.with_traceback(tb)


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
            tb = _strip_dp_frames(exc.__traceback__)
            raise import_error.with_traceback(tb) from None
        module_name = module_name or "<unknown module name>"
        module_file = getattr(module, "__file__", None)
        message = f"cannot import name {attr!r} from {module_name!r}"
        if module_file is not None:
            message = f"{message} ({module_file})"
        else:
            message = f"{message} (unknown location)"
        import_error = ImportError(message, name=module_name, path=module_file)
        tb = _strip_dp_frames(exc.__traceback__)
        raise import_error.with_traceback(tb) from None


def _strip_dp_frames(tb):
    if tb is None:
        return None

    internal_files = {__file__}
    hook = sys.modules.get("diet_import_hook")
    if hook is not None:
        hook_file = getattr(hook, "__file__", None)
        if hook_file:
            internal_files.add(hook_file)
    frames = []
    changed = False
    current = tb
    while current is not None:
        if current.tb_frame.f_code.co_filename in internal_files:
            changed = True
        else:
            frames.append((current.tb_frame, current.tb_lasti, current.tb_lineno))
        current = current.tb_next

    if not changed:
        return tb

    stripped = None
    for frame, lasti, lineno in reversed(frames):
        stripped = _types.TracebackType(stripped, frame, lasti, lineno)
    return stripped



def with_enter(ctx):
    try:
        enter = ctx.__enter__
    except AttributeError as exc:
        raise TypeError("the context manager protocol requires __enter__") from exc
    try:
        exit = ctx.__exit__
    except AttributeError as exc:
        raise TypeError("the context manager protocol requires __exit__") from exc
    var = enter()
    return (var, exit)


def with_exit(exit_fn, exc_info: tuple | None):
    if exc_info is not None:
        exc_type, exc, tb = exc_info
        try:
            suppress = exit_fn(*exc_info)
            if suppress:
                exc.__traceback__ = None
                return
            raise exc.with_traceback(tb)
        finally:
            # Clear the reference for GC in long-lived frames.
            exc_info = None
            exc_type = None
            exc = None
            tb = None
    else:
        exit_fn(None, None, None)


def _ensure_awaitable(awaitable, method_name: str):
    try:
        iterator = awaitable.__await__()
    except AttributeError:
        awaitable_type = type(awaitable).__name__
        awaitable = None
        raise TypeError(
            f"object returned from {method_name} does not implement __await__: {awaitable_type}"
        ) from None
    except Exception as exc:
        awaitable_type = type(awaitable).__name__
        awaitable = None
        raise TypeError(
            f"object returned from {method_name} does not implement __await__: {awaitable_type}"
        ) from exc
    if not hasattr(iterator, "__next__"):
        awaitable_type = type(awaitable).__name__
        awaitable = None
        raise TypeError(
            f"object returned from {method_name} does not implement __await__: {awaitable_type}"
        ) from None
    return iterator


async def with_aenter(ctx):
    try:
        aenter = ctx.__aenter__
    except AttributeError as exc:
        raise TypeError("the asynchronous context manager protocol requires __aenter__") from exc
    try:
        aexit = ctx.__aexit__
    except AttributeError as exc:
        raise TypeError("the asynchronous context manager protocol requires __aexit__") from exc
    await_iter = _ensure_awaitable(aenter(), "__aenter__")
    var = await _await_from_iter(await_iter)
    return (var, aexit)


async def with_aexit(exit_fn, exc_info: tuple | None):
    if exc_info is not None:
        exc_type, exc, tb = exc_info
        try:
            await_iter = _ensure_awaitable(exit_fn(*exc_info), "__aexit__")
            suppress = await _await_from_iter(await_iter)
            if suppress:
                exc.__traceback__ = None
                return
            raise exc.with_traceback(tb)
        finally:
            exc_info = None
            exc_type = None
            exc = None
            tb = None
    else:
        await_iter = _ensure_awaitable(exit_fn(None, None, None), "__aexit__")
        await _await_from_iter(await_iter)

def cleanup_dp_globals(globals_dict):    
    for _dp_name in list(globals_dict):
        if _dp_name.startswith("_dp_"):
            del globals_dict[_dp_name]

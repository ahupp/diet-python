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


def missing_name(name):
    raise NameError(name)


def float_from_literal(literal):
    # Preserve CPython's literal parsing for values that Rust rounds differently.
    return float(literal.replace("_", ""))




def global_(globals_dict, name):
    if name in globals_dict:
        return globals_dict[name]

    if hasattr(builtins, name):
        return getattr(builtins, name)

    return missing_name(name)


def class_lookup(class_ns, globals_dict, name):
    try:
        return class_ns[name]
    except KeyError:
        return global_(globals_dict, name)


def _validate_exception_type(exc_type):
    if isinstance(exc_type, tuple):
        for entry in exc_type:
            _validate_exception_type(entry)
        return
    if isinstance(exc_type, type) and issubclass(exc_type, BaseException):
        return
    raise TypeError("catching classes that do not inherit from BaseException is not allowed")


def exception_matches(exc, exc_type):
    _validate_exception_type(exc_type)
    return isinstance(exc, exc_type)


def unpack(iterable, spec):
    iterator = iter(iterable)

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


def super_(class_namespace, instance_or_cls):
    """Return a super() proxy using the defining class, falling back to cls during class creation."""
    defining = None
    try:
        locals_dict = object.__getattribute__(class_namespace, "_locals")
        defining = locals_dict.get("__dp_class")
    except Exception:
        defining = None
    if defining is None:
        defining = instance_or_cls
    return super(defining, instance_or_cls)


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


class _ClassNamespace:
    __slots__ = ("_namespace", "_locals")

    def __init__(self, namespace):
        self._namespace = namespace
        self._locals = {}

    def __getattribute__(self, name):
        if name in ("_namespace", "_locals", "__slots__"):
            return object.__getattribute__(self, name)

        locals_dict = object.__getattribute__(self, "_locals")
        if name in locals_dict:
            return locals_dict[name]

        namespace = object.__getattribute__(self, "_namespace")
        if name in namespace:
            return namespace[name]

        return object.__getattribute__(self, name)

    def __getattr__(self, name):
        try:
            return self[name]
        except KeyError as exc:
            raise AttributeError(name) from exc

    def __setattr__(self, name, value):
        if name in self.__slots__:
            return super().__setattr__(name, value)
        self[name] = value

    def __delattr__(self, name):
        if name in self.__slots__:
            return super().__delattr__(name)
        try:
            del self[name]
        except KeyError as exc:
            raise AttributeError(name) from exc

    def __setitem__(self, name, value):
        setitem(self._namespace, name, value)
        try:
            stored = self._namespace[name]
        except Exception:
            stored = value
        setitem(self._locals, name, stored)
        return stored

    def __getitem__(self, name, *rest):
        if rest:
            name = (name, *rest)
        if name in self._locals:
            return self._locals[name]
        return self._namespace[name]

    def __delitem__(self, name):
        if name in self._locals:
            del self._locals[name]
        del self._namespace[name]

    def get(self, name, default=None):
        if name in self._locals:
            return self._locals.get(name, default)
        return self._namespace.get(name, default)

def _set_qualname(obj, qualname):
    try:
        obj.__qualname__ = qualname
    except (AttributeError, TypeError):
        pass


def _update_qualname(owner_module, owner_qualname, attr_name, value):
    target = f"{owner_qualname}.{attr_name}"

    def update_if_local(obj):
        if getattr(obj, "__module__", None) != owner_module:
            return
        qualname = getattr(obj, "__qualname__", None)
        if not isinstance(qualname, str):
            return
        if qualname.startswith("_dp_"):
            return
        if "<locals>" not in qualname:
            return
        _set_qualname(obj, target)
        if isinstance(obj, _types.FunctionType):
            try:
                obj.__code__ = obj.__code__.replace(co_qualname=target)
            except (AttributeError, ValueError):
                pass

    if isinstance(value, staticmethod):
        _update_qualname(owner_module, owner_qualname, attr_name, value.__func__)
    elif isinstance(value, classmethod):
        _update_qualname(owner_module, owner_qualname, attr_name, value.__func__)
    elif isinstance(value, property):
        if value.fget is not None:
            update_if_local(value.fget)
        if value.fset is not None:
            update_if_local(value.fset)
        if value.fdel is not None:
            update_if_local(value.fdel)
    elif isinstance(value, _types.FunctionType):
        update_if_local(value)
    else:
        wrapped = getattr(value, "__wrapped__", None)
        if wrapped is not None and wrapped is not value:
            _update_qualname(owner_module, owner_qualname, attr_name, wrapped)
        update_if_local(value)


_typing = None


def _typing_module():
    # Lazy import to avoid circular import when typing imports __dp__ mid-init.
    global _typing
    if _typing is None:
        _typing = builtins.__import__("typing")
    return _typing


def _normalize_constraints(constraints):
    if constraints is None:
        return ()
    if isinstance(constraints, tuple):
        return constraints
    return (constraints,)


def type_param_typevar(name, bound=None, default=None, constraints=None):
    module = _typing_module()
    args = _normalize_constraints(constraints)
    kwargs = {}
    if bound is not None:
        kwargs["bound"] = bound
    if default is not None:
        kwargs["default"] = default
    return module.TypeVar(name, *args, **kwargs)


def type_param_typevar_tuple(name, default=None):
    module = _typing_module()
    if default is None:
        return module.TypeVarTuple(name)
    return module.TypeVarTuple(name, default=default)


def type_param_param_spec(name, default=None):
    module = _typing_module()
    if default is None:
        return module.ParamSpec(name)
    return module.ParamSpec(name, default=default)


def create_class(name, namespace_fn, bases, kwds=None):
    orig_bases = bases
    bases = resolve_bases(orig_bases)
    meta, ns, meta_kwds = prepare_class(name, bases, kwds)

    namespace = _ClassNamespace(ns)
    namespace_fn(namespace)
    qualname = ns.get("__qualname__", name)
    module_name = ns.get("__module__")
    for attr_name, value in list(ns.items()):
        if attr_name == "__qualname__":
            continue
        _update_qualname(module_name, qualname, attr_name, value)
    if orig_bases is not bases and "__orig_bases__" not in ns:
        ns["__orig_bases__"] = orig_bases
    cls = meta(name, bases, ns, **meta_kwds)
    namespace._locals["__dp_class"] = cls
    return cls

def exc_info():
    exc = sys.exception()
    if exc is None:
        return (None, None, None)
    return (type(exc), exc, exc.__traceback__)


def current_exception():
    exc = sys.exception()
    if exc is None:
        return None
    tb = _strip_dp_frames(exc.__traceback__)
    if tb is not exc.__traceback__:
        exc = exc.with_traceback(tb)
    return exc


def check_stopiteration():
    if not isinstance(current_exception(), StopIteration):
        raise


def acheck_stopiteration():
    if not isinstance(current_exception(), StopAsyncIteration):
        raise


def raise_from(exc, cause):
    from asyncio import CancelledError
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
        if type(cause) is CancelledError:
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

    def strip(current):
        if current is None:
            return None
        next_tb = strip(current.tb_next)
        if current.tb_frame.f_code.co_filename in internal_files:
            return next_tb
        return _types.TracebackType(
            next_tb,
            current.tb_frame,
            current.tb_lasti,
            current.tb_lineno,
        )

    return strip(tb)


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
    enter = ctx.__aenter__
    exit = ctx.__aexit__
    var = await enter()
    return (var, exit)


async def with_aexit(aexit_fn, exc_info: tuple | None):
    if exc_info is not None:
        try:
            if not await aexit_fn(*exc_info):
                raise
        finally:
            # Clear the reference for GC in long-lived frames.
            exc_info = None
    else:
        await aexit_fn(None, None, None)


def with_enter(ctx):
    enter = ctx.__enter__
    exit = ctx.__exit__
    var = enter()
    return (var, exit)


def with_exit(exit_fn, exc_info: tuple | None):
    if exc_info is not None:
        try:
            if not exit_fn(*exc_info):
                raise
        finally:
            # Clear the reference for GC in long-lived frames.
            exc_info = None
    else:
        exit_fn(None, None, None)

# diet-python: disabled
import operator as _operator
import sys
import builtins
import types as _types

_TYPEVAR_SUPPORTS_DEFAULT = False
_PARAMSPEC_SUPPORTS_DEFAULT = False
_TYPEVAR_TUPLE_SUPPORTS_DEFAULT = False
_TYPING_FEATURES_INITIALIZED = False
_typing = None

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


def global_(globals_dict, name):
    if name in globals_dict:
        return globals_dict[name]

    if hasattr(builtins, name):
        return getattr(builtins, name)

    return missing_name(name)


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
        setitem(self._locals, name, value)
        setitem(self._namespace, name, value)
        return value

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


class Scope:
    __slots__ = ("_scope", "_fallback")

    def __init__(self, scope=None, fallback=None):
        if scope is None:
            scope = {}
        self._scope = scope
        self._fallback = fallback

    def __getattribute__(self, name):
        if name in ("_scope", "_fallback", "__slots__"):
            return object.__getattribute__(self, name)
        scope = object.__getattribute__(self, "_scope")
        if name in scope:
            return scope[name]
        fallback = object.__getattribute__(self, "_fallback")
        if fallback is not None:
            if isinstance(fallback, dict):
                if name in fallback:
                    return fallback[name]
            elif hasattr(fallback, name):
                return getattr(fallback, name)
        raise AttributeError(name)

    def __setattr__(self, name, value):
        if name in self.__slots__:
            return super().__setattr__(name, value)
        self._scope[name] = value

    def __delattr__(self, name):
        if name in self.__slots__:
            return super().__delattr__(name)
        try:
            del self._scope[name]
        except KeyError as exc:
            raise AttributeError(name) from exc


def _set_qualname(obj, qualname):
    try:
        obj.__qualname__ = qualname
    except (AttributeError, TypeError):
        pass


def _update_qualname(owner_qualname, attr_name, value):
    target = f"{owner_qualname}.{attr_name}"
    if isinstance(value, staticmethod):
        _update_qualname(owner_qualname, attr_name, value.__func__)
    elif isinstance(value, classmethod):
        _update_qualname(owner_qualname, attr_name, value.__func__)
    elif isinstance(value, property):
        if value.fget is not None:
            _set_qualname(value.fget, target)
        if value.fset is not None:
            _set_qualname(value.fset, target)
        if value.fdel is not None:
            _set_qualname(value.fdel, target)
    elif isinstance(value, _types.FunctionType):
        _set_qualname(value, target)


def _ensure_typing_module():
    global _typing
    module = _typing
    if module is None:
        module = sys.modules.get("typing")
        if module is None:
            module = builtins.__import__("typing")
        _typing = module
    return module


def _ensure_typing_features(module):
    global _TYPEVAR_SUPPORTS_DEFAULT, _PARAMSPEC_SUPPORTS_DEFAULT
    global _TYPEVAR_TUPLE_SUPPORTS_DEFAULT, _TYPING_FEATURES_INITIALIZED
    if _TYPING_FEATURES_INITIALIZED:
        return

    try:
        module.TypeVar("_dp_default_probe", default=int)
    except TypeError:
        _TYPEVAR_SUPPORTS_DEFAULT = False
    else:
        _TYPEVAR_SUPPORTS_DEFAULT = True

    try:
        module.ParamSpec("_dp_param_spec_default_probe", default=int)
    except TypeError:
        _PARAMSPEC_SUPPORTS_DEFAULT = False
    else:
        _PARAMSPEC_SUPPORTS_DEFAULT = True

    try:
        module.TypeVarTuple("_dp_typevartuple_default_probe", default=())
    except TypeError:
        _TYPEVAR_TUPLE_SUPPORTS_DEFAULT = False
    else:
        _TYPEVAR_TUPLE_SUPPORTS_DEFAULT = True

    _TYPING_FEATURES_INITIALIZED = True


def _normalize_constraints(constraints):
    if constraints is None:
        return ()
    if isinstance(constraints, tuple):
        return constraints
    return (constraints,)


def type_param_typevar(name, bound=None, default=None, constraints=None):
    module = _ensure_typing_module()
    _ensure_typing_features(module)
    args = _normalize_constraints(constraints)
    kwargs = {}
    if bound is not None:
        kwargs["bound"] = bound
    if default is not None and _TYPEVAR_SUPPORTS_DEFAULT:
        kwargs["default"] = default
    try:
        return module.TypeVar(name, *args, **kwargs)
    except TypeError:
        if "default" in kwargs and not _TYPEVAR_SUPPORTS_DEFAULT:
            kwargs.pop("default")
            return module.TypeVar(name, *args, **kwargs)
        raise


def type_param_typevar_tuple(name, default=None):
    module = _ensure_typing_module()
    _ensure_typing_features(module)
    if default is not None and _TYPEVAR_TUPLE_SUPPORTS_DEFAULT:
        return module.TypeVarTuple(name, default=default)
    return module.TypeVarTuple(name)


def type_param_param_spec(name, default=None):
    module = _ensure_typing_module()
    _ensure_typing_features(module)
    if default is not None and _PARAMSPEC_SUPPORTS_DEFAULT:
        return module.ParamSpec(name, default=default)
    return module.ParamSpec(name)


def create_class(name, namespace_fn, bases, kwds=None):
    orig_bases = bases
    bases = resolve_bases(orig_bases)
    meta, ns, meta_kwds = prepare_class(name, bases, kwds)

    namespace = _ClassNamespace(ns)
    namespace_fn(namespace)
    qualname = ns.get("__qualname__", name)
    for attr_name, value in list(ns.items()):
        if attr_name == "__qualname__":
            continue
        _update_qualname(qualname, attr_name, value)
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
        module_dict = getattr(module, "__dict__", None)
        warned = False
        for attr in fromlist:
            if attr == "*":
                continue
            if (
                module_name == name
                and "." in module_name
                and module_name.rsplit(".", 1)[1] == attr
            ):
                continue
            value = None
            if not warned:
                try:
                    value = getattr(module, attr)
                except AttributeError as exc:
                    if module_name:
                        submodule = sys.modules.get(f"{module_name}.{attr}")
                        if submodule is not None:
                            setattr(module, attr, submodule)
                            warned = True
                            continue
                    message = f"cannot import name {attr!r} from {module_name!r}"
                    if module_file is not None:
                        message = f"{message} ({module_file})"
                    raise ImportError(message, name=module_name, path=module_file) from exc
                warned = True
            elif module_dict is not None and attr in module_dict:
                value = module_dict[attr]
            else:
                try:
                    value = getattr(module, attr)
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
            if module_dict is not None and attr not in module_dict:
                setattr(module, attr, value)
    return module


def import_attr(module, attr):
    module_dict = getattr(module, "__dict__", None)
    if module_dict is not None and attr in module_dict:
        return module_dict[attr]
    return getattr(module, attr)


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

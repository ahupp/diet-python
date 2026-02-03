# diet-python: disabled
import operator as _operator
import collections.abc as _abc
import reprlib
import sys
import builtins
import types as _types
import warnings

class _DpProxy:
    def __init__(self, module):
        object.__setattr__(self, "_module", module)

    def __getattr__(self, name):
        return getattr(self._module, name)

    def __setattr__(self, name, value):
        return setattr(self._module, name, value)

    def __repr__(self):
        return repr(self._module)

    def __deepcopy__(self, memo):
        return self

if not isinstance(getattr(builtins, "__dp__", None), _DpProxy):
    builtins.__dp__ = _DpProxy(sys.modules[__name__])

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
_DP_CELL_PREFIX = "_dp_cell_"



def class_lookup_cell(class_ns, name, cell):
    try:
        return class_ns[name]
    except KeyError:
        try:
            return load_cell(cell)
        except AttributeError:
            return cell


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



_MISSING_CELL = object()


def make_cell(value=_MISSING_CELL):
    cell = _types.CellType()
    if value is not _MISSING_CELL:
        cell.cell_contents = value
    return cell

def load_cell(cell):
    if isinstance(cell, _types.CellType):
        try:
            return cell.cell_contents
        except ValueError as exc:
            raise UnboundLocalError("local variable referenced before assignment") from exc
    return cell

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


class FrameLocalsProxy(_abc.MutableMapping):
    def __init__(self, frame, /):
        if not isinstance(frame, _types.FrameType):
            raise TypeError("expected a frame object")
        self.frame = frame

    def _is_local_name(self, name):
        if not isinstance(name, str):
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
        return _normalize_mapping(self.frame.f_locals)

    def __getitem__(self, name):
        locals_map = self.frame.f_locals
        if isinstance(name, str):
            if not name.startswith(_DP_CELL_PREFIX):
                cell_name = _DP_CELL_PREFIX + name
                if cell_name in locals_map:
                    value = locals_map[cell_name]
                    if isinstance(value, _types.CellType):
                        try:
                            return value.cell_contents
                        except ValueError:
                            raise KeyError(name)
                    return value
            if name in locals_map:
                value = locals_map[name]
                if isinstance(value, _types.CellType):
                    try:
                        return value.cell_contents
                    except ValueError:
                        raise KeyError(name)
                return value
            raise KeyError(name)
        return locals_map[name]

    def __setitem__(self, name, value):
        locals_map = self.frame.f_locals
        if isinstance(name, str) and not name.startswith(_DP_CELL_PREFIX):
            cell_name = _DP_CELL_PREFIX + name
            if cell_name in locals_map and isinstance(locals_map[cell_name], _types.CellType):
                locals_map[cell_name].cell_contents = value
                return
        locals_map[name] = value

    def __delitem__(self, name):
        if self._is_local_name(name):
            raise ValueError("cannot remove local variables from FrameLocalsProxy")
        locals_map = self.frame.f_locals
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

    def __iter__(self):
        return iter(self._snapshot())

    def __len__(self):
        return len(self._snapshot())

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
        if isinstance(other, FrameLocalsProxy):
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

class GlobalsProxy(_abc.MutableMapping):
    def __init__(self, globals_dict):
        self._globals = globals_dict

    def __getitem__(self, name):
        if name.startswith("_dp_"):
            raise KeyError(name)
        if name in self._globals:
            value = self._globals[name]
        else:
            cell_name = _DP_CELL_PREFIX + name
            if cell_name not in self._globals:
                raise KeyError(name)
            value = self._globals[cell_name]
        if isinstance(value, _types.CellType):
            try:
                return value.cell_contents
            except ValueError:
                raise KeyError(name)
        return value

    def __setitem__(self, name, value):
        self._globals[name] = value

    def __delitem__(self, name):
        del self._globals[name]

    def __iter__(self):
        return iter(_normalize_mapping(self._globals))

    def __len__(self):
        return len(_normalize_mapping(self._globals))

    def __contains__(self, name):
        try:
            self[name]
        except KeyError:
            return False
        return True

    def keys(self):
        return _normalize_mapping(self._globals).keys()

    def items(self):
        return _normalize_mapping(self._globals).items()

    def values(self):
        return _normalize_mapping(self._globals).values()

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
    if globals is None:
        frame = sys._getframe(1)
        globals = frame.f_globals
        if locals is None:
            locals = _normalize_mapping(frame.f_locals)
    else:
        if isinstance(globals, GlobalsProxy):
            globals = globals._globals
        if locals is None:
            locals = globals
    if isinstance(locals, LocalsProxy):
        locals = _normalize_mapping(locals.frame.f_locals)
    elif isinstance(locals, dict) and any(
        name.startswith(_DP_CELL_PREFIX) for name in locals
    ):
        locals = _normalize_mapping(locals)
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


def truth(value):
    return builtins.bool(value)


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
    if "__annotate__" not in ns:
        meta_annotate = getattr(meta, "__annotate__", None)
        if meta_annotate is not None:
            ns["__annotate__"] = None

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


if "annotationlib" in sys.modules:
    _patch_annotationlib(sys.modules["annotationlib"])


def import_(name, spec, fromlist=None, level=0):
    if fromlist is None:
        fromlist = []
    globals_dict = {"__spec__": spec}
    if spec is not None:
        globals_dict["__package__"] = spec.parent
        globals_dict["__name__"] = spec.name
    try:
        module = builtins.__import__(name, globals_dict, {}, fromlist, level)
        if name == "annotationlib":
            _patch_annotationlib(module)
        return module
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
            raise exc.with_traceback(tb)
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

def cleanup_dp_globals(globals_dict):    
    for _dp_name in list(globals_dict):
        if _dp_name.startswith("_dp_"):
            if _dp_name in ("_dp_typing", "_dp_templatelib"):
                continue
            del globals_dict[_dp_name]

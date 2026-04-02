# diet-python: disabled

_MISSING = object()


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


def pow(lhs, rhs):
    return _oper("__pow__", "__rpow__", "**", lhs, rhs)


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


def ipow(lhs, rhs):
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

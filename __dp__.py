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
neg = _operator.neg
invert = _operator.invert
not_ = _operator.not_
eq = _operator.eq
ne = _operator.ne
lt = _operator.lt
gt = _operator.gt
is_not = _operator.is_not
contains = _operator.contains
next = builtins.next
iter = builtins.iter
aiter = builtins.aiter
anext = builtins.anext

def resolve_bases(bases):
    return _types.resolve_bases(bases)

def prepare_class(name, bases, kwds=None):
    if kwds is None:
        return _types.prepare_class(name, bases)
    return _types.prepare_class(name, bases, kwds)

def exc_info():
    return sys.exc_info()


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
    return builtins.__import__(name, {"__spec__": spec}, {}, fromlist, level)


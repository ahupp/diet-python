# diet-python: disabled
import operator
import sys
import builtins
import types as _types

operator = operator
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

def set_classcell(cell, cls):
    cell.cell_contents = cls

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


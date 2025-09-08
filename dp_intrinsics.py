# diet-python: disabled
import operator
import sys
import builtins

operator = operator
next = builtins.next
iter = builtins.iter
aiter = builtins.aiter
anext = builtins.anext

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
    raise exc


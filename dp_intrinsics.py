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


from tests._integration import transformed_module


def test_frame_locals_updates_locals(tmp_path):
    source = """
import sys

def run():
    x = 1
    f_locals = sys._getframe().f_locals
    f_locals['x'] = 2
    f_locals['y'] = 3
    return x, locals()['y'], set(f_locals.keys())
"""
    with transformed_module(tmp_path, "frame_locals_basic", source) as module:
        x, y, keys = module.run()
        assert x == 2
        assert y == 3
        assert 'x' in keys
        assert 'y' in keys


def test_frame_locals_updates_closure(tmp_path):
    source = """
import sys

def outer():
    x = 1
    def inner():
        nonlocal x
        d = sys._getframe().f_locals
        d['x'] = 2
    inner()
    return x
"""
    with transformed_module(tmp_path, "frame_locals_closure", source) as module:
        assert module.outer() == 2


def test_frame_locals_reversed_keys(tmp_path):
    source = """
import sys

def run():
    x = 1
    y = 2
    d = sys._getframe().f_locals
    return list(reversed(d)), list(reversed(d.keys()))
"""
    with transformed_module(tmp_path, "frame_locals_reversed_keys", source) as module:
        rev_locals, rev_keys = module.run()
        assert rev_locals == rev_keys


def test_frame_locals_proxy_constructor(tmp_path):
    source = """
import sys

def run():
    FrameLocalsProxy = type(sys._getframe().f_locals)
    errors = []
    try:
        FrameLocalsProxy()
    except TypeError:
        errors.append("no-args")
    try:
        FrameLocalsProxy(123)
    except TypeError:
        errors.append("wrong-type")
    try:
        FrameLocalsProxy(frame=sys._getframe())
    except TypeError:
        errors.append("keyword")
    proxy = FrameLocalsProxy(sys._getframe())
    return errors, isinstance(proxy, FrameLocalsProxy)
"""
    with transformed_module(tmp_path, "frame_locals_proxy_constructor", source) as module:
        errors, ok = module.run()
        assert set(errors) == {"no-args", "wrong-type", "keyword"}
        assert ok is True


def test_frame_locals_delete_semantics(tmp_path):
    source = """
import sys

def run():
    x = 1
    d = sys._getframe().f_locals
    errors = []
    try:
        del d['x']
    except ValueError:
        errors.append('local')
    d['m'] = 1
    del d['m']
    return errors, 'm' in d
"""
    with transformed_module(tmp_path, "frame_locals_delete_semantics", source) as module:
        errors, has_m = module.run()
        assert errors == ['local']
        assert has_m is False


def test_frame_locals_listcomp_frame(tmp_path):
    source = """
import sys

def run():
    outer = 1
    d = [sys._getframe().f_locals for b in [0]][0]
    return d['outer']
"""
    with transformed_module(tmp_path, "frame_locals_listcomp_frame", source) as module:
        assert module.run() == 1


def test_locals_in_listcomp(tmp_path):
    source = """
def run():
    k = 1
    lst = [locals() for k in [0]]
    return lst[0]['k']
"""
    with transformed_module(tmp_path, "frame_locals_listcomp_locals", source) as module:
        assert module.run() == 0


def test_frame_locals_class_scope(tmp_path):
    source = """
import sys

class A:
    x = 1
    sys._getframe().f_locals['x'] = 2
    sys._getframe().f_locals['y'] = 3
"""
    with transformed_module(tmp_path, "frame_locals_class_scope", source) as module:
        assert module.A.x == 2
        assert module.A.y == 3


def test_frame_locals_repr_recursive(tmp_path):
    source = """
import sys

def run():
    d = sys._getframe().f_locals
    d['self'] = d
    return repr(d)
"""
    with transformed_module(tmp_path, "frame_locals_repr_recursive", source) as module:
        assert "{...}" in module.run()

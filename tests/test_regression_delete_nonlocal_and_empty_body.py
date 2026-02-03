from tests._integration import transformed_module


def test_nonlocal_delete_compiles(tmp_path):
    source = """
def outer():
    x = 1
    def inner():
        nonlocal x
        del x
        return "ok"
    inner()
    return "done"

RESULT = outer()
"""
    with transformed_module(tmp_path, "delete_nonlocal_compiles", source) as module:
        assert module.RESULT == "done"


def test_global_only_body_compiles(tmp_path):
    source = """
def f():
    global g

RESULT = f()
"""
    with transformed_module(tmp_path, "global_only_body_compiles", source) as module:
        assert module.RESULT is None


def test_annotation_only_body_compiles(tmp_path):
    source = """
def f():
    a: int

RESULT = f()
"""
    with transformed_module(tmp_path, "annotation_only_body_compiles", source) as module:
        assert module.RESULT is None


def test_except_star_global_binding(tmp_path):
    source = """
def run():
    global caught
    ok = False
    try:
        raise ExceptionGroup("eg", [ValueError("boom")])
    except* ValueError as caught:
        value = caught
        ok = isinstance(value, ExceptionGroup)
    return ok

RESULT = run()
CLEARED = "caught" not in globals()
"""
    with transformed_module(tmp_path, "except_star_global_binding", source) as module:
        assert module.RESULT is True
        assert module.CLEARED is True

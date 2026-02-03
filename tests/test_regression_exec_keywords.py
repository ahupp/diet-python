from tests._integration import transformed_module


def test_exec_accepts_globals_keyword(tmp_path):
    source = """

def run():
    ns = {}
    exec("x = 1", globals=ns)
    return ns["x"]
"""
    with transformed_module(tmp_path, "exec_globals_kw", source) as module:
        assert module.run() == 1


def test_exec_accepts_locals_keyword(tmp_path):
    source = """

def run():
    ns = {}
    exec("global x\\nx = 1", locals=ns)
    return ns
"""
    with transformed_module(tmp_path, "exec_locals_kw", source) as module:
        assert module.run() == {}


def test_exec_accepts_closure_keyword(tmp_path):
    source = """

def run():
    out = {"value": 0}
    def make():
        a = 2
        def inner():
            out["value"] = a
        return inner
    inner = make()
    exec(inner.__code__, inner.__globals__, closure=inner.__closure__)
    return out["value"]
"""
    with transformed_module(tmp_path, "exec_closure_kw", source) as module:
        assert module.run() == 2

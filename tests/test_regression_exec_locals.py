from tests._integration import transformed_module


def test_exec_sees_locals(tmp_path):
    source = """

def run():
    x = 10
    code = compile("x + 1", "", "exec")
    exec(code)
    return True
"""
    with transformed_module(tmp_path, "exec_locals", source) as module:
        assert module.run() is True

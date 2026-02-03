from tests._integration import transformed_module


def test_exec_accepts_globals_dict(tmp_path):
    source = """
value = 0

def run():
    exec(\"value = 1\", globals())
    return value
"""
    with transformed_module(tmp_path, "exec_globals", source) as module:
        assert module.run() == 1

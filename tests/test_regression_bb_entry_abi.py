from tests._integration import transformed_module


def test_module_init_entry_accepts_empty_bb_args(tmp_path):
    source = """
x = 1
"""
    with transformed_module(tmp_path, "bb_entry_module_init_empty", source) as module:
        assert module.x == 1


def test_function_entry_missing_state_uses_deleted_sentinel(tmp_path):
    source = """
def f():
    try:
        pass
    except Exception:
        pass
    return 1

y = f()
"""
    with transformed_module(tmp_path, "bb_entry_missing_state_deleted", source) as module:
        assert module.y == 1

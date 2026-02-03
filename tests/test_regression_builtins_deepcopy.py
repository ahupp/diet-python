import builtins
import copy

from tests._integration import transformed_module


def test_deepcopy_builtins_dict_with_dp(tmp_path):
    source = """
import builtins
import copy

def run():
    ns = {"__builtins__": builtins.__dict__}
    copy.deepcopy(ns)
    return True
"""
    with transformed_module(tmp_path, "builtins_deepcopy", source) as module:
        assert module.run() is True

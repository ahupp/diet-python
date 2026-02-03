from tests._integration import transformed_module


def test_dir_filters_dp_internal_names(tmp_path):
    source = """

def run():
    junk = 1
    return dir()
"""
    with transformed_module(tmp_path, "dir_filters", source) as module:
        assert module.run() == ["junk"]

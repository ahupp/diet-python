from tests._integration import transformed_module


def test_genexpr_only_iterates_once(tmp_path):
    source = """
class Iterator:
    def __next__(self):
        raise StopIteration

class Iterable:
    def __iter__(self):
        return Iterator()

def run():
    return list(x for x in Iterable())
"""
    with transformed_module(tmp_path, "genexpr_iter_once", source) as module:
        assert module.run() == []

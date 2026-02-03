from tests._integration import transformed_module


def test_listcomp_calls_iter_once(tmp_path):
    source = """
class Iterator:
    def __init__(self):
        self.val = 0
    def __next__(self):
        if self.val == 2:
            raise StopIteration
        self.val += 1
        return self.val

class C:
    def __iter__(self):
        return Iterator()

def run():
    return [i for i in C()]
"""
    with transformed_module(tmp_path, "listcomp_iter_once", source) as module:
        assert module.run() == [1, 2]

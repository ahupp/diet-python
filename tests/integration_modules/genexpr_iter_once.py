
class Iterator:
    def __next__(self):
        raise StopIteration

class Iterable:
    def __iter__(self):
        return Iterator()

def run():
    return list(x for x in Iterable())


# diet-python: validate

def validate_module(module):
    assert module.run() == []

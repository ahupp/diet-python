
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


# diet-python: validate

def validate_module(module):
    assert module.run() == [1, 2]


def gen(flag):
    yield flag and 1

def main():
    return list(gen(True)), list(gen(False))


# diet-python: validate

def validate_module(module):
    assert module.main() == ([1], [False])

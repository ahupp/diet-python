
class _:
    def __a(self):
        return "ok"


def run():
    return "__a" in _.__dict__, "___a" in _.__dict__


# diet-python: validate

def validate_module(module):
    assert module.run() == (True, False)

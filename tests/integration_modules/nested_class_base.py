class Outer:
    class BaseThing(str):
        pass

    class Thing(BaseThing):
        pass


def get_base_name():
    return Outer.Thing.__bases__[0].__name__

# diet-python: validate

def validate(module):
    assert module.get_base_name() == "BaseThing"

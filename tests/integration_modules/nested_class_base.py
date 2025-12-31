class Outer:
    class BaseThing(str):
        pass

    class Thing(BaseThing):
        pass


def get_base_name():
    return Outer.Thing.__bases__[0].__name__

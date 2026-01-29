class Base:
    def __init__(self):
        self.value = "base"


class Derived(Base):
    def __init__(self):
        super().__init__()
        self.child = True


Alias = Derived
Derived = dict

INSTANCE = Alias()
VALUE = INSTANCE.value

# diet-python: validate

module = __import__("sys").modules[__name__]
assert module.VALUE == "base"
assert module.INSTANCE.child is True

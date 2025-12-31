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

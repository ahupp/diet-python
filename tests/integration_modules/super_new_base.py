from collections import namedtuple


class Base(namedtuple("Base", "a b")):
    def __new__(cls, value):
        return super().__new__(cls, value, value)


class Child(Base):
    pass


def build_child():
    return Child(1)

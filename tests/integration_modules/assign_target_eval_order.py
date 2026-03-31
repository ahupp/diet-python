events = []


class Box:
    def __init__(self, label):
        self.label = label
        self.values = {}

    def __setitem__(self, key, value):
        events.append(("set", self.label, key, value))
        self.values[key] = value


class AttrBox:
    def __setattr__(self, name, value):
        if name == "value":
            events.append(("setattr", name, value))
        object.__setattr__(self, name, value)


def key():
    events.append("key")
    return 0


def rhs():
    events.append("rhs")
    return 2


def make_box():
    events.append("box")
    return Box("tmp")


def make_attr_box():
    events.append("attr_box")
    return AttrBox()


def run_named_subscript():
    events.clear()
    box = Box("named")
    box[key()] = rhs()
    return list(events)


def run_nested_subscript():
    events.clear()
    make_box()[key()] = rhs()
    return list(events)


def run_attr():
    events.clear()
    make_attr_box().value = rhs()
    return list(events)


# diet-python: validate

def validate_module(module):
    assert module.run_named_subscript() == [
        "rhs",
        "key",
        ("set", "named", 0, 2),
    ]
    assert module.run_nested_subscript() == [
        "rhs",
        "box",
        "key",
        ("set", "tmp", 0, 2),
    ]
    assert module.run_attr() == [
        "rhs",
        "attr_box",
        ("setattr", "value", 2),
    ]

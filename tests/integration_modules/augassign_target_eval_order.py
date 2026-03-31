events = []


class Box:
    def __init__(self, label):
        self.label = label
        self.values = {0: 1}

    def __getitem__(self, key):
        events.append(("get", self.label, key))
        return self.values[key]

    def __setitem__(self, key, value):
        events.append(("set", self.label, key, value))
        self.values[key] = value


class AttrBox:
    def __init__(self):
        object.__setattr__(self, "value", 1)

    def __getattribute__(self, name):
        if name == "value":
            events.append(("getattr", name))
        return object.__getattribute__(self, name)

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
    box[key()] += rhs()
    return list(events)


def run_nested_subscript():
    events.clear()
    make_box()[key()] += rhs()
    return list(events)


def run_attr():
    events.clear()
    make_attr_box().value += rhs()
    return list(events)


# diet-python: validate

def validate_module(module):
    assert module.run_named_subscript() == [
        "key",
        ("get", "named", 0),
        "rhs",
        ("set", "named", 0, 3),
    ]
    assert module.run_nested_subscript() == [
        "box",
        "key",
        ("get", "tmp", 0),
        "rhs",
        ("set", "tmp", 0, 3),
    ]
    assert module.run_attr() == [
        "attr_box",
        ("getattr", "value"),
        "rhs",
        ("setattr", "value", 3),
    ]

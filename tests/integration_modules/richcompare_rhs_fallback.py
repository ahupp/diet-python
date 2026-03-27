

class Left:
    def __eq__(self, other):
        return NotImplemented

    def __ne__(self, other):
        return NotImplemented


class Right:
    def __eq__(self, other):
        return "RIGHT_EQ"

    def __ne__(self, other):
        return "RIGHT_NE"


def run():
    lhs = Left()
    rhs = Right()
    return lhs == rhs, lhs != rhs


# diet-python: validate

def validate_module(module):
    assert module.run() == ("RIGHT_EQ", "RIGHT_NE")

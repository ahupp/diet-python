from tests._integration import transformed_module


def test_richcompare_uses_matching_rhs_method(tmp_path):
    source = """

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
"""

    with transformed_module(tmp_path, "richcompare_rhs_fallback", source) as module:
        assert module.run() == ("RIGHT_EQ", "RIGHT_NE")

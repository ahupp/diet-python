results = {}

z1 = "outer"


class InnerSeesOuterScope:
    z1 = "inner"

    class Inner:
        y = z1


result = InnerSeesOuterScope.Inner.y

# diet-python: validate

def validate_module(module):


    assert module.result == "outer"

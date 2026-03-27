results = {}

x = "global"


class C1:
    x = "class"

    def read():
        return x


result = (C1.x, C1.read(), x)

# diet-python: validate

def validate_module(module):


    assert module.result == ("class", "global", "global")

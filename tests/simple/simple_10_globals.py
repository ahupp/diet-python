X = 7
Y = "hello"
Z = [1, "foo", (8,)]

# diet-python: validate

def validate(module):
    assert module.X == 7
    assert module.Y == "hello"
    assert module.Z == [1, "foo", (8,)]

X = 7
Y = "hello"
Z = [1, "foo", (8,)]

# diet-python: validate

module = __import__("sys").modules[__name__]
assert module.X == 7
assert module.Y == "hello"
assert module.Z == [1, "foo", (8,)]

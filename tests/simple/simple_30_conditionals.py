X = 5
Y = 8
if X < Y:
    Z = 1
else:
    Z = 0

# diet-python: validate

module = __import__("sys").modules[__name__]
assert module.X == 5
assert module.Y == 8
assert module.Z == 1

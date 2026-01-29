T = 3
S = T + 1

# diet-python: validate

module = __import__("sys").modules[__name__]
assert module.T == 3
assert module.S == 4

def walk_until_truthy(values):
    idx = 0
    seen = []
    while not (value := values[idx]):
        seen.append(idx)
        idx += 1
        if idx > 3:
            break
    return seen, idx, value

# diet-python: validate

module = __import__("sys").modules[__name__]
seen, idx, value = module.walk_until_truthy([0, 1])
assert seen == [0]
assert idx == 1
assert value == 1

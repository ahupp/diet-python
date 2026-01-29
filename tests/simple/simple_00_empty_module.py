# diet-python: validate

module = __import__("sys").modules[__name__]
assert "X" not in module.__dict__
assert "Y" not in module.__dict__
assert "Z" not in module.__dict__

X = 5
Y = 8
if X < Y:
    Z = 1
else:
    Z = 0

# diet-python: validate

def validate(module):
    assert module.X == 5
    assert module.Y == 8
    assert module.Z == 1

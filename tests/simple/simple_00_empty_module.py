

# diet-python: validate

def validate_module(module):
    assert "X" not in module.__dict__
    assert "Y" not in module.__dict__
    assert "Z" not in module.__dict__

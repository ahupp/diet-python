def gen(value):
    return (yield value)

# diet-python: validate

def validate_module(module):

    import pytest

    g = module.gen("start")
    assert next(g) == "start"
    with pytest.raises(StopIteration) as exc:
        g.send("done")
    assert exc.value.value == "done"

    g2 = module.gen("x")
    assert next(g2) == "x"
    with pytest.raises(ValueError):
        g2.throw(ValueError("boom"))

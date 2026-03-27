func = lambda: "global"

def build():
    funcs = [lambda: "local"]
    out = []
    for i, func in enumerate(funcs):
        class S:
            value = func()
        out.append((i, S.value))
    return out

# diet-python: validate

def validate_module(module):
    func = lambda: "global"

    result = module.build()
    assert result == [(0, "local")]

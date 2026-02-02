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

func = lambda: "global"

result = build()
assert result == [(0, "local")]

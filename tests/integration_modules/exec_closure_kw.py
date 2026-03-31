def run():
    out = {"value": 0}

    def make():
        a = 2

        def inner():
            out["value"] = a

        return inner

    inner = make()
    exec(inner.__code__, inner.__globals__, closure=inner.__closure__)
    return out["value"]


# diet-python: validate

def validate_module(module):
    assert module.run() == 2

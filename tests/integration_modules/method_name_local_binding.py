class Example:
    def close(self):
        close = lambda: "ok"
        if close:
            return close()
        return "no"

# diet-python: validate

def validate_module(module):

    instance = module.Example()
    assert instance.close() == "ok"

class C4:
    def outer():
        x = "outer"

        def inner():
            nonlocal x
            x = "inner"

        inner()
        return x


result = C4.outer()

# diet-python: validate

def validate_module(module):


    assert module.result == "inner"

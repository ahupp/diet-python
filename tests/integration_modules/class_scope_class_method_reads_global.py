results = {}

x = "module"


def outer_with_class_method_reads_global():
    x = "outer"

    class Inner:
        x = "class"

        def read():
            return x

    return (Inner.x, Inner.read(), x)


result = outer_with_class_method_reads_global()

# diet-python: validate

def validate_module(module):


    assert module.result == ("class", "outer", "outer")

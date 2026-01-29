class Example:
    atexit = "class"

    def __init__(self):
        import atexit
        self.module_name = atexit.__name__


VALUE = Example().module_name
CLASS_ATTR = Example.atexit

# diet-python: validate

module = __import__("sys").modules[__name__]
assert module.VALUE == "atexit"
assert module.CLASS_ATTR == "class"

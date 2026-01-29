import time


class Outer:
    def format_help(self):
        return "outer"

    class Inner:
        def format_help(self):
            return time.get_clock_info("monotonic").resolution


VALUE = Outer.Inner().format_help()

# diet-python: validate

module = __import__("sys").modules[__name__]
assert isinstance(module.VALUE, float)

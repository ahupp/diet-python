import time


class Base:
    def __init__(self):
        self.resolution = time.get_clock_info("monotonic").resolution

    def time(self):
        return time.monotonic()


VALUE = Base().resolution

# diet-python: validate

module = __import__("sys").modules[__name__]
assert isinstance(module.VALUE, float)

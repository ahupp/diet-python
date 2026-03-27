import time


class Base:
    def __init__(self):
        self.resolution = time.get_clock_info("monotonic").resolution

    def time(self):
        return time.monotonic()


VALUE = Base().resolution

# diet-python: validate

def validate_module(module):
    assert isinstance(module.VALUE, float)

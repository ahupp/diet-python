def call_original(func):
    def inner(*args, **kwargs):
        return func(*args, **kwargs)
    return inner


class Example:
    @call_original
    def __getitem__(self, item):
        return item

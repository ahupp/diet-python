class Example:
    def __init__(self, value):
        self.__value = value

    def update(self, value):
        self.__value = value

    def read(self):
        return self.__value


def use_example():
    instance = Example("initial")
    instance.update("payload")
    return instance.read()

class Example:
    def set_value(self):
        self.__value = "ok"

    def get_value(self):
        return self.__value


def run():
    instance = Example()
    instance.set_value()
    return instance.get_value()


# diet-python: validate

def validate_module(module):
    assert module.run() == "ok"

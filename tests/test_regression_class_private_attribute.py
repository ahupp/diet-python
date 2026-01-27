from tests._integration import transformed_module


def test_class_private_attribute_mangling(tmp_path):
    source = """
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
"""
    with transformed_module(tmp_path, "class_private_attribute", source) as module:
        assert module.use_example() == "payload"


def test_class_private_attribute_setattr(tmp_path):
    source = """
class Example:
    def set_value(self):
        self.__value = "ok"

    def get_value(self):
        return self.__value


def run():
    instance = Example()
    instance.set_value()
    return instance.get_value()
"""
    with transformed_module(tmp_path, "class_private_attribute_set", source) as module:
        assert module.run() == "ok"

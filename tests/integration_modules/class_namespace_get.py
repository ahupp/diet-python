import types


class Example:
    def get(self):
        return "value"

    __class_getitem__ = classmethod(types.GenericAlias)


RESULT = Example[int]

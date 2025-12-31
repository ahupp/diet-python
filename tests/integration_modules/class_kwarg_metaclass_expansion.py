class Meta(int):
    def __init__(*args, **kwargs):
        pass

    def __new__(cls, name, bases, attrs, **kwargs):
        return bases, kwargs


d = {"metaclass": Meta}


class A(**d):
    pass


RESULT = A

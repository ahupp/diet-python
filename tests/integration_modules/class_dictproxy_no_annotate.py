
class C:
    def meth(self):
        pass

def run():
    return "__annotate__" in C.__dict__


# diet-python: validate

def validate_module(module):
    assert module.run() is False

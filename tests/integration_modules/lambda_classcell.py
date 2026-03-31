def classcell_lambda():
    class C:
        f = lambda: __class__

    return C.f(), C


# diet-python: validate

def validate_module(module):
    cls_from_lambda, cls = module.classcell_lambda()
    assert cls_from_lambda is cls

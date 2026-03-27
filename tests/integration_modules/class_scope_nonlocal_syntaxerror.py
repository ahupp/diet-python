
def nonlocal_in_class_body_error():
    try:
        exec("class Bad:\n    nonlocal x\n", globals())
    except SyntaxError as exc:
        return exc.msg
    return None


result = nonlocal_in_class_body_error()

# diet-python: validate

def validate_module(module):


    assert module.result is not None
    assert module.result

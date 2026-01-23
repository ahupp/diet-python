import sys

sys.set_coroutine_origin_tracking_depth(1)


async def corofn():
    return 1


def a1():
    return corofn()  # comment in a1


def get_origin():
    coro = a1()
    origin = coro.cr_origin
    coro.close()
    return origin


RESULT = get_origin()

# diet-python: validate

import diet_import_hook

def validate(module):
    origin = module.RESULT
    assert origin is not None
    ((filename, lineno, funcname),) = origin
    assert funcname == "a1"

    transformed = diet_import_hook._transform_source(module.__file__)
    target_line = None
    for idx, line in enumerate(transformed.splitlines(), 1):
        if "return corofn$0()" in line:
            target_line = idx
            break
        assert target_line is not None
        assert lineno == target_line

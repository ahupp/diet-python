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

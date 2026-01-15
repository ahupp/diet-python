from contextlib import nullcontext


def unpack_starred_list():
    with nullcontext(range(1, 5)) as (a, *b, c):
        return a, b, c

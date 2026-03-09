# import_simple

import a

# ==


# import_dotted_alias

import a.b as c

# ==


# import_from_alias

from pkg.mod import name as alias

# ==


# decorator_function


@dec
def f():
    pass


# ==


# assign_attr

obj.x = 1

# ==


# assign_subscript

obj[i] = v

# ==


# assign_tuple_unpack

a, b = it

# ==


# assign_star_unpack

a, *b = it

# ==


# assign_multi_targets

a = b = f()

# ==


# ann_assign_simple

x: int = 1

# ==


# ann_assign_attr

obj.x: int = 1

# ==


# aug_assign_attr

obj.x += 1

# ==


# delete_mixed

del obj.x, obj[i], x

# ==


# assert_no_msg

assert cond

# ==


# assert_with_msg

assert cond, "oops"

# ==


# raise_from

raise E from cause

# ==


# try_except_typed

try:
    f()
except E as e:
    g(e)
except:
    h()

# ==


# for_else

for x in it:
    body()
else:
    done()

# ==


# while_else

while cond:
    body()
else:
    done()

# ==


# with_as

with cm as x:
    body()

# ==


# function_local_ann_assign


def inner():
    value: int = 1
    return value


# ==


# comprehension_global

xs = [x for x in it]
ys = {x for x in it}
zs = {k: v for k, v in items}

# ==


# comprehension_in_function


def f():
    return [x for x in it if x > 0]


# ==


# comprehension_in_class_body


class C:
    xs = [x for x in it]


# ==


# with_multi

with a as x, b as y:
    body()

# ==


# async_for


async def run():
    async for x in ait:
        body()


# ==


# async_with


async def run():
    async with cm as x:
        body()


# ==


# match_simple

match value:
    case 1:
        one()
    case _:
        other()

# ==


# generator_yield


def gen():
    yield 1


# ==


# yield_from


def gen():
    yield from it


# ==


# with_exit_suppresses_exception

with Suppress():
    raise RuntimeError("boom")

# ==


# closure_cell_simple


def outer():
    x = 5

    def inner():
        return x

    return inner()


# ==


# bb_if_else_function


def choose(a, b):
    total = a + b
    if total > 5:
        return a
    else:
        return b


# ==


# closure_cell_nonlocal


def outer():
    x = 5

    def inner():
        nonlocal x
        x = 2
        return x

    return inner()


# ==


# plain try / catch

try:
    print(1)
except Exception:
    print(2)

# ==


# complicated generator


def complicated(a):
    for i in a:
        try:
            j = i + 1
            yield j
        except Exception:
            print("oops")
    else:
        print("finsihed")


# ==


# complicated generator


def complicated(a):
    for i in a:
        try:
            j = i + 1
            yield j
        except Exception:
            print("oops")


# ==

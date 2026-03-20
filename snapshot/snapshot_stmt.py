# import_simple

import a

# ==

# snapshot regeneration failed
# panic: ast-to-ast pass should be tracked

# import_dotted_alias

import a.b as c

# ==

# snapshot regeneration failed
# panic: ast-to-ast pass should be tracked

# import_from_alias

from pkg.mod import name as alias

# ==

# snapshot regeneration failed
# panic: ast-to-ast pass should be tracked

# decorator_function


@dec
def f():
    pass


# ==

# snapshot regeneration failed
# panic: ast-to-ast pass should be tracked

# assign_attr

obj.x = 1

# ==

# snapshot regeneration failed
# panic: ast-to-ast pass should be tracked

# assign_subscript

obj[i] = v

# ==

# snapshot regeneration failed
# panic: ast-to-ast pass should be tracked

# assign_tuple_unpack

a, b = it

# ==

# snapshot regeneration failed
# panic: ast-to-ast pass should be tracked

# assign_star_unpack

a, *b = it

# ==

# snapshot regeneration failed
# panic: ast-to-ast pass should be tracked

# assign_multi_targets

a = b = f()

# ==

# snapshot regeneration failed
# panic: ast-to-ast pass should be tracked

# ann_assign_simple

x: int = 1

# ==

# snapshot regeneration failed
# panic: ast-to-ast pass should be tracked

# ann_assign_attr

obj.x: int = 1

# ==

# snapshot regeneration failed
# panic: ast-to-ast pass should be tracked

# aug_assign_attr

obj.x += 1

# ==

# snapshot regeneration failed
# panic: ast-to-ast pass should be tracked

# delete_mixed

del obj.x, obj[i], x

# ==

# snapshot regeneration failed
# panic: ast-to-ast pass should be tracked

# assert_no_msg

assert cond

# ==

# snapshot regeneration failed
# panic: ast-to-ast pass should be tracked

# assert_with_msg

assert cond, "oops"

# ==

# snapshot regeneration failed
# panic: ast-to-ast pass should be tracked

# raise_from

raise E from cause

# ==

# snapshot regeneration failed
# panic: ast-to-ast pass should be tracked

# try_except_typed

try:
    f()
except E as e:
    g(e)
except:
    h()

# ==

# snapshot regeneration failed
# panic: py_stmt template must produce exactly one statement, got 2

# for_else

for x in it:
    body()
else:
    done()

# ==

# snapshot regeneration failed
# panic: ast-to-ast pass should be tracked

# while_else

while cond:
    body()
else:
    done()

# ==

# snapshot regeneration failed
# panic: ast-to-ast pass should be tracked

# with_as

with cm as x:
    body()

# ==

# snapshot regeneration failed
# panic: ast-to-ast pass should be tracked

# function_local_ann_assign


def inner():
    value: int = 1
    return value


# ==

# snapshot regeneration failed
# panic: ast-to-ast pass should be tracked

# comprehension_global

xs = [x for x in it]
ys = {x for x in it}
zs = {k: v for k, v in items}

# ==

# snapshot regeneration failed
# panic: ast-to-ast pass should be tracked

# comprehension_in_function


def f():
    return [x for x in it if x > 0]


# ==

# snapshot regeneration failed
# panic: ast-to-ast pass should be tracked

# comprehension_in_class_body


class C:
    xs = [x for x in it]


# ==

# snapshot regeneration failed
# panic: ast-to-ast pass should be tracked

# with_multi

with a as x, b as y:
    body()

# ==

# snapshot regeneration failed
# panic: ast-to-ast pass should be tracked

# async_for


async def run():
    async for x in ait:
        body()


# ==

# snapshot regeneration failed
# panic: core BlockPy yield lowering is not explicit yet: yield-family expr reached the core no-yield boundary for run

# async_with


async def run():
    async with cm as x:
        body()


# ==

# snapshot regeneration failed
# panic: core BlockPy yield lowering is not explicit yet: yield-family expr reached the core no-yield boundary for run

# match_simple

match value:
    case 1:
        one()
    case _:
        other()

# ==

# snapshot regeneration failed
# panic: ast-to-ast pass should be tracked

# generator_yield


def gen():
    yield 1


# ==

# snapshot regeneration failed
# panic: core BlockPy yield lowering is not explicit yet: yield-family expr reached the core no-yield boundary for gen

# yield_from


def gen():
    yield from it


# ==

# snapshot regeneration failed
# panic: core BlockPy yield lowering is not explicit yet: yield-family expr reached the core no-yield boundary for gen

# with_exit_suppresses_exception

with Suppress():
    raise RuntimeError("boom")

# ==

# snapshot regeneration failed
# panic: ast-to-ast pass should be tracked

# closure_cell_simple


def outer():
    x = 5

    def inner():
        return x

    return inner()


# ==

# snapshot regeneration failed
# panic: ast-to-ast pass should be tracked

# bb_if_else_function


def choose(a, b):
    total = a + b
    if total > 5:
        return a
    else:
        return b


# ==

# snapshot regeneration failed
# panic: ast-to-ast pass should be tracked

# closure_cell_nonlocal


def outer():
    x = 5

    def inner():
        nonlocal x
        x = 2
        return x

    return inner()


# ==

# snapshot regeneration failed
# panic: ast-to-ast pass should be tracked

# plain try / catch

try:
    print(1)
except Exception:
    print(2)

# ==

# snapshot regeneration failed
# panic: ast-to-ast pass should be tracked

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

# snapshot regeneration failed
# panic: core BlockPy yield lowering is not explicit yet: yield-family expr reached the core no-yield boundary for complicated

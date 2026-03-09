# import_simple

import a

# ==

__dp_store_global(
    globals(),
    __dp_decode_literal_bytes(b"_dp_module_init"),
    __dp_def_fn(
        __dp_decode_literal_bytes(b"_dp_bb__dp_module_init_start"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_tuple(),
        __dp_tuple(),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)

# import_dotted_alias

import a.b as c

# ==

__dp_store_global(
    globals(),
    __dp_decode_literal_bytes(b"_dp_module_init"),
    __dp_def_fn(
        __dp_decode_literal_bytes(b"_dp_bb__dp_module_init_start"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_tuple(),
        __dp_tuple(),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)

# import_from_alias

from pkg.mod import name as alias

# ==

__dp_store_global(
    globals(),
    __dp_decode_literal_bytes(b"_dp_module_init"),
    __dp_def_fn(
        __dp_decode_literal_bytes(b"_dp_bb__dp_module_init_start"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_tuple(),
        __dp_tuple(),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)

# decorator_function


@dec
def f():
    pass


# ==

__dp_store_global(
    globals(),
    __dp_decode_literal_bytes(b"_dp_module_init"),
    __dp_def_fn(
        __dp_decode_literal_bytes(b"_dp_bb__dp_module_init_start"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_tuple(),
        __dp_tuple(),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)

# assign_attr

obj.x = 1

# ==

__dp_store_global(
    globals(),
    __dp_decode_literal_bytes(b"_dp_module_init"),
    __dp_def_fn(
        __dp_decode_literal_bytes(b"_dp_bb__dp_module_init_start"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_tuple(),
        __dp_tuple(),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)

# assign_subscript

obj[i] = v

# ==

__dp_store_global(
    globals(),
    __dp_decode_literal_bytes(b"_dp_module_init"),
    __dp_def_fn(
        __dp_decode_literal_bytes(b"_dp_bb__dp_module_init_start"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_tuple(),
        __dp_tuple(),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)

# assign_tuple_unpack

a, b = it

# ==

__dp_store_global(
    globals(),
    __dp_decode_literal_bytes(b"_dp_module_init"),
    __dp_def_fn(
        __dp_decode_literal_bytes(b"_dp_bb__dp_module_init_start"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_tuple(),
        __dp_tuple(),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)

# assign_star_unpack

a, *b = it

# ==

__dp_store_global(
    globals(),
    __dp_decode_literal_bytes(b"_dp_module_init"),
    __dp_def_fn(
        __dp_decode_literal_bytes(b"_dp_bb__dp_module_init_start"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_tuple(),
        __dp_tuple(),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)

# assign_multi_targets

a = b = f()

# ==

__dp_store_global(
    globals(),
    __dp_decode_literal_bytes(b"_dp_module_init"),
    __dp_def_fn(
        __dp_decode_literal_bytes(b"_dp_bb__dp_module_init_start"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_tuple(),
        __dp_tuple(),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)

# ann_assign_simple

x: int = 1

# ==

__dp_store_global(
    globals(),
    __dp_decode_literal_bytes(b"_dp_module_init"),
    __dp_def_fn(
        __dp_decode_literal_bytes(b"_dp_bb__dp_module_init_start"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_tuple(),
        __dp_tuple(),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)

# ann_assign_attr

obj.x: int = 1

# ==

__dp_store_global(
    globals(),
    __dp_decode_literal_bytes(b"_dp_module_init"),
    __dp_def_fn(
        __dp_decode_literal_bytes(b"_dp_bb__dp_module_init_start"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_tuple(),
        __dp_tuple(),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)

# aug_assign_attr

obj.x += 1

# ==

__dp_store_global(
    globals(),
    __dp_decode_literal_bytes(b"_dp_module_init"),
    __dp_def_fn(
        __dp_decode_literal_bytes(b"_dp_bb__dp_module_init_start"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_tuple(),
        __dp_tuple(),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)

# delete_mixed

del obj.x, obj[i], x

# ==

__dp_store_global(
    globals(),
    __dp_decode_literal_bytes(b"_dp_module_init"),
    __dp_def_fn(
        __dp_decode_literal_bytes(b"_dp_bb__dp_module_init_start"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_tuple(),
        __dp_tuple(),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)

# assert_no_msg

assert cond

# ==

__dp_store_global(
    globals(),
    __dp_decode_literal_bytes(b"_dp_module_init"),
    __dp_def_fn(
        __dp_decode_literal_bytes(b"_dp_bb__dp_module_init_start"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_tuple(),
        __dp_tuple(),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)

# assert_with_msg

assert cond, "oops"

# ==

__dp_store_global(
    globals(),
    __dp_decode_literal_bytes(b"_dp_module_init"),
    __dp_def_fn(
        __dp_decode_literal_bytes(b"_dp_bb__dp_module_init_start"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_tuple(),
        __dp_tuple(),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)

# raise_from

raise E from cause

# ==

__dp_store_global(
    globals(),
    __dp_decode_literal_bytes(b"_dp_module_init"),
    __dp_def_fn(
        __dp_decode_literal_bytes(b"_dp_bb__dp_module_init_start"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_tuple(),
        __dp_tuple(),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)

# try_except_typed

try:
    f()
except E as e:
    g(e)
except:
    h()

# ==

__dp_store_global(
    globals(),
    __dp_decode_literal_bytes(b"_dp_module_init"),
    __dp_def_fn(
        __dp_decode_literal_bytes(b"_dp_bb__dp_module_init_start"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_tuple(),
        __dp_tuple(),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)

# for_else

for x in it:
    body()
else:
    done()

# ==

__dp_store_global(
    globals(),
    __dp_decode_literal_bytes(b"_dp_module_init"),
    __dp_def_fn(
        __dp_decode_literal_bytes(b"_dp_bb__dp_module_init_start"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_tuple(),
        __dp_tuple(),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)

# while_else

while cond:
    body()
else:
    done()

# ==

__dp_store_global(
    globals(),
    __dp_decode_literal_bytes(b"_dp_module_init"),
    __dp_def_fn(
        __dp_decode_literal_bytes(b"_dp_bb__dp_module_init_start"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_tuple(),
        __dp_tuple(),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)

# with_as

with cm as x:
    body()

# ==

__dp_store_global(
    globals(),
    __dp_decode_literal_bytes(b"_dp_module_init"),
    __dp_def_fn(
        __dp_decode_literal_bytes(b"_dp_bb__dp_module_init_start"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_tuple(),
        __dp_tuple(),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)

# function_local_ann_assign


def inner():
    value: int = 1
    return value


# ==

__dp_store_global(
    globals(),
    __dp_decode_literal_bytes(b"_dp_module_init"),
    __dp_def_fn(
        __dp_decode_literal_bytes(b"_dp_bb__dp_module_init_start"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_tuple(),
        __dp_tuple(),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)

# comprehension_global

xs = [x for x in it]
ys = {x for x in it}
zs = {k: v for k, v in items}

# ==

__dp_store_global(
    globals(),
    __dp_decode_literal_bytes(b"_dp_module_init"),
    __dp_def_fn(
        __dp_decode_literal_bytes(b"_dp_bb__dp_module_init_start"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_tuple(),
        __dp_tuple(),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)

# comprehension_in_function


def f():
    return [x for x in it if x > 0]


# ==

__dp_store_global(
    globals(),
    __dp_decode_literal_bytes(b"_dp_module_init"),
    __dp_def_fn(
        __dp_decode_literal_bytes(b"_dp_bb__dp_module_init_start"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_tuple(),
        __dp_tuple(),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)

# comprehension_in_class_body


class C:
    xs = [x for x in it]


# ==

__dp_store_global(
    globals(),
    __dp_decode_literal_bytes(b"_dp_module_init"),
    __dp_def_fn(
        __dp_decode_literal_bytes(b"_dp_bb__dp_module_init_start"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_tuple(),
        __dp_tuple(),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)

# with_multi

with a as x, b as y:
    body()

# ==

__dp_store_global(
    globals(),
    __dp_decode_literal_bytes(b"_dp_module_init"),
    __dp_def_fn(
        __dp_decode_literal_bytes(b"_dp_bb__dp_module_init_start"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_tuple(),
        __dp_tuple(),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)

# async_for


async def run():
    async for x in ait:
        body()


# ==

__dp_store_global(
    globals(),
    __dp_decode_literal_bytes(b"_dp_module_init"),
    __dp_def_fn(
        __dp_decode_literal_bytes(b"_dp_bb__dp_module_init_start"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_tuple(),
        __dp_tuple(),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)

# async_with


async def run():
    async with cm as x:
        body()


# ==

__dp_store_global(
    globals(),
    __dp_decode_literal_bytes(b"_dp_module_init"),
    __dp_def_fn(
        __dp_decode_literal_bytes(b"_dp_bb__dp_module_init_start"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_tuple(),
        __dp_tuple(),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)

# match_simple

match value:
    case 1:
        one()
    case _:
        other()

# ==

__dp_store_global(
    globals(),
    __dp_decode_literal_bytes(b"_dp_module_init"),
    __dp_def_fn(
        __dp_decode_literal_bytes(b"_dp_bb__dp_module_init_start"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_tuple(),
        __dp_tuple(),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)

# generator_yield


def gen():
    yield 1


# ==

__dp_store_global(
    globals(),
    __dp_decode_literal_bytes(b"_dp_module_init"),
    __dp_def_fn(
        __dp_decode_literal_bytes(b"_dp_bb__dp_module_init_start"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_tuple(),
        __dp_tuple(),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)

# yield_from


def gen():
    yield from it


# ==

__dp_store_global(
    globals(),
    __dp_decode_literal_bytes(b"_dp_module_init"),
    __dp_def_fn(
        __dp_decode_literal_bytes(b"_dp_bb__dp_module_init_start"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_tuple(),
        __dp_tuple(),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)

# with_exit_suppresses_exception

with Suppress():
    raise RuntimeError("boom")

# ==

__dp_store_global(
    globals(),
    __dp_decode_literal_bytes(b"_dp_module_init"),
    __dp_def_fn(
        __dp_decode_literal_bytes(b"_dp_bb__dp_module_init_start"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_tuple(),
        __dp_tuple(),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)

# closure_cell_simple


def outer():
    x = 5

    def inner():
        return x

    return inner()


# ==

__dp_store_global(
    globals(),
    __dp_decode_literal_bytes(b"_dp_module_init"),
    __dp_def_fn(
        __dp_decode_literal_bytes(b"_dp_bb__dp_module_init_start"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_tuple(),
        __dp_tuple(),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)

# bb_if_else_function


def choose(a, b):
    total = a + b
    if total > 5:
        return a
    else:
        return b


# ==

__dp_store_global(
    globals(),
    __dp_decode_literal_bytes(b"_dp_module_init"),
    __dp_def_fn(
        __dp_decode_literal_bytes(b"_dp_bb__dp_module_init_start"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_tuple(),
        __dp_tuple(),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)

# closure_cell_nonlocal


def outer():
    x = 5

    def inner():
        nonlocal x
        x = 2
        return x

    return inner()


# ==

__dp_store_global(
    globals(),
    __dp_decode_literal_bytes(b"_dp_module_init"),
    __dp_def_fn(
        __dp_decode_literal_bytes(b"_dp_bb__dp_module_init_start"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_tuple(),
        __dp_tuple(),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)

# plain try / catch

try:
    print(1)
except Exception:
    print(2)

# ==

__dp_store_global(
    globals(),
    __dp_decode_literal_bytes(b"_dp_module_init"),
    __dp_def_fn(
        __dp_decode_literal_bytes(b"_dp_bb__dp_module_init_start"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_tuple(),
        __dp_tuple(),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)

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

__dp_store_global(
    globals(),
    __dp_decode_literal_bytes(b"_dp_module_init"),
    __dp_def_fn(
        __dp_decode_literal_bytes(b"_dp_bb__dp_module_init_start"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_tuple(),
        __dp_tuple(),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)

# complicated generator


def complicated(a):
    for i in a:
        try:
            j = i + 1
            yield j
        except Exception:
            print("oops")


# ==

__dp_store_global(
    globals(),
    __dp_decode_literal_bytes(b"_dp_module_init"),
    __dp_def_fn(
        __dp_decode_literal_bytes(b"_dp_bb__dp_module_init_start"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_tuple(),
        __dp_tuple(),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)

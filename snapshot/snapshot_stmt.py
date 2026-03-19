# import_simple

import a

# ==

# module_init: _dp_module_init

# function _dp_module_init():
#     block start:
#         __dp_store_global(globals(), "a", __dp_import_("a", __spec__))
#         return

# import_dotted_alias

import a.b as c

# ==

# module_init: _dp_module_init

# function _dp_module_init():
#     block start:
#         __dp_store_global(globals(), "c", __dp_import_attr(__dp_import_("a.b", __spec__), "b"))
#         return

# import_from_alias

from pkg.mod import name as alias

# ==

# module_init: _dp_module_init

# function _dp_module_init():
#     block start:
#         _dp_import_1 = __dp_import_("pkg.mod", __spec__, ["name"])
#         __dp_store_global(globals(), "alias", __dp_import_attr(_dp_import_1, "name"))
#         return

# decorator_function


@dec
def f():
    pass


# ==

# module_init: _dp_module_init

# function f():
#     block start:
#         return

# function _dp_module_init():
#     block start:
#         __dp_store_global(globals(), "f", dec(__dp_make_function("start", 0, "f", "f", __dp_tuple(), __dp_tuple(), __dp_tuple(), __dp_globals(), __name__, None, None)))
#         return

# assign_attr

obj.x = 1

# ==

# module_init: _dp_module_init

# function _dp_module_init():
#     block start:
#         __dp_setattr(__dp_load_deleted_name("obj", obj), "x", 1)
#         return

# assign_subscript

obj[i] = v

# ==

# module_init: _dp_module_init

# function _dp_module_init():
#     block start:
#         __dp_setitem(__dp_load_deleted_name("obj", obj), i, v)
#         return

# assign_tuple_unpack

a, b = it

# ==

# module_init: _dp_module_init

# function _dp_module_init():
#     block start:
#         _dp_tmp_1 = __dp_unpack(it, __dp_tuple(True, True))
#         __dp_store_global(globals(), "a", __dp_getitem(__dp_load_deleted_name("_dp_tmp_1", _dp_tmp_1), 0))
#         __dp_store_global(globals(), "b", __dp_getitem(__dp_load_deleted_name("_dp_tmp_1", _dp_tmp_1), 1))
#         _dp_tmp_1 = __dp_DELETED
#         return

# assign_star_unpack

a, *b = it

# ==

# module_init: _dp_module_init

# function _dp_module_init():
#     block start:
#         _dp_tmp_1 = __dp_unpack(it, __dp_tuple(True, False))
#         __dp_store_global(globals(), "a", __dp_getitem(__dp_load_deleted_name("_dp_tmp_1", _dp_tmp_1), 0))
#         __dp_store_global(globals(), "b", __dp_list(__dp_getitem(__dp_load_deleted_name("_dp_tmp_1", _dp_tmp_1), 1)))
#         _dp_tmp_1 = __dp_DELETED
#         return

# assign_multi_targets

a = b = f()

# ==

# module_init: _dp_module_init

# function _dp_module_init():
#     block start:
#         _dp_tmp_1 = f()
#         __dp_store_global(globals(), "a", _dp_tmp_1)
#         __dp_store_global(globals(), "b", _dp_tmp_1)
#         return

# ann_assign_simple

x: int = 1

# ==

# module_init: _dp_module_init

# function _dp_module_init():
#     block start:
#         __dp_store_global(globals(), "x", 1)
#         __annotate__ = __dp_exec_function_def_source('def __annotate__(_dp_format, _dp=__dp__, *, __dp__=__dp__, __dp_tuple=__dp_tuple):\n    if _dp.eq(_dp_format, 4):\n        return _dp.dict(__dp_tuple(("x", "int")))\n    if _dp.gt(_dp_format, 2):\n        raise _dp.builtins.NotImplementedError\n    return _dp.dict(__dp_tuple(("x", int)))', __dp_globals(), __dp_tuple(), "__annotate__")
#         __dp_store_global(globals(), "__annotate__", __dp_update_fn(__annotate__, "__annotate__", "__annotate__", None))
#         return

# ann_assign_attr

obj.x: int = 1

# ==

# module_init: _dp_module_init

# function _dp_module_init():
#     block start:
#         __dp_setattr(__dp_load_deleted_name("obj", obj), "x", 1)
#         return

# aug_assign_attr

obj.x += 1

# ==

# module_init: _dp_module_init

# function _dp_module_init():
#     block start:
#         __dp_setattr(__dp_load_deleted_name("obj", obj), "x", __dp_iadd(obj.x, 1))
#         return

# delete_mixed

del obj.x, obj[i], x

# ==

# module_init: _dp_module_init

# function _dp_module_init():
#     block start:
#         __dp_delattr(obj, "x")
#         __dp_delitem(obj, i)
#         __dp_delitem(globals(), "x")
#         return

# assert_no_msg

assert cond

# ==

# module_init: _dp_module_init

# function _dp_module_init():
#     block start:
#         if_term __debug__:
#             then:
#                 block _dp_bb_1:
#                     if_term not cond:
#                         then:
#                             block _dp_bb_0:
#                                 raise __dp_AssertionError
#                         else:
#                             jump _dp_bb_2
#             else:
#                 jump _dp_bb_2
#         block _dp_bb_2:
#             return

# assert_with_msg

assert cond, "oops"

# ==

# module_init: _dp_module_init

# function _dp_module_init():
#     block start:
#         if_term __debug__:
#             then:
#                 block _dp_bb_1:
#                     if_term not cond:
#                         then:
#                             block _dp_bb_0:
#                                 raise __dp_AssertionError("oops")
#                         else:
#                             jump _dp_bb_2
#             else:
#                 jump _dp_bb_2
#         block _dp_bb_2:
#             return

# raise_from

raise E from cause

# ==

# module_init: _dp_module_init

# function _dp_module_init():
#     block start:
#         raise __dp_raise_from(E, cause)

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

# module_init: _dp_module_init

# function _dp_module_init():
#     block start:
#         _dp_iter_1 = __dp_iter(it)
#         jump _dp_bb_3
#         block _dp_bb_3:
#             _dp_tmp_2 = __dp_next_or_sentinel(_dp_iter_1)
#             if_term __dp_is_(_dp_tmp_2, __dp__.ITER_COMPLETE):
#                 then:
#                     block _dp_bb_0:
#                         done()
#                         return
#                 else:
#                     block _dp_bb_2:
#                         x = _dp_tmp_2
#                         _dp_tmp_2 = None
#                         jump _dp_bb_1
#                         block _dp_bb_1:
#                             __dp_store_global(globals(), "x", x)
#                             body()
#                             jump _dp_bb_3

# while_else

while cond:
    body()
else:
    done()

# ==

# module_init: _dp_module_init

# function _dp_module_init():
#     block start:
#         if_term cond:
#             then:
#                 block _dp_bb_1:
#                     body()
#                     jump start
#             else:
#                 block _dp_bb_0:
#                     done()
#                     return

# with_as

with cm as x:
    body()

# ==

# snapshot regeneration failed
# panic: TryJump is not allowed in BbTerm

# function_local_ann_assign


def inner():
    value: int = 1
    return value


# ==

# module_init: _dp_module_init

# function inner():
#     block start:
#         value = 1
#         return value

# function _dp_module_init():
#     block start:
#         __dp_store_global(globals(), "inner", __dp_make_function("start", 0, "inner", "inner", __dp_tuple(), __dp_tuple(), __dp_tuple(), __dp_globals(), __name__, None, None))
#         return

# comprehension_global

xs = [x for x in it]
ys = {x for x in it}
zs = {k: v for k, v in items}

# ==

# module_init: _dp_module_init

# function _dp_listcomp_3(_dp_iter_2):
#     display_name: <listcomp>
#     block start:
#         _dp_tmp_1 = []
#         _dp_iter_1 = __dp_iter(_dp_iter_2)
#         jump _dp_bb_3
#         block _dp_bb_3:
#             _dp_tmp_2 = __dp_next_or_sentinel(_dp_iter_1)
#             if_term __dp_is_(_dp_tmp_2, __dp__.ITER_COMPLETE):
#                 then:
#                     block _dp_bb_0:
#                         return _dp_tmp_1
#                 else:
#                     block _dp_bb_2:
#                         x = _dp_tmp_2
#                         _dp_tmp_2 = None
#                         jump _dp_bb_1
#                         block _dp_bb_1:
#                             _dp_tmp_1.append(x)
#                             jump _dp_bb_3

# function _dp_setcomp_6(_dp_iter_5):
#     display_name: <setcomp>
#     block start:
#         _dp_tmp_4 = set()
#         _dp_iter_9 = __dp_iter(_dp_iter_5)
#         jump _dp_bb_3
#         block _dp_bb_3:
#             _dp_tmp_10 = __dp_next_or_sentinel(_dp_iter_9)
#             if_term __dp_is_(_dp_tmp_10, __dp__.ITER_COMPLETE):
#                 then:
#                     block _dp_bb_0:
#                         return _dp_tmp_4
#                 else:
#                     block _dp_bb_2:
#                         x = _dp_tmp_10
#                         _dp_tmp_10 = None
#                         jump _dp_bb_1
#                         block _dp_bb_1:
#                             _dp_tmp_4.add(x)
#                             jump _dp_bb_3

# function _dp_dictcomp_9(_dp_iter_8):
#     display_name: <dictcomp>
#     block start:
#         _dp_tmp_7 = {}
#         _dp_iter_17 = __dp_iter(_dp_iter_8)
#         jump _dp_bb_3
#         block _dp_bb_3:
#             _dp_tmp_18 = __dp_next_or_sentinel(_dp_iter_17)
#             if_term __dp_is_(_dp_tmp_18, __dp__.ITER_COMPLETE):
#                 then:
#                     block _dp_bb_0:
#                         return _dp_tmp_7
#                 else:
#                     block _dp_bb_2:
#                         _dp_tmp_20 = __dp_unpack(_dp_tmp_18, __dp_tuple(True, True))
#                         k = __dp_getitem(_dp_tmp_20, 0)
#                         v = __dp_getitem(_dp_tmp_20, 1)
#                         del _dp_tmp_20
#                         _dp_tmp_18 = None
#                         jump _dp_bb_1
#                         block _dp_bb_1:
#                             __dp_setitem(_dp_tmp_7, k, v)
#                             jump _dp_bb_3

# function _dp_module_init():
#     block start:
#         _dp_listcomp_3 = __dp_make_function("start", 0, "<listcomp>", "_dp_listcomp_3", __dp_tuple("_dp_iter_2"), __dp_tuple(__dp_tuple("_dp_iter_2", "Any", False)), __dp_tuple(), __dp_globals(), __name__, None, None)
#         __dp_store_global(globals(), "xs", _dp_listcomp_3(it))
#         _dp_setcomp_6 = __dp_make_function("start", 1, "<setcomp>", "_dp_setcomp_6", __dp_tuple("_dp_iter_5"), __dp_tuple(__dp_tuple("_dp_iter_5", "Any", False)), __dp_tuple(), __dp_globals(), __name__, None, None)
#         __dp_store_global(globals(), "ys", _dp_setcomp_6(it))
#         _dp_dictcomp_9 = __dp_make_function("start", 2, "<dictcomp>", "_dp_dictcomp_9", __dp_tuple("_dp_iter_8"), __dp_tuple(__dp_tuple("_dp_iter_8", "Any", False)), __dp_tuple(), __dp_globals(), __name__, None, None)
#         __dp_store_global(globals(), "zs", _dp_dictcomp_9(items))
#         return

# comprehension_in_function


def f():
    return [x for x in it if x > 0]


# ==

# module_init: _dp_module_init

# function f.<locals>._dp_listcomp_3(_dp_iter_2):
#     display_name: <listcomp>
#     block start:
#         _dp_tmp_1 = []
#         _dp_iter_1 = __dp_iter(_dp_iter_2)
#         jump _dp_bb_4
#         block _dp_bb_4:
#             _dp_tmp_2 = __dp_next_or_sentinel(_dp_iter_1)
#             if_term __dp_is_(_dp_tmp_2, __dp__.ITER_COMPLETE):
#                 then:
#                     block _dp_bb_0:
#                         return _dp_tmp_1
#                 else:
#                     block _dp_bb_3:
#                         x = _dp_tmp_2
#                         _dp_tmp_2 = None
#                         jump _dp_bb_2
#                         block _dp_bb_2:
#                             if_term __dp_gt(x, 0):
#                                 then:
#                                     block _dp_bb_1:
#                                         _dp_tmp_1.append(x)
#                                         jump _dp_bb_4
#                                 else:
#                                     jump _dp_bb_4

# function f():
#     block start:
#         _dp_listcomp_3 = __dp_make_function("start", 0, "<listcomp>", "f.<locals>._dp_listcomp_3", __dp_tuple("_dp_iter_2"), __dp_tuple(__dp_tuple("_dp_iter_2", "Any", False)), __dp_tuple(), __dp_globals(), __name__, None, None)
#         return _dp_listcomp_3(it)

# function _dp_module_init():
#     block start:
#         __dp_store_global(globals(), "f", __dp_make_function("start", 1, "f", "f", __dp_tuple(), __dp_tuple(), __dp_tuple(), __dp_globals(), __name__, None, None))
#         return

# comprehension_in_class_body


class C:
    xs = [x for x in it]


# ==

# module_init: _dp_module_init

# function C._dp_listcomp_3(_dp_iter_2):
#     display_name: <listcomp>
#     block start:
#         _dp_tmp_1 = []
#         _dp_iter_1 = __dp_iter(_dp_iter_2)
#         jump _dp_bb_3
#         block _dp_bb_3:
#             _dp_tmp_2 = __dp_next_or_sentinel(_dp_iter_1)
#             if_term __dp_is_(_dp_tmp_2, __dp__.ITER_COMPLETE):
#                 then:
#                     block _dp_bb_0:
#                         return _dp_tmp_1
#                 else:
#                     block _dp_bb_2:
#                         x = _dp_tmp_2
#                         _dp_tmp_2 = None
#                         jump _dp_bb_1
#                         block _dp_bb_1:
#                             _dp_tmp_1.append(x)
#                             jump _dp_bb_3

# function _dp_class_ns_C(_dp_class_ns, _dp_classcell_arg):
#     block start:
#         _dp_classcell = _dp_classcell_arg
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__module__", __name__)
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__qualname__", "C")
#         _dp_listcomp_3 = __dp_make_function("start", 0, "<listcomp>", "C._dp_listcomp_3", __dp_tuple("_dp_iter_2"), __dp_tuple(__dp_tuple("_dp_iter_2", "Any", False)), __dp_tuple(), __dp_globals(), __name__, None, None)
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "xs", _dp_listcomp_3(__dp_class_lookup_global(_dp_class_ns, "it", globals())))
#         return

# function _dp_define_class_C(_dp_class_ns_fn, _dp_class_ns_outer, _dp_prepare_dict=None):
#     block start:
#         _dp_class_ns = _dp_class_ns_outer
#         return __dp_create_class("C", _dp_class_ns_fn, __dp_tuple(), _dp_prepare_dict, False, 3, ())

# function _dp_module_init():
#     block start:
#         _dp_class_ns_C = __dp_make_function("start", 1, "_dp_class_ns_C", "_dp_class_ns_C", __dp_tuple("_dp_class_ns", "_dp_classcell_arg"), __dp_tuple(__dp_tuple("_dp_class_ns", "Any", False), __dp_tuple("_dp_classcell_arg", "Any", False)), __dp_tuple(), __dp_globals(), __name__, None, None)
#         _dp_define_class_C = __dp_make_function("start", 2, "_dp_define_class_C", "_dp_define_class_C", __dp_tuple("_dp_class_ns_fn", "_dp_class_ns_outer", "_dp_prepare_dict"), __dp_tuple(__dp_tuple("_dp_class_ns_fn", "Any", False), __dp_tuple("_dp_class_ns_outer", "Any", False), __dp_tuple("_dp_prepare_dict", "Any", True)), __dp_tuple(None), __dp_globals(), __name__, None, None)
#         __dp_store_global(globals(), "C", _dp_define_class_C(_dp_class_ns_C, globals()))
#         return

# with_multi

with a as x, b as y:
    body()

# ==

# snapshot regeneration failed
# panic: TryJump is not allowed in BbTerm

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

# module_init: _dp_module_init

# function _dp_module_init():
#     block start:
#         _dp_match_1 = value
#         if_term __dp_eq(_dp_match_1, 1):
#             then:
#                 block _dp_bb_0:
#                     one()
#                     return
#             else:
#                 block _dp_bb_1:
#                     other()
#                     return

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
# panic: TryJump is not allowed in BbTerm

# closure_cell_simple


def outer():
    x = 5

    def inner():
        return x

    return inner()


# ==

# module_init: _dp_module_init

# function outer.<locals>.inner():
#     entry_liveins: [_dp_cell_x]
#     freevars: [x->_dp_cell_x@inherited]
#     block start:
#         return __dp_load_cell(_dp_cell_x)

# function outer():
#     local_cell_slots: [_dp_cell_x]
#     cellvars: [x->_dp_cell_x@deferred]
#     block start:
#         _dp_cell_x = __dp_make_cell()
#         __dp_store_cell(_dp_cell_x, 5)
#         inner = __dp_make_function("start", 0, "inner", "outer.<locals>.inner", __dp_tuple(__dp_tuple("_dp_cell_x", _dp_cell_x)), __dp_tuple(), __dp_tuple(), __dp_globals(), __name__, None, None)
#         return inner()

# function _dp_module_init():
#     block start:
#         __dp_store_global(globals(), "outer", __dp_make_function("start", 1, "outer", "outer", __dp_tuple(), __dp_tuple(), __dp_tuple(), __dp_globals(), __name__, None, None))
#         return

# bb_if_else_function


def choose(a, b):
    total = a + b
    if total > 5:
        return a
    else:
        return b


# ==

# module_init: _dp_module_init

# function choose(a, b):
#     block start:
#         total = a + b
#         if_term __dp_gt(total, 5):
#             then:
#                 block _dp_bb_0:
#                     return a
#             else:
#                 block _dp_bb_1:
#                     return b

# function _dp_module_init():
#     block start:
#         __dp_store_global(globals(), "choose", __dp_make_function("start", 0, "choose", "choose", __dp_tuple("a", "b"), __dp_tuple(__dp_tuple("a", "Any", False), __dp_tuple("b", "Any", False)), __dp_tuple(), __dp_globals(), __name__, None, None))
#         return

# closure_cell_nonlocal


def outer():
    x = 5

    def inner():
        nonlocal x
        x = 2
        return x

    return inner()


# ==

# module_init: _dp_module_init

# function outer.<locals>.inner():
#     entry_liveins: [_dp_cell_x]
#     freevars: [x->_dp_cell_x@inherited]
#     block start:
#         __dp_store_cell(_dp_cell_x, 2)
#         return __dp_load_cell(_dp_cell_x)

# function outer():
#     local_cell_slots: [_dp_cell_x]
#     cellvars: [x->_dp_cell_x@deferred]
#     block start:
#         _dp_cell_x = __dp_make_cell()
#         __dp_store_cell(_dp_cell_x, 5)
#         inner = __dp_make_function("start", 0, "inner", "outer.<locals>.inner", __dp_tuple(__dp_tuple("_dp_cell_x", _dp_cell_x)), __dp_tuple(), __dp_tuple(), __dp_globals(), __name__, None, None)
#         return inner()

# function _dp_module_init():
#     block start:
#         __dp_store_global(globals(), "outer", __dp_make_function("start", 1, "outer", "outer", __dp_tuple(), __dp_tuple(), __dp_tuple(), __dp_globals(), __name__, None, None))
#         return

# plain try / catch

try:
    print(1)
except Exception:
    print(2)

# ==

# snapshot regeneration failed
# panic: TryJump is not allowed in BbTerm

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

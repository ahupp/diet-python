# import_simple

import a

# ==

# function _dp_module_init():
#     function_id: 0
#     block bb1:
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..1 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..1, id: Name("a"), ctx: Load }), value: Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 28..88 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 28..44 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 28..44, value: "import_" }) }), args: [Positional(Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(None), range: 0..3 }, literal: StringLiteral(CoreStringLiteral { node_index: NodeIndex(None), range: 0..3, value: "a" }) })), Positional(Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 79..87 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 79..87, id: Name("__spec__"), ctx: Load }) }))], keywords: [] }) })
#         return Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) })

# import_dotted_alias

import a.b as c

# ==

# function _dp_module_init():
#     function_id: 0
#     block bb1:
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..1 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..1, id: Name("c"), ctx: Store }), value: Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 4..64 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 4..24 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 4..24, value: "import_attr" }) }), args: [Positional(Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 25..58 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 25..41 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 25..41, value: "import_" }) }), args: [Positional(Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(None), range: 42..47 }, literal: StringLiteral(CoreStringLiteral { node_index: NodeIndex(None), range: 42..47, value: "a.b" }) })), Positional(Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 49..57 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 49..57, id: Name("__spec__"), ctx: Load }) }))], keywords: [] })), Positional(Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(None), range: 60..63 }, literal: StringLiteral(CoreStringLiteral { node_index: NodeIndex(None), range: 60..63, value: "b" }) }))], keywords: [] }) })
#         return Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) })

# import_from_alias

from pkg.mod import name as alias

# ==

# function _dp_module_init():
#     function_id: 0
#     block bb1:
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..12 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..12, id: Name("_dp_import_1"), ctx: Store }), value: Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 15..62 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 15..31 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 15..31, value: "import_" }) }), args: [Positional(Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(None), range: 32..41 }, literal: StringLiteral(CoreStringLiteral { node_index: NodeIndex(None), range: 32..41, value: "pkg.mod" }) })), Positional(Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 43..51 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 43..51, id: Name("__spec__"), ctx: Load }) })), Positional(Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 53..61 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 53..61 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 53..61, value: "list" }) }), args: [Positional(Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 0..23 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..21 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..21, value: "tuple_values" }) }), args: [Positional(Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(None), range: 54..60 }, literal: StringLiteral(CoreStringLiteral { node_index: NodeIndex(None), range: 54..60, value: "name" }) }))], keywords: [] }))], keywords: [] }))], keywords: [] }) })
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..5 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..5, id: Name("alias"), ctx: Load }), value: Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 28..109 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 28..48 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 28..48, value: "import_attr" }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..12 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..12, id: Name("_dp_import_1"), ctx: Load }) })), Positional(Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(None), range: 0..6 }, literal: StringLiteral(CoreStringLiteral { node_index: NodeIndex(None), range: 0..6, value: "name" }) }))], keywords: [] }) })
#         return Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) })

# decorator_function


@dec
def f():
    pass


# ==

# function f():
#     function_id: 0
#     block bb1:
#         return Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) })

# function _dp_module_init():
#     function_id: 1
#     block bb1:
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..1 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..1, id: Name("f"), ctx: Load }), value: Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 0..70 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(3), range: 3..6 }, name: ExprName(ExprName { node_index: NodeIndex(3), range: 3..6, id: Name("dec"), ctx: Load }) }), args: [Positional(MakeFunction(MakeFunction { _meta: Meta { node_index: NodeIndex(None), range: 0..200 }, function_id: FunctionId(0), kind: Function, param_defaults: Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 0..23 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..21 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..21, value: "tuple_values" }) }), args: [], keywords: [] }), annotate_fn: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) }) }))], keywords: [] }) })
#         return Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) })

# assign_attr

obj.x = 1

# ==

# function _dp_module_init():
#     function_id: 0
#     block bb1:
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_assign_value_1"), ctx: Store }), value: Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(3), range: 9..10 }, literal: NumberLiteral(CoreNumberLiteral { node_index: NodeIndex(3), range: 9..10, value: Int(1) }) }) })
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_assign_obj_2"), ctx: Store }), value: Call(Call { _meta: Meta { node_index: NodeIndex(5), range: 1..4 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(5), range: 1..4 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(5), range: 1..4, value: "load_deleted_name" }) }), args: [Positional(Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(None), range: 0..5 }, literal: StringLiteral(CoreStringLiteral { node_index: NodeIndex(None), range: 0..5, value: "obj" }) })), Positional(Load(Load { _meta: Meta { node_index: NodeIndex(5), range: 1..4 }, name: ExprName(ExprName { node_index: NodeIndex(5), range: 1..4, id: Name("obj"), ctx: Load }) }))], keywords: [] }) })
#         SetAttr(SetAttr { _meta: Meta { node_index: NodeIndex(4), range: 1..6 }, value: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_assign_obj_2"), ctx: Load }) }), attr: Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(4), range: 1..6 }, literal: StringLiteral(CoreStringLiteral { node_index: NodeIndex(4), range: 1..6, value: "x" }) }), replacement: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_assign_value_1"), ctx: Load }) }) })
#         return Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) })

# assign_subscript

obj[i] = v

# ==

# function _dp_module_init():
#     function_id: 0
#     block bb1:
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_assign_value_1"), ctx: Store }), value: Load(Load { _meta: Meta { node_index: NodeIndex(3), range: 10..11 }, name: ExprName(ExprName { node_index: NodeIndex(3), range: 10..11, id: Name("v"), ctx: Load }) }) })
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_assign_obj_2"), ctx: Store }), value: Call(Call { _meta: Meta { node_index: NodeIndex(5), range: 1..4 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(5), range: 1..4 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(5), range: 1..4, value: "load_deleted_name" }) }), args: [Positional(Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(None), range: 0..5 }, literal: StringLiteral(CoreStringLiteral { node_index: NodeIndex(None), range: 0..5, value: "obj" }) })), Positional(Load(Load { _meta: Meta { node_index: NodeIndex(5), range: 1..4 }, name: ExprName(ExprName { node_index: NodeIndex(5), range: 1..4, id: Name("obj"), ctx: Load }) }))], keywords: [] }) })
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_assign_index_3"), ctx: Store }), value: Load(Load { _meta: Meta { node_index: NodeIndex(6), range: 5..6 }, name: ExprName(ExprName { node_index: NodeIndex(6), range: 5..6, id: Name("i"), ctx: Load }) }) })
#         SetItem(SetItem { _meta: Meta { node_index: NodeIndex(4), range: 1..7 }, value: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_assign_obj_2"), ctx: Load }) }), index: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_assign_index_3"), ctx: Load }) }), replacement: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_assign_value_1"), ctx: Load }) }) })
#         return Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) })

# assign_tuple_unpack

a, b = it

# ==

# function _dp_module_init():
#     function_id: 0
#     block bb1:
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_assign_value_1"), ctx: Store }), value: Load(Load { _meta: Meta { node_index: NodeIndex(3), range: 8..10 }, name: ExprName(ExprName { node_index: NodeIndex(3), range: 8..10, id: Name("it"), ctx: Load }) }) })
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_unpack_2"), ctx: Store }), value: Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 0..23 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..23 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..23, value: "unpack" }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_assign_value_1"), ctx: Load }) })), Positional(Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 0..23 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..21 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..21, value: "tuple_values" }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "TRUE" }) })), Positional(Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "TRUE" }) }))], keywords: [] }))], keywords: [] }) })
#         Store(Store { _meta: Meta { node_index: NodeIndex(5), range: 1..2 }, name: ExprName(ExprName { node_index: NodeIndex(5), range: 1..2, id: Name("a"), ctx: Store }), value: GetItem(GetItem { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, value: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_unpack_2"), ctx: Load }) }), index: Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(None), range: 0..1 }, literal: NumberLiteral(CoreNumberLiteral { node_index: NodeIndex(None), range: 0..1, value: Int(0) }) }) }) })
#         Store(Store { _meta: Meta { node_index: NodeIndex(6), range: 4..5 }, name: ExprName(ExprName { node_index: NodeIndex(6), range: 4..5, id: Name("b"), ctx: Store }), value: GetItem(GetItem { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, value: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_unpack_2"), ctx: Load }) }), index: Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(None), range: 0..1 }, literal: NumberLiteral(CoreNumberLiteral { node_index: NodeIndex(None), range: 0..1, value: Int(1) }) }) }) })
#         Del(Del { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_unpack_2"), ctx: Del }), quietly: false })
#         return Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) })

# assign_star_unpack

a, *b = it

# ==

# function _dp_module_init():
#     function_id: 0
#     block bb1:
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_assign_value_1"), ctx: Store }), value: Load(Load { _meta: Meta { node_index: NodeIndex(3), range: 9..11 }, name: ExprName(ExprName { node_index: NodeIndex(3), range: 9..11, id: Name("it"), ctx: Load }) }) })
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_unpack_2"), ctx: Store }), value: Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 0..23 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..23 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..23, value: "unpack" }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_assign_value_1"), ctx: Load }) })), Positional(Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 0..23 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..21 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..21, value: "tuple_values" }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "TRUE" }) })), Positional(Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "FALSE" }) }))], keywords: [] }))], keywords: [] }) })
#         Store(Store { _meta: Meta { node_index: NodeIndex(5), range: 1..2 }, name: ExprName(ExprName { node_index: NodeIndex(5), range: 1..2, id: Name("a"), ctx: Store }), value: GetItem(GetItem { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, value: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_unpack_2"), ctx: Load }) }), index: Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(None), range: 0..1 }, literal: NumberLiteral(CoreNumberLiteral { node_index: NodeIndex(None), range: 0..1, value: Int(0) }) }) }) })
#         Store(Store { _meta: Meta { node_index: NodeIndex(7), range: 5..6 }, name: ExprName(ExprName { node_index: NodeIndex(7), range: 5..6, id: Name("b"), ctx: Store }), value: Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "list" }) }), args: [Positional(GetItem(GetItem { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, value: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_unpack_2"), ctx: Load }) }), index: Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(None), range: 0..1 }, literal: NumberLiteral(CoreNumberLiteral { node_index: NodeIndex(None), range: 0..1, value: Int(1) }) }) }))], keywords: [] }) })
#         Del(Del { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_unpack_2"), ctx: Del }), quietly: false })
#         return Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) })

# assign_multi_targets

a = b = f()

# ==

# function _dp_module_init():
#     function_id: 0
#     block bb1:
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_assign_value_1"), ctx: Store }), value: Call(Call { _meta: Meta { node_index: NodeIndex(3), range: 9..12 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(4), range: 9..10 }, name: ExprName(ExprName { node_index: NodeIndex(4), range: 9..10, id: Name("f"), ctx: Load }) }), args: [], keywords: [] }) })
#         Store(Store { _meta: Meta { node_index: NodeIndex(5), range: 1..2 }, name: ExprName(ExprName { node_index: NodeIndex(5), range: 1..2, id: Name("a"), ctx: Store }), value: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_assign_value_1"), ctx: Load }) }) })
#         Store(Store { _meta: Meta { node_index: NodeIndex(6), range: 5..6 }, name: ExprName(ExprName { node_index: NodeIndex(6), range: 5..6, id: Name("b"), ctx: Store }), value: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_assign_value_1"), ctx: Load }) }) })
#         return Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) })

# ann_assign_simple

x: int = 1

# ==

# function __annotate__(_dp_format, __soac__):
#     function_id: 0
#     block bb1:
#         if_term Call(Call { _meta: Meta { node_index: NodeIndex(17), range: 144..170 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(18), range: 144..155 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(18), range: 144..155, value: "eq" }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(20), range: 156..166 }, name: ExprName(ExprName { node_index: NodeIndex(20), range: 156..166, id: Name("_dp_format"), ctx: Load }) })), Positional(Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(21), range: 168..169 }, literal: NumberLiteral(CoreNumberLiteral { node_index: NodeIndex(21), range: 168..169, value: Int(4) }) }))], keywords: [] }):
#             then:
#                 block bb5:
#                     return Call(Call { _meta: Meta { node_index: NodeIndex(23), range: 0..43 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(24), range: 0..13 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(24), range: 0..13, value: "dict" }) }), args: [Positional(Call(Call { _meta: Meta { node_index: NodeIndex(26), range: 0..23 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(27), range: 0..21 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(27), range: 0..21, value: "tuple_values" }) }), args: [Positional(Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 0..23 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..21 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..21, value: "tuple_values" }) }), args: [Positional(Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(30), range: 0..3 }, literal: StringLiteral(CoreStringLiteral { node_index: NodeIndex(30), range: 0..3, value: "x" }) })), Positional(Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(31), range: 0..5 }, literal: StringLiteral(CoreStringLiteral { node_index: NodeIndex(31), range: 0..5, value: "int" }) }))], keywords: [] }))], keywords: [] }))], keywords: [] })
#             else:
#                 block bb2:
#                     if_term Call(Call { _meta: Meta { node_index: NodeIndex(33), range: 229..255 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(34), range: 229..240 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(34), range: 229..240, value: "gt" }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(36), range: 241..251 }, name: ExprName(ExprName { node_index: NodeIndex(36), range: 241..251, id: Name("_dp_format"), ctx: Load }) })), Positional(Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(37), range: 253..254 }, literal: NumberLiteral(CoreNumberLiteral { node_index: NodeIndex(37), range: 253..254, value: Int(2) }) }))], keywords: [] }):
#                         then:
#                             block bb4:
#                                 raise GetAttr(GetAttr { _meta: Meta { node_index: NodeIndex(39), range: 271..308 }, value: Load(Load { _meta: Meta { node_index: NodeIndex(40), range: 271..288 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(40), range: 271..288, value: "builtins" }) }), attr: Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(39), range: 271..308 }, literal: StringLiteral(CoreStringLiteral { node_index: NodeIndex(39), range: 271..308, value: "NotImplementedError" }) }) })
#                         else:
#                             block bb3:
#                                 return Call(Call { _meta: Meta { node_index: NodeIndex(43), range: 0..43 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(44), range: 0..13 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(44), range: 0..13, value: "dict" }) }), args: [Positional(Call(Call { _meta: Meta { node_index: NodeIndex(46), range: 0..23 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(47), range: 0..21 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(47), range: 0..21, value: "tuple_values" }) }), args: [Positional(Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 0..23 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..21 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..21, value: "tuple_values" }) }), args: [Positional(Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(50), range: 0..3 }, literal: StringLiteral(CoreStringLiteral { node_index: NodeIndex(50), range: 0..3, value: "x" }) })), Positional(Load(Load { _meta: Meta { node_index: NodeIndex(51), range: 4..7 }, name: ExprName(ExprName { node_index: NodeIndex(51), range: 4..7, id: Name("int"), ctx: Load }) }))], keywords: [] }))], keywords: [] }))], keywords: [] })

# function _dp_module_init():
#     function_id: 1
#     block bb1:
#         Store(Store { _meta: Meta { node_index: NodeIndex(4), range: 1..2 }, name: ExprName(ExprName { node_index: NodeIndex(4), range: 1..2, id: Name("x"), ctx: Store }), value: Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(3), range: 10..11 }, literal: NumberLiteral(CoreNumberLiteral { node_index: NodeIndex(3), range: 10..11, value: Int(1) }) }) })
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..12 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..12, id: Name("__annotate__"), ctx: Load }), value: MakeFunction(MakeFunction { _meta: Meta { node_index: NodeIndex(None), range: 0..200 }, function_id: FunctionId(0), kind: Function, param_defaults: Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 0..23 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..21 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..21, value: "tuple_values" }) }), args: [Positional(Call(Call { _meta: Meta { node_index: NodeIndex(6), range: 70..132 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(7), range: 70..80 }, name: ExprName(ExprName { node_index: NodeIndex(7), range: 70..80, id: Name("__import__"), ctx: Load }) }), args: [Positional(Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(8), range: 81..95 }, literal: StringLiteral(CoreStringLiteral { node_index: NodeIndex(8), range: 81..95, value: "soac.runtime" }) })), Positional(Call(Call { _meta: Meta { node_index: NodeIndex(9), range: 97..106 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(10), range: 97..104 }, name: ExprName(ExprName { node_index: NodeIndex(10), range: 97..104, id: Name("globals"), ctx: Load }) }), args: [], keywords: [] })), Positional(Call(Call { _meta: Meta { node_index: NodeIndex(11), range: 108..114 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(12), range: 108..112 }, name: ExprName(ExprName { node_index: NodeIndex(12), range: 108..112, id: Name("dict"), ctx: Load }) }), args: [], keywords: [] })), Positional(Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 0..23 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..21 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..21, value: "tuple_values" }) }), args: [Positional(Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(14), range: 117..126 }, literal: StringLiteral(CoreStringLiteral { node_index: NodeIndex(14), range: 117..126, value: "runtime" }) }))], keywords: [] })), Positional(Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(15), range: 130..131 }, literal: NumberLiteral(CoreNumberLiteral { node_index: NodeIndex(15), range: 130..131, value: Int(0) }) }))], keywords: [] }))], keywords: [] }), annotate_fn: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) }) }) })
#         return Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) })

# ann_assign_attr

obj.x: int = 1

# ==

# function _dp_module_init():
#     function_id: 0
#     block bb1:
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_assign_value_1"), ctx: Store }), value: Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(3), range: 14..15 }, literal: NumberLiteral(CoreNumberLiteral { node_index: NodeIndex(3), range: 14..15, value: Int(1) }) }) })
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_assign_obj_2"), ctx: Store }), value: Call(Call { _meta: Meta { node_index: NodeIndex(5), range: 1..4 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(5), range: 1..4 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(5), range: 1..4, value: "load_deleted_name" }) }), args: [Positional(Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(None), range: 0..5 }, literal: StringLiteral(CoreStringLiteral { node_index: NodeIndex(None), range: 0..5, value: "obj" }) })), Positional(Load(Load { _meta: Meta { node_index: NodeIndex(5), range: 1..4 }, name: ExprName(ExprName { node_index: NodeIndex(5), range: 1..4, id: Name("obj"), ctx: Load }) }))], keywords: [] }) })
#         SetAttr(SetAttr { _meta: Meta { node_index: NodeIndex(4), range: 1..6 }, value: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_assign_obj_2"), ctx: Load }) }), attr: Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(4), range: 1..6 }, literal: StringLiteral(CoreStringLiteral { node_index: NodeIndex(4), range: 1..6, value: "x" }) }), replacement: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_assign_value_1"), ctx: Load }) }) })
#         return Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) })

# aug_assign_attr

obj.x += 1

# ==

# function _dp_module_init():
#     function_id: 0
#     block bb1:
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_augassign_obj_1"), ctx: Store }), value: Call(Call { _meta: Meta { node_index: NodeIndex(5), range: 1..4 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(5), range: 1..4 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(5), range: 1..4, value: "load_deleted_name" }) }), args: [Positional(Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(None), range: 0..5 }, literal: StringLiteral(CoreStringLiteral { node_index: NodeIndex(None), range: 0..5, value: "obj" }) })), Positional(Load(Load { _meta: Meta { node_index: NodeIndex(5), range: 1..4 }, name: ExprName(ExprName { node_index: NodeIndex(5), range: 1..4, id: Name("obj"), ctx: Load }) }))], keywords: [] }) })
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_augassign_value_2"), ctx: Store }), value: GetAttr(GetAttr { _meta: Meta { node_index: NodeIndex(4), range: 1..6 }, value: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_augassign_obj_1"), ctx: Load }) }), attr: Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(4), range: 1..6 }, literal: StringLiteral(CoreStringLiteral { node_index: NodeIndex(4), range: 1..6, value: "x" }) }) }) })
#         SetAttr(SetAttr { _meta: Meta { node_index: NodeIndex(4), range: 1..6 }, value: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_augassign_obj_1"), ctx: Load }) }), attr: Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(4), range: 1..6 }, literal: StringLiteral(CoreStringLiteral { node_index: NodeIndex(4), range: 1..6, value: "x" }) }), replacement: BinOp(BinOp { _meta: Meta { node_index: NodeIndex(4), range: 1..6 }, kind: InplaceAdd, left: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_augassign_value_2"), ctx: Load }) }), right: Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(3), range: 10..11 }, literal: NumberLiteral(CoreNumberLiteral { node_index: NodeIndex(3), range: 10..11, value: Int(1) }) }) }) })
#         return Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) })

# delete_mixed

del obj.x, obj[i], x

# ==

# function _dp_module_init():
#     function_id: 0
#     block bb1:
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_delete_obj_1"), ctx: Store }), value: Call(Call { _meta: Meta { node_index: NodeIndex(4), range: 5..8 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(4), range: 5..8 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(4), range: 5..8, value: "load_deleted_name" }) }), args: [Positional(Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(None), range: 0..5 }, literal: StringLiteral(CoreStringLiteral { node_index: NodeIndex(None), range: 0..5, value: "obj" }) })), Positional(Load(Load { _meta: Meta { node_index: NodeIndex(4), range: 5..8 }, name: ExprName(ExprName { node_index: NodeIndex(4), range: 5..8, id: Name("obj"), ctx: Load }) }))], keywords: [] }) })
#         Call(Call { _meta: Meta { node_index: NodeIndex(3), range: 5..10 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(3), range: 5..10 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(3), range: 5..10, value: "delattr" }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_delete_obj_1"), ctx: Load }) })), Positional(Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(None), range: 0..3 }, literal: StringLiteral(CoreStringLiteral { node_index: NodeIndex(None), range: 0..3, value: "x" }) }))], keywords: [] })
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_delete_obj_2"), ctx: Store }), value: Call(Call { _meta: Meta { node_index: NodeIndex(6), range: 12..15 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(6), range: 12..15 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(6), range: 12..15, value: "load_deleted_name" }) }), args: [Positional(Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(None), range: 0..5 }, literal: StringLiteral(CoreStringLiteral { node_index: NodeIndex(None), range: 0..5, value: "obj" }) })), Positional(Load(Load { _meta: Meta { node_index: NodeIndex(6), range: 12..15 }, name: ExprName(ExprName { node_index: NodeIndex(6), range: 12..15, id: Name("obj"), ctx: Load }) }))], keywords: [] }) })
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_delete_index_3"), ctx: Store }), value: Load(Load { _meta: Meta { node_index: NodeIndex(7), range: 16..17 }, name: ExprName(ExprName { node_index: NodeIndex(7), range: 16..17, id: Name("i"), ctx: Load }) }) })
#         DelItem(DelItem { _meta: Meta { node_index: NodeIndex(5), range: 12..18 }, value: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_delete_obj_2"), ctx: Load }) }), index: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_delete_index_3"), ctx: Load }) }) })
#         Del(Del { _meta: Meta { node_index: NodeIndex(8), range: 20..21 }, name: ExprName(ExprName { node_index: NodeIndex(8), range: 20..21, id: Name("x"), ctx: Del }), quietly: false })
#         return Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) })

# assert_no_msg

assert cond

# ==

# function _dp_module_init():
#     function_id: 0
#     block bb1:
#         if_term Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 4..13 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 4..13, id: Name("__debug__"), ctx: Load }) }):
#             then:
#                 block bb2:
#                     if_term UnaryOp(UnaryOp { _meta: Meta { node_index: NodeIndex(None), range: 22..53 }, kind: Not, operand: Load(Load { _meta: Meta { node_index: NodeIndex(3), range: 8..12 }, name: ExprName(ExprName { node_index: NodeIndex(3), range: 8..12, id: Name("cond"), ctx: Load }) }) }):
#                         then:
#                             block bb3:
#                                 raise Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 69..92 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 69..92, value: "AssertionError" }) })
#                         else:
#                             jump bb0
#             else:
#                 jump bb0
#         block bb0:
#             return Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) })

# assert_with_msg

assert cond, "oops"

# ==

# function _dp_module_init():
#     function_id: 0
#     block bb1:
#         if_term Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 4..13 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 4..13, id: Name("__debug__"), ctx: Load }) }):
#             then:
#                 block bb2:
#                     if_term UnaryOp(UnaryOp { _meta: Meta { node_index: NodeIndex(None), range: 22..53 }, kind: Not, operand: Load(Load { _meta: Meta { node_index: NodeIndex(3), range: 8..12 }, name: ExprName(ExprName { node_index: NodeIndex(3), range: 8..12, id: Name("cond"), ctx: Load }) }) }):
#                         then:
#                             block bb3:
#                                 raise Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 69..120 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 69..92 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 69..92, value: "AssertionError" }) }), args: [Positional(Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(4), range: 14..20 }, literal: StringLiteral(CoreStringLiteral { node_index: NodeIndex(4), range: 14..20, value: "oops" }) }))], keywords: [] })
#                         else:
#                             jump bb0
#             else:
#                 jump bb0
#         block bb0:
#             return Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) })

# raise_from

raise E from cause

# ==

# function _dp_module_init():
#     function_id: 0
#     block bb1:
#         raise Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 6..83 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 6..25 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 6..25, value: "raise_from" }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(3), range: 7..8 }, name: ExprName(ExprName { node_index: NodeIndex(3), range: 7..8, id: Name("E"), ctx: Load }) })), Positional(Load(Load { _meta: Meta { node_index: NodeIndex(4), range: 14..19 }, name: ExprName(ExprName { node_index: NodeIndex(4), range: 14..19, id: Name("cause"), ctx: Load }) }))], keywords: [] })

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

# function _dp_module_init():
#     function_id: 0
#     block bb3:
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..12 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..12, id: Name("_dp_iter_0_0"), ctx: Load }), value: Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 28..74 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 28..41 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 28..41, value: "iter" }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(3), range: 10..12 }, name: ExprName(ExprName { node_index: NodeIndex(3), range: 10..12, id: Name("it"), ctx: Load }) }))], keywords: [] }) })
#         jump bb1
#         block bb1:
#             Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..11 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..11, id: Name("_dp_tmp_0_1"), ctx: Load }), value: Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 27..81 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 27..52 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 27..52, value: "next_or_sentinel" }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..12 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..12, id: Name("_dp_iter_0_0"), ctx: Load }) }))], keywords: [] }) })
#             if_term BinOp(BinOp { _meta: Meta { node_index: NodeIndex(None), range: 0..54 }, kind: Is, left: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..11 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..11, id: Name("_dp_tmp_0_1"), ctx: Load }) }), right: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 32..54 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 32..54, value: "ITER_COMPLETE" }) }) }):
#                 then:
#                     block bb4:
#                         Call(Call { _meta: Meta { node_index: NodeIndex(9), range: 35..41 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(10), range: 35..39 }, name: ExprName(ExprName { node_index: NodeIndex(10), range: 35..39, id: Name("done"), ctx: Load }) }), args: [], keywords: [] })
#                         return Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) })
#                 else:
#                     block bb2:
#                         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..11 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..11, id: Name("_dp_tmp_0_1"), ctx: Load }), value: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..11 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..11, id: Name("_dp_tmp_0_1"), ctx: Load }) }) })
#                         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..1 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..1, id: Name("x"), ctx: Load }), value: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..11 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..11, id: Name("_dp_tmp_0_1"), ctx: Load }) }) })
#                         Del(Del { _meta: Meta { node_index: NodeIndex(None), range: 0..11 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..11, id: Name("_dp_tmp_0_1"), ctx: Load }), quietly: false })
#                         jump bb5
#                         block bb5:
#                             Call(Call { _meta: Meta { node_index: NodeIndex(6), range: 18..24 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(7), range: 18..22 }, name: ExprName(ExprName { node_index: NodeIndex(7), range: 18..22, id: Name("body"), ctx: Load }) }), args: [], keywords: [] })
#                             jump bb1

# while_else

while cond:
    body()
else:
    done()

# ==

# function _dp_module_init():
#     function_id: 0
#     block bb1:
#         if_term Load(Load { _meta: Meta { node_index: NodeIndex(3), range: 7..11 }, name: ExprName(ExprName { node_index: NodeIndex(3), range: 7..11, id: Name("cond"), ctx: Load }) }):
#             then:
#                 block bb3:
#                     Call(Call { _meta: Meta { node_index: NodeIndex(5), range: 17..23 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(6), range: 17..21 }, name: ExprName(ExprName { node_index: NodeIndex(6), range: 17..21, id: Name("body"), ctx: Load }) }), args: [], keywords: [] })
#                     jump bb1
#             else:
#                 block bb2:
#                     Call(Call { _meta: Meta { node_index: NodeIndex(8), range: 34..40 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(9), range: 34..38 }, name: ExprName(ExprName { node_index: NodeIndex(9), range: 34..38, id: Name("done"), ctx: Load }) }), args: [], keywords: [] })
#                     return Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) })

# with_as

with cm as x:
    body()

# ==

# function _dp_module_init():
#     function_id: 0
#     block bb4:
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..15 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..15, id: Name("_dp_with_exit_1"), ctx: Load }), value: Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 78..143 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 78..110 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 78..110, value: "contextmanager_get_exit" }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(3), range: 6..8 }, name: ExprName(ExprName { node_index: NodeIndex(3), range: 6..8, id: Name("cm"), ctx: Load }) }))], keywords: [] }) })
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..1 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..1, id: Name("x"), ctx: Load }), value: Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 0..57 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..29 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..29, value: "contextmanager_enter" }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(3), range: 6..8 }, name: ExprName(ExprName { node_index: NodeIndex(3), range: 6..8, id: Name("cm"), ctx: Load }) }))], keywords: [] }) })
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..13 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..13, id: Name("_dp_with_ok_2"), ctx: Load }), value: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "TRUE" }) }) })
#         jump bb13
#         block bb13:
#             Call(Call { _meta: Meta { node_index: NodeIndex(6), range: 19..25 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(7), range: 19..23 }, name: ExprName(ExprName { node_index: NodeIndex(7), range: 19..23, id: Name("body"), ctx: Load }) }), args: [], keywords: [] })
#             jump bb8
#             block bb8:
#                 jump bb5(AbruptKind(Fallthrough), None)
#                 block bb5(_dp_try_exc_0_0: Exception, _dp_try_abrupt_kind_0_1: AbruptKind, _dp_try_abrupt_payload_0_2: AbruptPayload):
#                     exc_param: _dp_try_exc_0_0
#                     if_term Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..13 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..13, id: Name("_dp_with_ok_2"), ctx: Load }) }):
#                         then:
#                             block bb7(_dp_try_exc_0_0: Exception):
#                                 exc_param: _dp_try_exc_0_0
#                                 Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 463..529 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 463..491 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 463..491, value: "contextmanager_exit" }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..15 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..15, id: Name("_dp_with_exit_1"), ctx: Load }) })), Positional(Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) }))], keywords: [] })
#                                 jump bb6
#                         else:
#                             jump bb6
#                     block bb6(_dp_try_exc_0_0: Exception):
#                         exc_param: _dp_try_exc_0_0
#                         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..15 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..15, id: Name("_dp_with_exit_1"), ctx: Load }), value: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) }) })
#                         jump bb1
#                         block bb1:
#                             branch_table Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..23 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..23, id: Name("_dp_try_abrupt_kind_0_1"), ctx: Load }) }) -> [bb0, bb2, bb3] default bb0
#                             block bb0:
#                                 return Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) })
#                             block bb2:
#                                 return Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..26 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..26, id: Name("_dp_try_abrupt_payload_0_2"), ctx: Load }) })
#                             block bb3:
#                                 raise Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..26 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..26, id: Name("_dp_try_abrupt_payload_0_2"), ctx: Load }) })
#     block bb9(_dp_try_exc_0_0: Exception):
#         exc_param: _dp_try_exc_0_0
#         jump bb5(AbruptKind(Exception), Name("_dp_try_exc_0_0"))
#     block bb10(_dp_try_exc_0_0: Exception):
#         exc_param: _dp_try_exc_0_0
#         if_term Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 0..84 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..26 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..26, value: "exception_matches" }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_try_exc_0_0"), ctx: Load }) })), Positional(Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 258..271 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 258..271, id: Name("BaseException"), ctx: Load }) }))], keywords: [] }):
#             then:
#                 jump bb11
#             else:
#                 jump bb12
#     block bb11(_dp_try_exc_0_0: Exception):
#         exc_param: _dp_try_exc_0_0
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..13 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..13, id: Name("_dp_with_ok_2"), ctx: Load }), value: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "FALSE" }) }) })
#         Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 318..408 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 318..346 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 318..346, value: "contextmanager_exit" }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..15 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..15, id: Name("_dp_with_exit_1"), ctx: Load }) })), Positional(Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_try_exc_0_0"), ctx: Load }) }))], keywords: [] })
#         jump bb8
#     block bb12(_dp_try_exc_0_0: Exception):
#         exc_param: _dp_try_exc_0_0
#         raise Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_try_exc_0_0"), ctx: Load }) })

# function_local_ann_assign


def inner():
    value: int = 1
    return value


# ==

# function inner():
#     function_id: 0
#     block bb1:
#         Store(Store { _meta: Meta { node_index: NodeIndex(5), range: 19..24 }, name: ExprName(ExprName { node_index: NodeIndex(5), range: 19..24, id: Name("value"), ctx: Store }), value: Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(4), range: 32..33 }, literal: NumberLiteral(CoreNumberLiteral { node_index: NodeIndex(4), range: 32..33, value: Int(1) }) }) })
#         return Load(Load { _meta: Meta { node_index: NodeIndex(7), range: 45..50 }, name: ExprName(ExprName { node_index: NodeIndex(7), range: 45..50, id: Name("value"), ctx: Load }) })

# function _dp_module_init():
#     function_id: 1
#     block bb1:
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..5 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..5, id: Name("inner"), ctx: Load }), value: MakeFunction(MakeFunction { _meta: Meta { node_index: NodeIndex(None), range: 0..200 }, function_id: FunctionId(0), kind: Function, param_defaults: Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 0..23 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..21 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..21, value: "tuple_values" }) }), args: [], keywords: [] }), annotate_fn: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) }) }) })
#         return Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) })

# comprehension_global

xs = [x for x in it]
ys = {x for x in it}
zs = {k: v for k, v in items}

# ==

# function _dp_listcomp_3(_dp_iter_2):
#     function_id: 0
#     display_name: <listcomp>
#     block bb3:
#         Store(Store { _meta: Meta { node_index: NodeIndex(5), range: 0..9 }, name: ExprName(ExprName { node_index: NodeIndex(5), range: 0..9, id: Name("_dp_tmp_1"), ctx: Load }), value: Call(Call { _meta: Meta { node_index: NodeIndex(4), range: 28..30 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(4), range: 28..30 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(4), range: 28..30, value: "list" }) }), args: [Positional(Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 0..23 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..21 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..21, value: "tuple_values" }) }), args: [], keywords: [] }))], keywords: [] }) })
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..12 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..12, id: Name("_dp_iter_0_0"), ctx: Load }), value: Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 28..74 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 28..41 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 28..41, value: "iter" }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(7), range: 0..10 }, name: ExprName(ExprName { node_index: NodeIndex(7), range: 0..10, id: Name("_dp_iter_2"), ctx: Load }) }))], keywords: [] }) })
#         jump bb1
#         block bb1:
#             Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..11 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..11, id: Name("_dp_tmp_0_1"), ctx: Load }), value: Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 27..81 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 27..52 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 27..52, value: "next_or_sentinel" }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..12 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..12, id: Name("_dp_iter_0_0"), ctx: Load }) }))], keywords: [] }) })
#             if_term BinOp(BinOp { _meta: Meta { node_index: NodeIndex(None), range: 0..54 }, kind: Is, left: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..11 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..11, id: Name("_dp_tmp_0_1"), ctx: Load }) }), right: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 32..54 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 32..54, value: "ITER_COMPLETE" }) }) }):
#                 then:
#                     block bb4:
#                         return Load(Load { _meta: Meta { node_index: NodeIndex(15), range: 0..9 }, name: ExprName(ExprName { node_index: NodeIndex(15), range: 0..9, id: Name("_dp_tmp_1"), ctx: Load }) })
#                 else:
#                     block bb2:
#                         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..11 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..11, id: Name("_dp_tmp_0_1"), ctx: Load }), value: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..11 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..11, id: Name("_dp_tmp_0_1"), ctx: Load }) }) })
#                         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..1 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..1, id: Name("x"), ctx: Load }), value: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..11 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..11, id: Name("_dp_tmp_0_1"), ctx: Load }) }) })
#                         Del(Del { _meta: Meta { node_index: NodeIndex(None), range: 0..11 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..11, id: Name("_dp_tmp_0_1"), ctx: Load }), quietly: false })
#                         jump bb5
#                         block bb5:
#                             Call(Call { _meta: Meta { node_index: NodeIndex(10), range: 0..64 }, func: GetAttr(GetAttr { _meta: Meta { node_index: NodeIndex(11), range: 0..34 }, value: Load(Load { _meta: Meta { node_index: NodeIndex(12), range: 0..9 }, name: ExprName(ExprName { node_index: NodeIndex(12), range: 0..9, id: Name("_dp_tmp_1"), ctx: Load }) }), attr: Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(11), range: 0..34 }, literal: StringLiteral(CoreStringLiteral { node_index: NodeIndex(11), range: 0..34, value: "append" }) }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(13), range: 7..8 }, name: ExprName(ExprName { node_index: NodeIndex(13), range: 7..8, id: Name("x"), ctx: Load }) }))], keywords: [] })
#                             jump bb1

# function _dp_setcomp_6(_dp_iter_5):
#     function_id: 1
#     display_name: <setcomp>
#     block bb3:
#         Store(Store { _meta: Meta { node_index: NodeIndex(25), range: 0..9 }, name: ExprName(ExprName { node_index: NodeIndex(25), range: 0..9, id: Name("_dp_tmp_4"), ctx: Load }), value: Call(Call { _meta: Meta { node_index: NodeIndex(23), range: 28..33 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(24), range: 28..31 }, name: ExprName(ExprName { node_index: NodeIndex(24), range: 28..31, id: Name("set"), ctx: Load }) }), args: [], keywords: [] }) })
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..12 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..12, id: Name("_dp_iter_1_0"), ctx: Load }), value: Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 28..74 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 28..41 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 28..41, value: "iter" }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(27), range: 0..10 }, name: ExprName(ExprName { node_index: NodeIndex(27), range: 0..10, id: Name("_dp_iter_5"), ctx: Load }) }))], keywords: [] }) })
#         jump bb1
#         block bb1:
#             Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..11 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..11, id: Name("_dp_tmp_1_1"), ctx: Load }), value: Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 27..81 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 27..52 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 27..52, value: "next_or_sentinel" }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..12 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..12, id: Name("_dp_iter_1_0"), ctx: Load }) }))], keywords: [] }) })
#             if_term BinOp(BinOp { _meta: Meta { node_index: NodeIndex(None), range: 0..54 }, kind: Is, left: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..11 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..11, id: Name("_dp_tmp_1_1"), ctx: Load }) }), right: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 32..54 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 32..54, value: "ITER_COMPLETE" }) }) }):
#                 then:
#                     block bb4:
#                         return Load(Load { _meta: Meta { node_index: NodeIndex(35), range: 0..9 }, name: ExprName(ExprName { node_index: NodeIndex(35), range: 0..9, id: Name("_dp_tmp_4"), ctx: Load }) })
#                 else:
#                     block bb2:
#                         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..11 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..11, id: Name("_dp_tmp_1_1"), ctx: Load }), value: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..11 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..11, id: Name("_dp_tmp_1_1"), ctx: Load }) }) })
#                         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..1 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..1, id: Name("x"), ctx: Load }), value: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..11 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..11, id: Name("_dp_tmp_1_1"), ctx: Load }) }) })
#                         Del(Del { _meta: Meta { node_index: NodeIndex(None), range: 0..11 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..11, id: Name("_dp_tmp_1_1"), ctx: Load }), quietly: false })
#                         jump bb5
#                         block bb5:
#                             Call(Call { _meta: Meta { node_index: NodeIndex(30), range: 0..61 }, func: GetAttr(GetAttr { _meta: Meta { node_index: NodeIndex(31), range: 0..31 }, value: Load(Load { _meta: Meta { node_index: NodeIndex(32), range: 0..9 }, name: ExprName(ExprName { node_index: NodeIndex(32), range: 0..9, id: Name("_dp_tmp_4"), ctx: Load }) }), attr: Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(31), range: 0..31 }, literal: StringLiteral(CoreStringLiteral { node_index: NodeIndex(31), range: 0..31, value: "add" }) }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(33), range: 28..29 }, name: ExprName(ExprName { node_index: NodeIndex(33), range: 28..29, id: Name("x"), ctx: Load }) }))], keywords: [] })
#                             jump bb1

# function _dp_dictcomp_11(_dp_iter_10):
#     function_id: 2
#     display_name: <dictcomp>
#     block bb3:
#         Store(Store { _meta: Meta { node_index: NodeIndex(44), range: 0..9 }, name: ExprName(ExprName { node_index: NodeIndex(44), range: 0..9, id: Name("_dp_tmp_7"), ctx: Load }), value: Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "dict" }) }), args: [], keywords: [] }) })
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..12 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..12, id: Name("_dp_iter_2_0"), ctx: Load }), value: Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 28..74 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 28..41 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 28..41, value: "iter" }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(46), range: 0..11 }, name: ExprName(ExprName { node_index: NodeIndex(46), range: 0..11, id: Name("_dp_iter_10"), ctx: Load }) }))], keywords: [] }) })
#         jump bb1
#         block bb1:
#             Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..11 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..11, id: Name("_dp_tmp_2_1"), ctx: Load }), value: Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 27..81 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 27..52 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 27..52, value: "next_or_sentinel" }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..12 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..12, id: Name("_dp_iter_2_0"), ctx: Load }) }))], keywords: [] }) })
#             if_term BinOp(BinOp { _meta: Meta { node_index: NodeIndex(None), range: 0..54 }, kind: Is, left: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..11 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..11, id: Name("_dp_tmp_2_1"), ctx: Load }) }), right: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 32..54 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 32..54, value: "ITER_COMPLETE" }) }) }):
#                 then:
#                     block bb4:
#                         return Load(Load { _meta: Meta { node_index: NodeIndex(62), range: 0..9 }, name: ExprName(ExprName { node_index: NodeIndex(62), range: 0..9, id: Name("_dp_tmp_7"), ctx: Load }) })
#                 else:
#                     block bb2:
#                         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..11 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..11, id: Name("_dp_tmp_2_1"), ctx: Load }), value: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..11 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..11, id: Name("_dp_tmp_2_1"), ctx: Load }) }) })
#                         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..11 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..11, id: Name("_dp_tmp_2_2"), ctx: Load }), value: Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 27..101 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 27..42 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 27..42, value: "unpack" }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..11 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..11, id: Name("_dp_tmp_2_1"), ctx: Load }) })), Positional(Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 0..23 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..21 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..21, value: "tuple_values" }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "TRUE" }) })), Positional(Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "TRUE" }) }))], keywords: [] }))], keywords: [] }) })
#                         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..1 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..1, id: Name("k"), ctx: Load }), value: GetItem(GetItem { _meta: Meta { node_index: NodeIndex(None), range: 0..57 }, value: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..11 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..11, id: Name("_dp_tmp_2_2"), ctx: Load }) }), index: Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(None), range: 0..1 }, literal: NumberLiteral(CoreNumberLiteral { node_index: NodeIndex(None), range: 0..1, value: Int(0) }) }) }) })
#                         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..1 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..1, id: Name("v"), ctx: Load }), value: GetItem(GetItem { _meta: Meta { node_index: NodeIndex(None), range: 0..57 }, value: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..11 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..11, id: Name("_dp_tmp_2_2"), ctx: Load }) }), index: Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(None), range: 0..1 }, literal: NumberLiteral(CoreNumberLiteral { node_index: NodeIndex(None), range: 0..1, value: Int(1) }) }) }) })
#                         Del(Del { _meta: Meta { node_index: NodeIndex(None), range: 0..11 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..11, id: Name("_dp_tmp_2_2"), ctx: Load }), quietly: false })
#                         Del(Del { _meta: Meta { node_index: NodeIndex(None), range: 0..11 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..11, id: Name("_dp_tmp_2_1"), ctx: Load }), quietly: false })
#                         jump bb5
#                         block bb5:
#                             Store(Store { _meta: Meta { node_index: NodeIndex(52), range: 0..18 }, name: ExprName(ExprName { node_index: NodeIndex(52), range: 0..18, id: Name("_dp_dictcomp_key_8"), ctx: Load }), value: Load(Load { _meta: Meta { node_index: NodeIndex(51), range: 49..50 }, name: ExprName(ExprName { node_index: NodeIndex(51), range: 49..50, id: Name("k"), ctx: Load }) }) })
#                             Store(Store { _meta: Meta { node_index: NodeIndex(55), range: 0..20 }, name: ExprName(ExprName { node_index: NodeIndex(55), range: 0..20, id: Name("_dp_dictcomp_value_9"), ctx: Load }), value: Load(Load { _meta: Meta { node_index: NodeIndex(54), range: 52..53 }, name: ExprName(ExprName { node_index: NodeIndex(54), range: 52..53, id: Name("v"), ctx: Load }) }) })
#                             Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_assign_value_12"), ctx: Store }), value: Load(Load { _meta: Meta { node_index: NodeIndex(57), range: 0..20 }, name: ExprName(ExprName { node_index: NodeIndex(57), range: 0..20, id: Name("_dp_dictcomp_value_9"), ctx: Load }) }) })
#                             Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_assign_obj_13"), ctx: Store }), value: Call(Call { _meta: Meta { node_index: NodeIndex(59), range: 0..9 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(59), range: 0..9 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(59), range: 0..9, value: "load_deleted_name" }) }), args: [Positional(Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(None), range: 0..11 }, literal: StringLiteral(CoreStringLiteral { node_index: NodeIndex(None), range: 0..11, value: "_dp_tmp_7" }) })), Positional(Load(Load { _meta: Meta { node_index: NodeIndex(59), range: 0..9 }, name: ExprName(ExprName { node_index: NodeIndex(59), range: 0..9, id: Name("_dp_tmp_7"), ctx: Load }) }))], keywords: [] }) })
#                             Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_assign_index_14"), ctx: Store }), value: Load(Load { _meta: Meta { node_index: NodeIndex(60), range: 0..18 }, name: ExprName(ExprName { node_index: NodeIndex(60), range: 0..18, id: Name("_dp_dictcomp_key_8"), ctx: Load }) }) })
#                             SetItem(SetItem { _meta: Meta { node_index: NodeIndex(58), range: 0..53 }, value: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_assign_obj_13"), ctx: Load }) }), index: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_assign_index_14"), ctx: Load }) }), replacement: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_assign_value_12"), ctx: Load }) }) })
#                             jump bb1

# function _dp_module_init():
#     function_id: 3
#     block bb1:
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..14 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..14, id: Name("_dp_listcomp_3"), ctx: Load }), value: MakeFunction(MakeFunction { _meta: Meta { node_index: NodeIndex(None), range: 0..200 }, function_id: FunctionId(0), kind: Function, param_defaults: Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 0..23 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..21 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..21, value: "tuple_values" }) }), args: [], keywords: [] }), annotate_fn: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) }) }) })
#         Store(Store { _meta: Meta { node_index: NodeIndex(20), range: 1..3 }, name: ExprName(ExprName { node_index: NodeIndex(20), range: 1..3, id: Name("xs"), ctx: Store }), value: Call(Call { _meta: Meta { node_index: NodeIndex(17), range: 6..21 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(18), range: 0..14 }, name: ExprName(ExprName { node_index: NodeIndex(18), range: 0..14, id: Name("_dp_listcomp_3"), ctx: Load }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(19), range: 18..20 }, name: ExprName(ExprName { node_index: NodeIndex(19), range: 18..20, id: Name("it"), ctx: Load }) }))], keywords: [] }) })
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..13 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..13, id: Name("_dp_setcomp_6"), ctx: Load }), value: MakeFunction(MakeFunction { _meta: Meta { node_index: NodeIndex(None), range: 0..200 }, function_id: FunctionId(1), kind: Function, param_defaults: Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 0..23 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..21 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..21, value: "tuple_values" }) }), args: [], keywords: [] }), annotate_fn: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) }) }) })
#         Store(Store { _meta: Meta { node_index: NodeIndex(40), range: 22..24 }, name: ExprName(ExprName { node_index: NodeIndex(40), range: 22..24, id: Name("ys"), ctx: Store }), value: Call(Call { _meta: Meta { node_index: NodeIndex(37), range: 27..42 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(38), range: 0..13 }, name: ExprName(ExprName { node_index: NodeIndex(38), range: 0..13, id: Name("_dp_setcomp_6"), ctx: Load }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(39), range: 39..41 }, name: ExprName(ExprName { node_index: NodeIndex(39), range: 39..41, id: Name("it"), ctx: Load }) }))], keywords: [] }) })
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..15 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..15, id: Name("_dp_dictcomp_11"), ctx: Load }), value: MakeFunction(MakeFunction { _meta: Meta { node_index: NodeIndex(None), range: 0..200 }, function_id: FunctionId(2), kind: Function, param_defaults: Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 0..23 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..21 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..21, value: "tuple_values" }) }), args: [], keywords: [] }), annotate_fn: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) }) }) })
#         Store(Store { _meta: Meta { node_index: NodeIndex(67), range: 43..45 }, name: ExprName(ExprName { node_index: NodeIndex(67), range: 43..45, id: Name("zs"), ctx: Store }), value: Call(Call { _meta: Meta { node_index: NodeIndex(64), range: 48..72 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(65), range: 0..15 }, name: ExprName(ExprName { node_index: NodeIndex(65), range: 0..15, id: Name("_dp_dictcomp_11"), ctx: Load }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(66), range: 66..71 }, name: ExprName(ExprName { node_index: NodeIndex(66), range: 66..71, id: Name("items"), ctx: Load }) }))], keywords: [] }) })
#         return Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) })

# comprehension_in_function


def f():
    return [x for x in it if x > 0]


# ==

# function f.<locals>._dp_listcomp_3(_dp_iter_2):
#     function_id: 0
#     display_name: <listcomp>
#     block bb3:
#         Store(Store { _meta: Meta { node_index: NodeIndex(6), range: 0..9 }, name: ExprName(ExprName { node_index: NodeIndex(6), range: 0..9, id: Name("_dp_tmp_1"), ctx: Load }), value: Call(Call { _meta: Meta { node_index: NodeIndex(5), range: 28..30 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(5), range: 28..30 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(5), range: 28..30, value: "list" }) }), args: [Positional(Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 0..23 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..21 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..21, value: "tuple_values" }) }), args: [], keywords: [] }))], keywords: [] }) })
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..12 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..12, id: Name("_dp_iter_0_0"), ctx: Load }), value: Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 28..74 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 28..41 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 28..41, value: "iter" }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(8), range: 0..10 }, name: ExprName(ExprName { node_index: NodeIndex(8), range: 0..10, id: Name("_dp_iter_2"), ctx: Load }) }))], keywords: [] }) })
#         jump bb1
#         block bb1:
#             Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..11 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..11, id: Name("_dp_tmp_0_1"), ctx: Load }), value: Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 27..81 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 27..52 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 27..52, value: "next_or_sentinel" }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..12 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..12, id: Name("_dp_iter_0_0"), ctx: Load }) }))], keywords: [] }) })
#             if_term BinOp(BinOp { _meta: Meta { node_index: NodeIndex(None), range: 0..54 }, kind: Is, left: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..11 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..11, id: Name("_dp_tmp_0_1"), ctx: Load }) }), right: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 32..54 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 32..54, value: "ITER_COMPLETE" }) }) }):
#                 then:
#                     block bb4:
#                         return Load(Load { _meta: Meta { node_index: NodeIndex(20), range: 0..9 }, name: ExprName(ExprName { node_index: NodeIndex(20), range: 0..9, id: Name("_dp_tmp_1"), ctx: Load }) })
#                 else:
#                     block bb2:
#                         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..11 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..11, id: Name("_dp_tmp_0_1"), ctx: Load }), value: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..11 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..11, id: Name("_dp_tmp_0_1"), ctx: Load }) }) })
#                         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..1 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..1, id: Name("x"), ctx: Load }), value: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..11 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..11, id: Name("_dp_tmp_0_1"), ctx: Load }) }) })
#                         Del(Del { _meta: Meta { node_index: NodeIndex(None), range: 0..11 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..11, id: Name("_dp_tmp_0_1"), ctx: Load }), quietly: false })
#                         jump bb5
#                         block bb5:
#                             if_term BinOp(BinOp { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, kind: Gt, left: Load(Load { _meta: Meta { node_index: NodeIndex(12), range: 40..41 }, name: ExprName(ExprName { node_index: NodeIndex(12), range: 40..41, id: Name("x"), ctx: Load }) }), right: Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(13), range: 44..45 }, literal: NumberLiteral(CoreNumberLiteral { node_index: NodeIndex(13), range: 44..45, value: Int(0) }) }) }):
#                                 then:
#                                     block bb6:
#                                         Call(Call { _meta: Meta { node_index: NodeIndex(15), range: 0..64 }, func: GetAttr(GetAttr { _meta: Meta { node_index: NodeIndex(16), range: 0..34 }, value: Load(Load { _meta: Meta { node_index: NodeIndex(17), range: 0..9 }, name: ExprName(ExprName { node_index: NodeIndex(17), range: 0..9, id: Name("_dp_tmp_1"), ctx: Load }) }), attr: Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(16), range: 0..34 }, literal: StringLiteral(CoreStringLiteral { node_index: NodeIndex(16), range: 0..34, value: "append" }) }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(18), range: 23..24 }, name: ExprName(ExprName { node_index: NodeIndex(18), range: 23..24, id: Name("x"), ctx: Load }) }))], keywords: [] })
#                                         jump bb1
#                                 else:
#                                     jump bb1

# function f():
#     function_id: 1
#     block bb1:
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..14 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..14, id: Name("_dp_listcomp_3"), ctx: Load }), value: MakeFunction(MakeFunction { _meta: Meta { node_index: NodeIndex(None), range: 0..200 }, function_id: FunctionId(0), kind: Function, param_defaults: Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 0..23 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..21 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..21, value: "tuple_values" }) }), args: [], keywords: [] }), annotate_fn: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) }) }) })
#         return Call(Call { _meta: Meta { node_index: NodeIndex(22), range: 22..46 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(23), range: 0..14 }, name: ExprName(ExprName { node_index: NodeIndex(23), range: 0..14, id: Name("_dp_listcomp_3"), ctx: Load }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(24), range: 34..36 }, name: ExprName(ExprName { node_index: NodeIndex(24), range: 34..36, id: Name("it"), ctx: Load }) }))], keywords: [] })

# function _dp_module_init():
#     function_id: 2
#     block bb1:
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..1 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..1, id: Name("f"), ctx: Load }), value: MakeFunction(MakeFunction { _meta: Meta { node_index: NodeIndex(None), range: 0..200 }, function_id: FunctionId(1), kind: Function, param_defaults: Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 0..23 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..21 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..21, value: "tuple_values" }) }), args: [], keywords: [] }), annotate_fn: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) }) }) })
#         return Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) })

# comprehension_in_class_body


class C:
    xs = [x for x in it]


# ==

# function C._dp_listcomp_3(_dp_iter_2):
#     function_id: 0
#     display_name: <listcomp>
#     block bb3:
#         Store(Store { _meta: Meta { node_index: NodeIndex(6), range: 0..9 }, name: ExprName(ExprName { node_index: NodeIndex(6), range: 0..9, id: Name("_dp_tmp_1"), ctx: Load }), value: Call(Call { _meta: Meta { node_index: NodeIndex(5), range: 28..30 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(5), range: 28..30 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(5), range: 28..30, value: "list" }) }), args: [Positional(Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 0..23 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..21 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..21, value: "tuple_values" }) }), args: [], keywords: [] }))], keywords: [] }) })
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..12 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..12, id: Name("_dp_iter_0_0"), ctx: Load }), value: Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 28..74 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 28..41 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 28..41, value: "iter" }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(8), range: 0..10 }, name: ExprName(ExprName { node_index: NodeIndex(8), range: 0..10, id: Name("_dp_iter_2"), ctx: Load }) }))], keywords: [] }) })
#         jump bb1
#         block bb1:
#             Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..11 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..11, id: Name("_dp_tmp_0_1"), ctx: Load }), value: Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 27..81 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 27..52 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 27..52, value: "next_or_sentinel" }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..12 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..12, id: Name("_dp_iter_0_0"), ctx: Load }) }))], keywords: [] }) })
#             if_term BinOp(BinOp { _meta: Meta { node_index: NodeIndex(None), range: 0..54 }, kind: Is, left: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..11 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..11, id: Name("_dp_tmp_0_1"), ctx: Load }) }), right: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 32..54 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 32..54, value: "ITER_COMPLETE" }) }) }):
#                 then:
#                     block bb4:
#                         return Load(Load { _meta: Meta { node_index: NodeIndex(16), range: 0..9 }, name: ExprName(ExprName { node_index: NodeIndex(16), range: 0..9, id: Name("_dp_tmp_1"), ctx: Load }) })
#                 else:
#                     block bb2:
#                         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..11 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..11, id: Name("_dp_tmp_0_1"), ctx: Load }), value: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..11 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..11, id: Name("_dp_tmp_0_1"), ctx: Load }) }) })
#                         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..1 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..1, id: Name("x"), ctx: Load }), value: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..11 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..11, id: Name("_dp_tmp_0_1"), ctx: Load }) }) })
#                         Del(Del { _meta: Meta { node_index: NodeIndex(None), range: 0..11 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..11, id: Name("_dp_tmp_0_1"), ctx: Load }), quietly: false })
#                         jump bb5
#                         block bb5:
#                             Call(Call { _meta: Meta { node_index: NodeIndex(11), range: 0..64 }, func: GetAttr(GetAttr { _meta: Meta { node_index: NodeIndex(12), range: 0..34 }, value: Load(Load { _meta: Meta { node_index: NodeIndex(13), range: 0..9 }, name: ExprName(ExprName { node_index: NodeIndex(13), range: 0..9, id: Name("_dp_tmp_1"), ctx: Load }) }), attr: Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(12), range: 0..34 }, literal: StringLiteral(CoreStringLiteral { node_index: NodeIndex(12), range: 0..34, value: "append" }) }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(14), range: 21..22 }, name: ExprName(ExprName { node_index: NodeIndex(14), range: 21..22, id: Name("x"), ctx: Load }) }))], keywords: [] })
#                             jump bb1

# function _dp_class_ns_C(_dp_class_ns, _dp_classcell_arg):
#     function_id: 1
#     block bb1:
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 88..101 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 88..101, id: Name("_dp_classcell"), ctx: Store }), value: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 104..121 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 104..121, id: Name("_dp_classcell_arg"), ctx: Load }) }) })
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_assign_value_4"), ctx: Store }), value: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 155..163 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 155..163, id: Name("__name__"), ctx: Load }) }) })
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_assign_obj_5"), ctx: Store }), value: Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 126..138 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 126..138 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 126..138, value: "load_deleted_name" }) }), args: [Positional(Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(None), range: 0..14 }, literal: StringLiteral(CoreStringLiteral { node_index: NodeIndex(None), range: 0..14, value: "_dp_class_ns" }) })), Positional(Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 126..138 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 126..138, id: Name("_dp_class_ns"), ctx: Load }) }))], keywords: [] }) })
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_assign_index_6"), ctx: Store }), value: Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(None), range: 139..151 }, literal: StringLiteral(CoreStringLiteral { node_index: NodeIndex(None), range: 139..151, value: "__module__" }) }) })
#         SetItem(SetItem { _meta: Meta { node_index: NodeIndex(None), range: 126..152 }, value: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_assign_obj_5"), ctx: Load }) }), index: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_assign_index_6"), ctx: Load }) }), replacement: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_assign_value_4"), ctx: Load }) }) })
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_assign_value_7"), ctx: Store }), value: Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(None), range: 0..3 }, literal: StringLiteral(CoreStringLiteral { node_index: NodeIndex(None), range: 0..3, value: "C" }) }) })
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_assign_obj_8"), ctx: Store }), value: Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 168..180 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 168..180 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 168..180, value: "load_deleted_name" }) }), args: [Positional(Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(None), range: 0..14 }, literal: StringLiteral(CoreStringLiteral { node_index: NodeIndex(None), range: 0..14, value: "_dp_class_ns" }) })), Positional(Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 168..180 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 168..180, id: Name("_dp_class_ns"), ctx: Load }) }))], keywords: [] }) })
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_assign_index_9"), ctx: Store }), value: Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(None), range: 181..195 }, literal: StringLiteral(CoreStringLiteral { node_index: NodeIndex(None), range: 181..195, value: "__qualname__" }) }) })
#         SetItem(SetItem { _meta: Meta { node_index: NodeIndex(None), range: 168..196 }, value: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_assign_obj_8"), ctx: Load }) }), index: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_assign_index_9"), ctx: Load }) }), replacement: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_assign_value_7"), ctx: Load }) }) })
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..14 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..14, id: Name("_dp_listcomp_3"), ctx: Load }), value: MakeFunction(MakeFunction { _meta: Meta { node_index: NodeIndex(None), range: 0..200 }, function_id: FunctionId(0), kind: Function, param_defaults: Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 0..23 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..21 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..21, value: "tuple_values" }) }), args: [], keywords: [] }), annotate_fn: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) }) }) })
#         Store(Store { _meta: Meta { node_index: NodeIndex(21), range: 15..17 }, name: ExprName(ExprName { node_index: NodeIndex(21), range: 15..17, id: Name("xs"), ctx: Store }), value: Call(Call { _meta: Meta { node_index: NodeIndex(18), range: 20..35 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(19), range: 0..14 }, name: ExprName(ExprName { node_index: NodeIndex(19), range: 0..14, id: Name("_dp_listcomp_3"), ctx: Load }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(20), range: 32..34 }, name: ExprName(ExprName { node_index: NodeIndex(20), range: 32..34, id: Name("it"), ctx: Load }) }))], keywords: [] }) })
#         return Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) })

# function _dp_define_class_C(_dp_class_ns_fn, _dp_class_ns_outer, _dp_prepare_dict):
#     function_id: 2
#     block bb1:
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 150..162 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 150..162, id: Name("_dp_class_ns"), ctx: Store }), value: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 165..183 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 165..183, id: Name("_dp_class_ns_outer"), ctx: Load }) }) })
#         return Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 242..507 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 242..263 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 242..263, value: "create_class" }) }), args: [Positional(Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(None), range: 0..3 }, literal: StringLiteral(CoreStringLiteral { node_index: NodeIndex(None), range: 0..3, value: "C" }) })), Positional(Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 316..331 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 316..331, id: Name("_dp_class_ns_fn"), ctx: Load }) })), Positional(Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 0..23 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..21 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..21, value: "tuple_values" }) }), args: [], keywords: [] })), Positional(Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 377..393 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 377..393, id: Name("_dp_prepare_dict"), ctx: Load }) })), Positional(Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "FALSE" }) })), Positional(Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(None), range: 0..1 }, literal: NumberLiteral(CoreNumberLiteral { node_index: NodeIndex(None), range: 0..1, value: Int(3) }) })), Positional(Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 0..23 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..21 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..21, value: "tuple_values" }) }), args: [], keywords: [] }))], keywords: [] })

# function _dp_module_init():
#     function_id: 3
#     block bb1:
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..14 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..14, id: Name("_dp_class_ns_C"), ctx: Load }), value: MakeFunction(MakeFunction { _meta: Meta { node_index: NodeIndex(None), range: 0..200 }, function_id: FunctionId(1), kind: Function, param_defaults: Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 0..23 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..21 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..21, value: "tuple_values" }) }), args: [], keywords: [] }), annotate_fn: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) }) }) })
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..18 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..18, id: Name("_dp_define_class_C"), ctx: Load }), value: MakeFunction(MakeFunction { _meta: Meta { node_index: NodeIndex(None), range: 0..200 }, function_id: FunctionId(2), kind: Function, param_defaults: Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 0..23 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..21 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..21, value: "tuple_values" }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) }))], keywords: [] }), annotate_fn: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) }) }) })
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..1 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..1, id: Name("C"), ctx: Load }), value: Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 0..109 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..18 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..18, id: Name("_dp_define_class_C"), ctx: Load }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..14 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..14, id: Name("_dp_class_ns_C"), ctx: Load }) })), Positional(Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 0..9 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..7 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..7, id: Name("globals"), ctx: Load }) }), args: [], keywords: [] }))], keywords: [] }) })
#         return Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) })

# with_multi

with a as x, b as y:
    body()

# ==

# function _dp_module_init():
#     function_id: 0
#     block bb4:
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..15 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..15, id: Name("_dp_with_exit_4"), ctx: Load }), value: Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 78..143 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 78..110 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 78..110, value: "contextmanager_get_exit" }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(3), range: 6..7 }, name: ExprName(ExprName { node_index: NodeIndex(3), range: 6..7, id: Name("a"), ctx: Load }) }))], keywords: [] }) })
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..1 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..1, id: Name("x"), ctx: Load }), value: Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 0..57 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..29 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..29, value: "contextmanager_enter" }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(3), range: 6..7 }, name: ExprName(ExprName { node_index: NodeIndex(3), range: 6..7, id: Name("a"), ctx: Load }) }))], keywords: [] }) })
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..13 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..13, id: Name("_dp_with_ok_5"), ctx: Load }), value: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "TRUE" }) }) })
#         jump bb16
#         block bb16:
#             Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..15 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..15, id: Name("_dp_with_exit_1"), ctx: Load }), value: Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 78..143 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 78..110 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 78..110, value: "contextmanager_get_exit" }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(5), range: 14..15 }, name: ExprName(ExprName { node_index: NodeIndex(5), range: 14..15, id: Name("b"), ctx: Load }) }))], keywords: [] }) })
#             Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..1 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..1, id: Name("y"), ctx: Load }), value: Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 0..57 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..29 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..29, value: "contextmanager_enter" }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(5), range: 14..15 }, name: ExprName(ExprName { node_index: NodeIndex(5), range: 14..15, id: Name("b"), ctx: Load }) }))], keywords: [] }) })
#             Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..13 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..13, id: Name("_dp_with_ok_2"), ctx: Load }), value: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "TRUE" }) }) })
#             jump bb25
#             block bb25:
#                 Call(Call { _meta: Meta { node_index: NodeIndex(8), range: 26..32 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(9), range: 26..30 }, name: ExprName(ExprName { node_index: NodeIndex(9), range: 26..30, id: Name("body"), ctx: Load }) }), args: [], keywords: [] })
#                 jump bb20
#                 block bb20:
#                     jump bb17(AbruptKind(Fallthrough), None)
#                     block bb17(_dp_try_exc_0_3: Exception, _dp_try_abrupt_kind_0_4: AbruptKind, _dp_try_abrupt_payload_0_5: AbruptPayload):
#                         exc_param: _dp_try_exc_0_3
#                         if_term Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..13 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..13, id: Name("_dp_with_ok_2"), ctx: Load }) }):
#                             then:
#                                 block bb19(_dp_try_exc_0_3: Exception):
#                                     exc_param: _dp_try_exc_0_3
#                                     Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 463..529 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 463..491 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 463..491, value: "contextmanager_exit" }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..15 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..15, id: Name("_dp_with_exit_1"), ctx: Load }) })), Positional(Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) }))], keywords: [] })
#                                     jump bb18
#                             else:
#                                 jump bb18
#                         block bb18(_dp_try_exc_0_3: Exception):
#                             exc_param: _dp_try_exc_0_3
#                             Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..15 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..15, id: Name("_dp_with_exit_1"), ctx: Load }), value: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) }) })
#                             jump bb13
#                             block bb13:
#                                 branch_table Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..23 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..23, id: Name("_dp_try_abrupt_kind_0_4"), ctx: Load }) }) -> [bb8, bb14, bb15] default bb8
#                                 block bb5(_dp_try_exc_0_0: Exception, _dp_try_abrupt_kind_0_1: AbruptKind, _dp_try_abrupt_payload_0_2: AbruptPayload):
#                                     exc_param: _dp_try_exc_0_0
#                                     if_term Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..13 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..13, id: Name("_dp_with_ok_5"), ctx: Load }) }):
#                                         then:
#                                             block bb7(_dp_try_exc_0_0: Exception):
#                                                 exc_param: _dp_try_exc_0_0
#                                                 Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 463..529 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 463..491 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 463..491, value: "contextmanager_exit" }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..15 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..15, id: Name("_dp_with_exit_4"), ctx: Load }) })), Positional(Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) }))], keywords: [] })
#                                                 jump bb6
#                                         else:
#                                             jump bb6
#                                     block bb6(_dp_try_exc_0_0: Exception):
#                                         exc_param: _dp_try_exc_0_0
#                                         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..15 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..15, id: Name("_dp_with_exit_4"), ctx: Load }), value: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) }) })
#                                         jump bb1
#                                         block bb1:
#                                             branch_table Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..23 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..23, id: Name("_dp_try_abrupt_kind_0_1"), ctx: Load }) }) -> [bb0, bb2, bb3] default bb0
#                                             block bb0:
#                                                 return Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) })
#                                             block bb2:
#                                                 return Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..26 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..26, id: Name("_dp_try_abrupt_payload_0_2"), ctx: Load }) })
#                                             block bb3:
#                                                 raise Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..26 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..26, id: Name("_dp_try_abrupt_payload_0_2"), ctx: Load }) })
#                                 block bb8:
#                                     jump bb5(AbruptKind(Fallthrough), None)
#                                 block bb14:
#                                     Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..26 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..26, id: Name("_dp_try_abrupt_payload_0_2"), ctx: Load }), value: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..26 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..26, id: Name("_dp_try_abrupt_payload_0_5"), ctx: Load }) }) })
#                                     jump bb5(AbruptKind(Return), Name("_dp_try_abrupt_payload_0_2"))
#                                 block bb15:
#                                     raise Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..26 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..26, id: Name("_dp_try_abrupt_payload_0_5"), ctx: Load }) })
#     block bb9(_dp_try_exc_0_0: Exception):
#         exc_param: _dp_try_exc_0_0
#         jump bb5(AbruptKind(Exception), Name("_dp_try_exc_0_0"))
#     block bb10(_dp_try_exc_0_0: Exception):
#         exc_param: _dp_try_exc_0_0
#         if_term Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 0..84 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..26 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..26, value: "exception_matches" }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_try_exc_0_0"), ctx: Load }) })), Positional(Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 258..271 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 258..271, id: Name("BaseException"), ctx: Load }) }))], keywords: [] }):
#             then:
#                 jump bb11
#             else:
#                 jump bb12
#     block bb11(_dp_try_exc_0_0: Exception):
#         exc_param: _dp_try_exc_0_0
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..13 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..13, id: Name("_dp_with_ok_5"), ctx: Load }), value: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "FALSE" }) }) })
#         Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 318..408 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 318..346 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 318..346, value: "contextmanager_exit" }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..15 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..15, id: Name("_dp_with_exit_4"), ctx: Load }) })), Positional(Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_try_exc_0_0"), ctx: Load }) }))], keywords: [] })
#         jump bb8
#     block bb12(_dp_try_exc_0_0: Exception):
#         exc_param: _dp_try_exc_0_0
#         raise Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_try_exc_0_0"), ctx: Load }) })
#     block bb21(_dp_try_exc_0_3: Exception):
#         exc_param: _dp_try_exc_0_3
#         jump bb17(AbruptKind(Exception), Name("_dp_try_exc_0_3"))
#     block bb22(_dp_try_exc_0_3: Exception):
#         exc_param: _dp_try_exc_0_3
#         if_term Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 0..84 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..26 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..26, value: "exception_matches" }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_try_exc_0_3"), ctx: Load }) })), Positional(Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 258..271 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 258..271, id: Name("BaseException"), ctx: Load }) }))], keywords: [] }):
#             then:
#                 jump bb23
#             else:
#                 jump bb24
#     block bb23(_dp_try_exc_0_3: Exception):
#         exc_param: _dp_try_exc_0_3
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..13 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..13, id: Name("_dp_with_ok_2"), ctx: Load }), value: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "FALSE" }) }) })
#         Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 318..408 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 318..346 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 318..346, value: "contextmanager_exit" }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..15 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..15, id: Name("_dp_with_exit_1"), ctx: Load }) })), Positional(Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_try_exc_0_3"), ctx: Load }) }))], keywords: [] })
#         jump bb20
#     block bb24(_dp_try_exc_0_3: Exception):
#         exc_param: _dp_try_exc_0_3
#         raise Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_try_exc_0_3"), ctx: Load }) })

# async_for


async def run():
    async for x in ait:
        body()


# ==

# coroutine run():
#     function_id: 0
#     block bb3:
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..12 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..12, id: Name("_dp_iter_0_0"), ctx: Load }), value: Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 28..75 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 28..42 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 28..42, value: "aiter" }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(4), range: 38..41 }, name: ExprName(ExprName { node_index: NodeIndex(4), range: 38..41, id: Name("ait"), ctx: Load }) }))], keywords: [] }) })
#         jump bb1
#         block bb1:
#             Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..10 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..10, id: Name("_dp_eval_1"), ctx: Load }), value: Await(CoreBlockPyAwait { _meta: Meta { node_index: NodeIndex(None), range: 27..88 }, value: Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 33..88 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 33..59 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 33..59, value: "anext_or_sentinel" }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..12 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..12, id: Name("_dp_iter_0_0"), ctx: Load }) }))], keywords: [] }) }) })
#             Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..11 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..11, id: Name("_dp_tmp_0_1"), ctx: Load }), value: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..10 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..10, id: Name("_dp_eval_1"), ctx: Load }) }) })
#             Del(Del { _meta: Meta { node_index: NodeIndex(None), range: 0..10 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..10, id: Name("_dp_eval_1"), ctx: Load }), quietly: false })
#             if_term BinOp(BinOp { _meta: Meta { node_index: NodeIndex(None), range: 0..54 }, kind: Is, left: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..11 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..11, id: Name("_dp_tmp_0_1"), ctx: Load }) }), right: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 32..54 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 32..54, value: "ITER_COMPLETE" }) }) }):
#                 then:
#                     block bb0:
#                         return Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) })
#                 else:
#                     block bb2:
#                         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..11 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..11, id: Name("_dp_tmp_0_1"), ctx: Load }), value: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..11 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..11, id: Name("_dp_tmp_0_1"), ctx: Load }) }) })
#                         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..1 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..1, id: Name("x"), ctx: Load }), value: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..11 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..11, id: Name("_dp_tmp_0_1"), ctx: Load }) }) })
#                         Del(Del { _meta: Meta { node_index: NodeIndex(None), range: 0..11 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..11, id: Name("_dp_tmp_0_1"), ctx: Load }), quietly: false })
#                         jump bb4
#                         block bb4:
#                             Call(Call { _meta: Meta { node_index: NodeIndex(7), range: 51..57 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(8), range: 51..55 }, name: ExprName(ExprName { node_index: NodeIndex(8), range: 51..55, id: Name("body"), ctx: Load }) }), args: [], keywords: [] })
#                             jump bb1

# function _dp_module_init():
#     function_id: 1
#     block bb1:
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..3 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..3, id: Name("run"), ctx: Load }), value: MakeFunction(MakeFunction { _meta: Meta { node_index: NodeIndex(None), range: 0..200 }, function_id: FunctionId(0), kind: Coroutine, param_defaults: Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 0..23 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..21 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..21, value: "tuple_values" }) }), args: [], keywords: [] }), annotate_fn: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) }) }) })
#         return Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) })

# async_with


async def run():
    async with cm as x:
        body()


# ==

# coroutine run():
#     function_id: 0
#     block bb4:
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..15 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..15, id: Name("_dp_with_exit_1"), ctx: Load }), value: Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 78..149 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 78..116 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 78..116, value: "asynccontextmanager_get_aexit" }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(4), range: 34..36 }, name: ExprName(ExprName { node_index: NodeIndex(4), range: 34..36, id: Name("cm"), ctx: Load }) }))], keywords: [] }) })
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..10 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..10, id: Name("_dp_eval_4"), ctx: Load }), value: Await(CoreBlockPyAwait { _meta: Meta { node_index: NodeIndex(None), range: 0..69 }, value: Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 6..69 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 6..41 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 6..41, value: "asynccontextmanager_aenter" }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(4), range: 34..36 }, name: ExprName(ExprName { node_index: NodeIndex(4), range: 34..36, id: Name("cm"), ctx: Load }) }))], keywords: [] }) }) })
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..1 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..1, id: Name("x"), ctx: Load }), value: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..10 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..10, id: Name("_dp_eval_4"), ctx: Load }) }) })
#         Del(Del { _meta: Meta { node_index: NodeIndex(None), range: 0..10 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..10, id: Name("_dp_eval_4"), ctx: Load }), quietly: false })
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..13 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..13, id: Name("_dp_with_ok_2"), ctx: Load }), value: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "TRUE" }) }) })
#         jump bb14
#         block bb14:
#             Call(Call { _meta: Meta { node_index: NodeIndex(7), range: 51..57 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(8), range: 51..55 }, name: ExprName(ExprName { node_index: NodeIndex(8), range: 51..55, id: Name("body"), ctx: Load }) }), args: [], keywords: [] })
#             jump bb8
#             block bb8:
#                 jump bb5(AbruptKind(Fallthrough), None)
#                 block bb5(_dp_try_exc_0_0: Exception, _dp_try_abrupt_kind_0_1: AbruptKind, _dp_try_abrupt_payload_0_2: AbruptPayload):
#                     exc_param: _dp_try_exc_0_0
#                     if_term Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..13 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..13, id: Name("_dp_with_ok_2"), ctx: Load }) }):
#                         then:
#                             block bb7(_dp_try_exc_0_0: Exception):
#                                 exc_param: _dp_try_exc_0_0
#                                 Await(CoreBlockPyAwait { _meta: Meta { node_index: NodeIndex(None), range: 618..695 }, value: Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 624..695 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 624..657 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 624..657, value: "asynccontextmanager_exit" }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..15 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..15, id: Name("_dp_with_exit_1"), ctx: Load }) })), Positional(Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) }))], keywords: [] }) })
#                                 jump bb6
#                         else:
#                             jump bb6
#                     block bb6(_dp_try_exc_0_0: Exception):
#                         exc_param: _dp_try_exc_0_0
#                         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..15 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..15, id: Name("_dp_with_exit_1"), ctx: Load }), value: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) }) })
#                         jump bb1
#                         block bb1:
#                             branch_table Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..23 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..23, id: Name("_dp_try_abrupt_kind_0_1"), ctx: Load }) }) -> [bb0, bb2, bb3] default bb0
#                             block bb0:
#                                 return Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) })
#                             block bb2:
#                                 return Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..26 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..26, id: Name("_dp_try_abrupt_payload_0_2"), ctx: Load }) })
#                             block bb3:
#                                 raise Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..26 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..26, id: Name("_dp_try_abrupt_payload_0_2"), ctx: Load }) })
#     block bb9(_dp_try_exc_0_0: Exception):
#         exc_param: _dp_try_exc_0_0
#         jump bb5(AbruptKind(Exception), Name("_dp_try_exc_0_0"))
#     block bb10(_dp_try_exc_0_0: Exception):
#         exc_param: _dp_try_exc_0_0
#         if_term Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 0..84 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..26 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..26, value: "exception_matches" }) }), args: [Positional(Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 27..55 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 27..53 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 27..53, value: "current_exception" }) }), args: [], keywords: [] })), Positional(Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 264..277 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 264..277, id: Name("BaseException"), ctx: Load }) }))], keywords: [] }):
#             then:
#                 jump bb11
#             else:
#                 jump bb13
#     block bb11(_dp_try_exc_0_0: Exception):
#         exc_param: _dp_try_exc_0_0
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..13 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..13, id: Name("_dp_with_ok_2"), ctx: Load }), value: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "FALSE" }) }) })
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..10 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..10, id: Name("_dp_eval_5"), ctx: Load }), value: Await(CoreBlockPyAwait { _meta: Meta { node_index: NodeIndex(None), range: 360..461 }, value: Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 366..461 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 366..399 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 366..399, value: "asynccontextmanager_exit" }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..15 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..15, id: Name("_dp_with_exit_1"), ctx: Load }) })), Positional(Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 432..460 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 432..458 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 432..458, value: "current_exception" }) }), args: [], keywords: [] }))], keywords: [] }) }) })
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..18 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..18, id: Name("_dp_with_reraise_3"), ctx: Load }), value: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..10 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..10, id: Name("_dp_eval_5"), ctx: Load }) }) })
#         Del(Del { _meta: Meta { node_index: NodeIndex(None), range: 0..10 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..10, id: Name("_dp_eval_5"), ctx: Load }), quietly: false })
#         if_term UnaryOp(UnaryOp { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, kind: Not, operand: BinOp(BinOp { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, kind: Is, left: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..18 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..18, id: Name("_dp_with_reraise_3"), ctx: Load }) }), right: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) }) }) }):
#             then:
#                 jump bb12
#             else:
#                 jump bb8
#     block bb12(_dp_try_exc_0_0: Exception):
#         exc_param: _dp_try_exc_0_0
#         raise Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..18 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..18, id: Name("_dp_with_reraise_3"), ctx: Load }) })
#     block bb13(_dp_try_exc_0_0: Exception):
#         exc_param: _dp_try_exc_0_0
#         raise

# function _dp_module_init():
#     function_id: 1
#     block bb1:
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..3 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..3, id: Name("run"), ctx: Load }), value: MakeFunction(MakeFunction { _meta: Meta { node_index: NodeIndex(None), range: 0..200 }, function_id: FunctionId(0), kind: Coroutine, param_defaults: Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 0..23 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..21 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..21, value: "tuple_values" }) }), args: [], keywords: [] }), annotate_fn: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) }) }) })
#         return Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) })

# match_simple

match value:
    case 1:
        one()
    case _:
        other()

# ==

# function _dp_module_init():
#     function_id: 0
#     block bb1:
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..11 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..11, id: Name("_dp_match_1"), ctx: Load }), value: Load(Load { _meta: Meta { node_index: NodeIndex(3), range: 7..12 }, name: ExprName(ExprName { node_index: NodeIndex(3), range: 7..12, id: Name("value"), ctx: Load }) }) })
#         if_term BinOp(BinOp { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, kind: Eq, left: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..11 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..11, id: Name("_dp_match_1"), ctx: Load }) }), right: Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(4), range: 23..24 }, literal: NumberLiteral(CoreNumberLiteral { node_index: NodeIndex(4), range: 23..24, value: Int(1) }) }) }):
#             then:
#                 block bb2:
#                     Call(Call { _meta: Meta { node_index: NodeIndex(6), range: 34..39 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(7), range: 34..37 }, name: ExprName(ExprName { node_index: NodeIndex(7), range: 34..37, id: Name("one"), ctx: Load }) }), args: [], keywords: [] })
#                     return Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) })
#             else:
#                 block bb3:
#                     Call(Call { _meta: Meta { node_index: NodeIndex(9), range: 60..67 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(10), range: 60..65 }, name: ExprName(ExprName { node_index: NodeIndex(10), range: 60..65, id: Name("other"), ctx: Load }) }), args: [], keywords: [] })
#                     return Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) })

# generator_yield


def gen():
    yield 1


# ==

# generator gen():
#     function_id: 0
#     block bb1:
#         Yield(CoreBlockPyYield { _meta: Meta { node_index: NodeIndex(4), range: 17..24 }, value: Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(5), range: 23..24 }, literal: NumberLiteral(CoreNumberLiteral { node_index: NodeIndex(5), range: 23..24, value: Int(1) }) }) })
#         return Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) })

# function _dp_module_init():
#     function_id: 1
#     block bb1:
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..3 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..3, id: Name("gen"), ctx: Load }), value: MakeFunction(MakeFunction { _meta: Meta { node_index: NodeIndex(None), range: 0..200 }, function_id: FunctionId(0), kind: Generator, param_defaults: Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 0..23 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..21 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..21, value: "tuple_values" }) }), args: [], keywords: [] }), annotate_fn: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) }) }) })
#         return Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) })

# yield_from


def gen():
    yield from it


# ==

# generator gen():
#     function_id: 0
#     block bb1:
#         YieldFrom(CoreBlockPyYieldFrom { _meta: Meta { node_index: NodeIndex(4), range: 17..30 }, value: Load(Load { _meta: Meta { node_index: NodeIndex(5), range: 28..30 }, name: ExprName(ExprName { node_index: NodeIndex(5), range: 28..30, id: Name("it"), ctx: Load }) }) })
#         return Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) })

# function _dp_module_init():
#     function_id: 1
#     block bb1:
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..3 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..3, id: Name("gen"), ctx: Load }), value: MakeFunction(MakeFunction { _meta: Meta { node_index: NodeIndex(None), range: 0..200 }, function_id: FunctionId(0), kind: Generator, param_defaults: Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 0..23 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..21 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..21, value: "tuple_values" }) }), args: [], keywords: [] }), annotate_fn: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) }) }) })
#         return Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) })

# with_exit_suppresses_exception

with Suppress():
    raise RuntimeError("boom")

# ==

# function _dp_module_init():
#     function_id: 0
#     block bb4:
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..9 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..9, id: Name("_dp_tmp_4"), ctx: Load }), value: Call(Call { _meta: Meta { node_index: NodeIndex(3), range: 6..16 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(4), range: 6..14 }, name: ExprName(ExprName { node_index: NodeIndex(4), range: 6..14, id: Name("Suppress"), ctx: Load }) }), args: [], keywords: [] }) })
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..15 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..15, id: Name("_dp_with_exit_1"), ctx: Load }), value: Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 78..143 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 78..110 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 78..110, value: "contextmanager_get_exit" }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..9 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..9, id: Name("_dp_tmp_4"), ctx: Load }) }))], keywords: [] }) })
#         Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 0..57 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..29 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..29, value: "contextmanager_enter" }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..9 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..9, id: Name("_dp_tmp_4"), ctx: Load }) }))], keywords: [] })
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..13 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..13, id: Name("_dp_with_ok_2"), ctx: Load }), value: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "TRUE" }) }) })
#         jump bb13
#         block bb13:
#             raise Call(Call { _meta: Meta { node_index: NodeIndex(6), range: 28..48 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(7), range: 28..40 }, name: ExprName(ExprName { node_index: NodeIndex(7), range: 28..40, id: Name("RuntimeError"), ctx: Load }) }), args: [Positional(Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(8), range: 41..47 }, literal: StringLiteral(CoreStringLiteral { node_index: NodeIndex(8), range: 41..47, value: "boom" }) }))], keywords: [] })
#     block bb0:
#         return Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) })
#     block bb1:
#         branch_table Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..23 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..23, id: Name("_dp_try_abrupt_kind_0_1"), ctx: Load }) }) -> [bb0, bb2, bb3] default bb0
#     block bb2:
#         return Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..26 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..26, id: Name("_dp_try_abrupt_payload_0_2"), ctx: Load }) })
#     block bb3:
#         raise Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..26 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..26, id: Name("_dp_try_abrupt_payload_0_2"), ctx: Load }) })
#     block bb5(_dp_try_exc_0_0: Exception, _dp_try_abrupt_kind_0_1: AbruptKind, _dp_try_abrupt_payload_0_2: AbruptPayload):
#         exc_param: _dp_try_exc_0_0
#         if_term Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..13 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..13, id: Name("_dp_with_ok_2"), ctx: Load }) }):
#             then:
#                 jump bb7
#             else:
#                 jump bb6
#     block bb6(_dp_try_exc_0_0: Exception):
#         exc_param: _dp_try_exc_0_0
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..15 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..15, id: Name("_dp_with_exit_1"), ctx: Load }), value: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) }) })
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..9 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..9, id: Name("_dp_tmp_4"), ctx: Load }), value: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) }) })
#         jump bb1
#     block bb7(_dp_try_exc_0_0: Exception):
#         exc_param: _dp_try_exc_0_0
#         Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 463..529 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 463..491 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 463..491, value: "contextmanager_exit" }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..15 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..15, id: Name("_dp_with_exit_1"), ctx: Load }) })), Positional(Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) }))], keywords: [] })
#         jump bb6
#     block bb8:
#         jump bb5(AbruptKind(Fallthrough), None)
#     block bb9(_dp_try_exc_0_0: Exception):
#         exc_param: _dp_try_exc_0_0
#         jump bb5(AbruptKind(Exception), Name("_dp_try_exc_0_0"))
#     block bb10(_dp_try_exc_0_0: Exception):
#         exc_param: _dp_try_exc_0_0
#         if_term Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 0..84 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..26 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..26, value: "exception_matches" }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_try_exc_0_0"), ctx: Load }) })), Positional(Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 258..271 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 258..271, id: Name("BaseException"), ctx: Load }) }))], keywords: [] }):
#             then:
#                 jump bb11
#             else:
#                 jump bb12
#     block bb11(_dp_try_exc_0_0: Exception):
#         exc_param: _dp_try_exc_0_0
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..13 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..13, id: Name("_dp_with_ok_2"), ctx: Load }), value: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "FALSE" }) }) })
#         Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 318..408 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 318..346 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 318..346, value: "contextmanager_exit" }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..15 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..15, id: Name("_dp_with_exit_1"), ctx: Load }) })), Positional(Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_try_exc_0_0"), ctx: Load }) }))], keywords: [] })
#         jump bb8
#     block bb12(_dp_try_exc_0_0: Exception):
#         exc_param: _dp_try_exc_0_0
#         raise Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_try_exc_0_0"), ctx: Load }) })

# closure_cell_simple


def outer():
    x = 5

    def inner():
        return x

    return inner()


# ==

# function outer.<locals>.inner():
#     function_id: 0
#     block bb1:
#         return Load(Load { _meta: Meta { node_index: NodeIndex(8), range: 58..59 }, name: ExprName(ExprName { node_index: NodeIndex(8), range: 58..59, id: Name("x"), ctx: Load }) })

# function outer():
#     function_id: 1
#     block bb1:
#         Store(Store { _meta: Meta { node_index: NodeIndex(5), range: 19..20 }, name: ExprName(ExprName { node_index: NodeIndex(5), range: 19..20, id: Name("x"), ctx: Store }), value: Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(4), range: 23..24 }, literal: NumberLiteral(CoreNumberLiteral { node_index: NodeIndex(4), range: 23..24, value: Int(5) }) }) })
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..5 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..5, id: Name("inner"), ctx: Load }), value: MakeFunction(MakeFunction { _meta: Meta { node_index: NodeIndex(None), range: 0..200 }, function_id: FunctionId(0), kind: Function, param_defaults: Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 0..23 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..21 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..21, value: "tuple_values" }) }), args: [], keywords: [] }), annotate_fn: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) }) }) })
#         return Call(Call { _meta: Meta { node_index: NodeIndex(10), range: 72..79 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(11), range: 72..77 }, name: ExprName(ExprName { node_index: NodeIndex(11), range: 72..77, id: Name("inner"), ctx: Load }) }), args: [], keywords: [] })

# function _dp_module_init():
#     function_id: 2
#     block bb1:
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..5 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..5, id: Name("outer"), ctx: Load }), value: MakeFunction(MakeFunction { _meta: Meta { node_index: NodeIndex(None), range: 0..200 }, function_id: FunctionId(1), kind: Function, param_defaults: Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 0..23 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..21 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..21, value: "tuple_values" }) }), args: [], keywords: [] }), annotate_fn: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) }) }) })
#         return Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) })

# bb_if_else_function


def choose(a, b):
    total = a + b
    if total > 5:
        return a
    else:
        return b


# ==

# function choose(a, b):
#     function_id: 0
#     block bb1:
#         Store(Store { _meta: Meta { node_index: NodeIndex(7), range: 24..29 }, name: ExprName(ExprName { node_index: NodeIndex(7), range: 24..29, id: Name("total"), ctx: Store }), value: BinOp(BinOp { _meta: Meta { node_index: NodeIndex(4), range: 32..37 }, kind: Add, left: Load(Load { _meta: Meta { node_index: NodeIndex(5), range: 32..33 }, name: ExprName(ExprName { node_index: NodeIndex(5), range: 32..33, id: Name("a"), ctx: Load }) }), right: Load(Load { _meta: Meta { node_index: NodeIndex(6), range: 36..37 }, name: ExprName(ExprName { node_index: NodeIndex(6), range: 36..37, id: Name("b"), ctx: Load }) }) }) })
#         if_term BinOp(BinOp { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, kind: Gt, left: Load(Load { _meta: Meta { node_index: NodeIndex(10), range: 45..50 }, name: ExprName(ExprName { node_index: NodeIndex(10), range: 45..50, id: Name("total"), ctx: Load }) }), right: Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(11), range: 53..54 }, literal: NumberLiteral(CoreNumberLiteral { node_index: NodeIndex(11), range: 53..54, value: Int(5) }) }) }):
#             then:
#                 block bb2:
#                     return Load(Load { _meta: Meta { node_index: NodeIndex(13), range: 71..72 }, name: ExprName(ExprName { node_index: NodeIndex(13), range: 71..72, id: Name("a"), ctx: Load }) })
#             else:
#                 block bb3:
#                     return Load(Load { _meta: Meta { node_index: NodeIndex(15), range: 98..99 }, name: ExprName(ExprName { node_index: NodeIndex(15), range: 98..99, id: Name("b"), ctx: Load }) })

# function _dp_module_init():
#     function_id: 1
#     block bb1:
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..6 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..6, id: Name("choose"), ctx: Load }), value: MakeFunction(MakeFunction { _meta: Meta { node_index: NodeIndex(None), range: 0..200 }, function_id: FunctionId(0), kind: Function, param_defaults: Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 0..23 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..21 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..21, value: "tuple_values" }) }), args: [], keywords: [] }), annotate_fn: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) }) }) })
#         return Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) })

# closure_cell_nonlocal


def outer():
    x = 5

    def inner():
        nonlocal x
        x = 2
        return x

    return inner()


# ==

# function outer.<locals>.inner():
#     function_id: 0
#     block bb1:
#         Store(Store { _meta: Meta { node_index: NodeIndex(10), range: 70..71 }, name: ExprName(ExprName { node_index: NodeIndex(10), range: 70..71, id: Name("x"), ctx: Store }), value: Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(9), range: 74..75 }, literal: NumberLiteral(CoreNumberLiteral { node_index: NodeIndex(9), range: 74..75, value: Int(2) }) }) })
#         return Load(Load { _meta: Meta { node_index: NodeIndex(12), range: 91..92 }, name: ExprName(ExprName { node_index: NodeIndex(12), range: 91..92, id: Name("x"), ctx: Load }) })

# function outer():
#     function_id: 1
#     block bb1:
#         Store(Store { _meta: Meta { node_index: NodeIndex(5), range: 19..20 }, name: ExprName(ExprName { node_index: NodeIndex(5), range: 19..20, id: Name("x"), ctx: Store }), value: Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(4), range: 23..24 }, literal: NumberLiteral(CoreNumberLiteral { node_index: NodeIndex(4), range: 23..24, value: Int(5) }) }) })
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..5 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..5, id: Name("inner"), ctx: Load }), value: MakeFunction(MakeFunction { _meta: Meta { node_index: NodeIndex(None), range: 0..200 }, function_id: FunctionId(0), kind: Function, param_defaults: Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 0..23 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..21 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..21, value: "tuple_values" }) }), args: [], keywords: [] }), annotate_fn: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) }) }) })
#         return Call(Call { _meta: Meta { node_index: NodeIndex(14), range: 105..112 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(15), range: 105..110 }, name: ExprName(ExprName { node_index: NodeIndex(15), range: 105..110, id: Name("inner"), ctx: Load }) }), args: [], keywords: [] })

# function _dp_module_init():
#     function_id: 2
#     block bb1:
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..5 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..5, id: Name("outer"), ctx: Load }), value: MakeFunction(MakeFunction { _meta: Meta { node_index: NodeIndex(None), range: 0..200 }, function_id: FunctionId(1), kind: Function, param_defaults: Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 0..23 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..21 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..21, value: "tuple_values" }) }), args: [], keywords: [] }), annotate_fn: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) }) }) })
#         return Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) })

# plain try / catch

try:
    print(1)
except Exception:
    print(2)

# ==

# function _dp_module_init():
#     function_id: 0
#     block bb1:
#         jump bb5
#         block bb5:
#             Call(Call { _meta: Meta { node_index: NodeIndex(4), range: 10..18 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(5), range: 10..15 }, name: ExprName(ExprName { node_index: NodeIndex(5), range: 10..15, id: Name("print"), ctx: Load }) }), args: [Positional(Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(6), range: 16..17 }, literal: NumberLiteral(CoreNumberLiteral { node_index: NodeIndex(6), range: 16..17, value: Int(1) }) }))], keywords: [] })
#             return Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) })
#     block bb2(_dp_try_exc_0_0: Exception):
#         exc_param: _dp_try_exc_0_0
#         if_term Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 0..84 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..26 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..26, value: "exception_matches" }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_try_exc_0_0"), ctx: Load }) })), Positional(Load(Load { _meta: Meta { node_index: NodeIndex(7), range: 26..35 }, name: ExprName(ExprName { node_index: NodeIndex(7), range: 26..35, id: Name("Exception"), ctx: Load }) }))], keywords: [] }):
#             then:
#                 jump bb3
#             else:
#                 jump bb4
#     block bb3(_dp_try_exc_0_0: Exception):
#         exc_param: _dp_try_exc_0_0
#         Call(Call { _meta: Meta { node_index: NodeIndex(9), range: 41..49 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(10), range: 41..46 }, name: ExprName(ExprName { node_index: NodeIndex(10), range: 41..46, id: Name("print"), ctx: Load }) }), args: [Positional(Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(11), range: 47..48 }, literal: NumberLiteral(CoreNumberLiteral { node_index: NodeIndex(11), range: 47..48, value: Int(2) }) }))], keywords: [] })
#         return Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) })
#     block bb4(_dp_try_exc_0_0: Exception):
#         exc_param: _dp_try_exc_0_0
#         raise Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_try_exc_0_0"), ctx: Load }) })

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

# generator complicated(a):
#     function_id: 0
#     block bb3:
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..12 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..12, id: Name("_dp_iter_0_0"), ctx: Load }), value: Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 28..74 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 28..41 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 28..41, value: "iter" }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(4), range: 35..36 }, name: ExprName(ExprName { node_index: NodeIndex(4), range: 35..36, id: Name("a"), ctx: Load }) }))], keywords: [] }) })
#         jump bb1
#         block bb1:
#             Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..11 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..11, id: Name("_dp_tmp_0_1"), ctx: Load }), value: Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 27..81 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 27..52 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 27..52, value: "next_or_sentinel" }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..12 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..12, id: Name("_dp_iter_0_0"), ctx: Load }) }))], keywords: [] }) })
#             if_term BinOp(BinOp { _meta: Meta { node_index: NodeIndex(None), range: 0..54 }, kind: Is, left: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..11 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..11, id: Name("_dp_tmp_0_1"), ctx: Load }) }), right: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 32..54 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 32..54, value: "ITER_COMPLETE" }) }) }):
#                 then:
#                     block bb4:
#                         Call(Call { _meta: Meta { node_index: NodeIndex(21), range: 163..180 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(22), range: 163..168 }, name: ExprName(ExprName { node_index: NodeIndex(22), range: 163..168, id: Name("print"), ctx: Load }) }), args: [Positional(Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(23), range: 169..179 }, literal: StringLiteral(CoreStringLiteral { node_index: NodeIndex(23), range: 169..179, value: "finsihed" }) }))], keywords: [] })
#                         return Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) })
#                 else:
#                     block bb2:
#                         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..11 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..11, id: Name("_dp_tmp_0_1"), ctx: Load }), value: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..11 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..11, id: Name("_dp_tmp_0_1"), ctx: Load }) }) })
#                         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..1 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..1, id: Name("i"), ctx: Load }), value: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..11 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..11, id: Name("_dp_tmp_0_1"), ctx: Load }) }) })
#                         Del(Del { _meta: Meta { node_index: NodeIndex(None), range: 0..11 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..11, id: Name("_dp_tmp_0_1"), ctx: Load }), quietly: false })
#                         jump bb5
#                         block bb5:
#                             jump bb9
#                             block bb9:
#                                 Store(Store { _meta: Meta { node_index: NodeIndex(11), range: 63..64 }, name: ExprName(ExprName { node_index: NodeIndex(11), range: 63..64, id: Name("j"), ctx: Store }), value: BinOp(BinOp { _meta: Meta { node_index: NodeIndex(8), range: 67..72 }, kind: Add, left: Load(Load { _meta: Meta { node_index: NodeIndex(9), range: 67..68 }, name: ExprName(ExprName { node_index: NodeIndex(9), range: 67..68, id: Name("i"), ctx: Load }) }), right: Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(10), range: 71..72 }, literal: NumberLiteral(CoreNumberLiteral { node_index: NodeIndex(10), range: 71..72, value: Int(1) }) }) }) })
#                                 Yield(CoreBlockPyYield { _meta: Meta { node_index: NodeIndex(13), range: 85..92 }, value: Load(Load { _meta: Meta { node_index: NodeIndex(14), range: 91..92 }, name: ExprName(ExprName { node_index: NodeIndex(14), range: 91..92, id: Name("j"), ctx: Load }) }) })
#                                 jump bb1
#     block bb6(_dp_try_exc_0_2: Exception):
#         exc_param: _dp_try_exc_0_2
#         if_term Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 0..84 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..26 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..26, value: "exception_matches" }) }), args: [Positional(Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 27..55 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 27..53 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 27..53, value: "current_exception" }) }), args: [], keywords: [] })), Positional(Load(Load { _meta: Meta { node_index: NodeIndex(15), range: 108..117 }, name: ExprName(ExprName { node_index: NodeIndex(15), range: 108..117, id: Name("Exception"), ctx: Load }) }))], keywords: [] }):
#             then:
#                 jump bb7
#             else:
#                 jump bb8
#     block bb7(_dp_try_exc_0_2: Exception):
#         exc_param: _dp_try_exc_0_2
#         Call(Call { _meta: Meta { node_index: NodeIndex(17), range: 131..144 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(18), range: 131..136 }, name: ExprName(ExprName { node_index: NodeIndex(18), range: 131..136, id: Name("print"), ctx: Load }) }), args: [Positional(Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(19), range: 137..143 }, literal: StringLiteral(CoreStringLiteral { node_index: NodeIndex(19), range: 137..143, value: "oops" }) }))], keywords: [] })
#         jump bb1
#     block bb8(_dp_try_exc_0_2: Exception):
#         exc_param: _dp_try_exc_0_2
#         raise

# function _dp_module_init():
#     function_id: 1
#     block bb1:
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..11 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..11, id: Name("complicated"), ctx: Load }), value: MakeFunction(MakeFunction { _meta: Meta { node_index: NodeIndex(None), range: 0..200 }, function_id: FunctionId(0), kind: Generator, param_defaults: Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 0..23 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..21 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..21, value: "tuple_values" }) }), args: [], keywords: [] }), annotate_fn: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) }) }) })
#         return Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) })

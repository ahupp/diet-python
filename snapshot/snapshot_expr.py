# subscript

x = a[b]

# ==

# function _dp_module_init():
#     function_id: 0
#     block bb1:
#         Store(Store { _meta: Meta { node_index: NodeIndex(6), range: 1..2 }, name: ExprName(ExprName { node_index: NodeIndex(6), range: 1..2, id: Name("x"), ctx: Store }), value: GetItem(GetItem { _meta: Meta { node_index: NodeIndex(3), range: 5..9 }, value: Load(Load { _meta: Meta { node_index: NodeIndex(4), range: 5..6 }, name: ExprName(ExprName { node_index: NodeIndex(4), range: 5..6, id: Name("a"), ctx: Load }) }), index: Load(Load { _meta: Meta { node_index: NodeIndex(5), range: 7..8 }, name: ExprName(ExprName { node_index: NodeIndex(5), range: 7..8, id: Name("b"), ctx: Load }) }) }) })
#         return Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) })

# subscript_slice

x = a[1:2:3]

# ==

# function _dp_module_init():
#     function_id: 0
#     block bb1:
#         Store(Store { _meta: Meta { node_index: NodeIndex(9), range: 1..2 }, name: ExprName(ExprName { node_index: NodeIndex(9), range: 1..2, id: Name("x"), ctx: Store }), value: GetItem(GetItem { _meta: Meta { node_index: NodeIndex(3), range: 5..13 }, value: Load(Load { _meta: Meta { node_index: NodeIndex(4), range: 5..6 }, name: ExprName(ExprName { node_index: NodeIndex(4), range: 5..6, id: Name("a"), ctx: Load }) }), index: Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 0..103 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..14 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..14, value: "slice" }) }), args: [Positional(Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(6), range: 7..8 }, literal: NumberLiteral(CoreNumberLiteral { node_index: NodeIndex(6), range: 7..8, value: Int(1) }) })), Positional(Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(7), range: 9..10 }, literal: NumberLiteral(CoreNumberLiteral { node_index: NodeIndex(7), range: 9..10, value: Int(2) }) })), Positional(Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(8), range: 11..12 }, literal: NumberLiteral(CoreNumberLiteral { node_index: NodeIndex(8), range: 11..12, value: Int(3) }) }))], keywords: [] }) }) })
#         return Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) })

# binary_add

x = a + b

# ==

# function _dp_module_init():
#     function_id: 0
#     block bb1:
#         Store(Store { _meta: Meta { node_index: NodeIndex(6), range: 1..2 }, name: ExprName(ExprName { node_index: NodeIndex(6), range: 1..2, id: Name("x"), ctx: Store }), value: BinOp(BinOp { _meta: Meta { node_index: NodeIndex(3), range: 5..10 }, kind: Add, left: Load(Load { _meta: Meta { node_index: NodeIndex(4), range: 5..6 }, name: ExprName(ExprName { node_index: NodeIndex(4), range: 5..6, id: Name("a"), ctx: Load }) }), right: Load(Load { _meta: Meta { node_index: NodeIndex(5), range: 9..10 }, name: ExprName(ExprName { node_index: NodeIndex(5), range: 9..10, id: Name("b"), ctx: Load }) }) }) })
#         return Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) })

# binary_bitwise_or

x = a | b

# ==

# function _dp_module_init():
#     function_id: 0
#     block bb1:
#         Store(Store { _meta: Meta { node_index: NodeIndex(6), range: 1..2 }, name: ExprName(ExprName { node_index: NodeIndex(6), range: 1..2, id: Name("x"), ctx: Store }), value: BinOp(BinOp { _meta: Meta { node_index: NodeIndex(3), range: 5..10 }, kind: Or, left: Load(Load { _meta: Meta { node_index: NodeIndex(4), range: 5..6 }, name: ExprName(ExprName { node_index: NodeIndex(4), range: 5..6, id: Name("a"), ctx: Load }) }), right: Load(Load { _meta: Meta { node_index: NodeIndex(5), range: 9..10 }, name: ExprName(ExprName { node_index: NodeIndex(5), range: 9..10, id: Name("b"), ctx: Load }) }) }) })
#         return Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) })

# unary_neg

x = -a

# ==

# function _dp_module_init():
#     function_id: 0
#     block bb1:
#         Store(Store { _meta: Meta { node_index: NodeIndex(5), range: 1..2 }, name: ExprName(ExprName { node_index: NodeIndex(5), range: 1..2, id: Name("x"), ctx: Store }), value: UnaryOp(UnaryOp { _meta: Meta { node_index: NodeIndex(3), range: 5..7 }, kind: Neg, operand: Load(Load { _meta: Meta { node_index: NodeIndex(4), range: 6..7 }, name: ExprName(ExprName { node_index: NodeIndex(4), range: 6..7, id: Name("a"), ctx: Load }) }) }) })
#         return Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) })

# boolop_chain

x = a and b or c

# ==

# function _dp_module_init():
#     function_id: 0
#     block bb1:
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_target_2"), ctx: Store }), value: Load(Load { _meta: Meta { node_index: NodeIndex(5), range: 5..6 }, name: ExprName(ExprName { node_index: NodeIndex(5), range: 5..6, id: Name("a"), ctx: Load }) }) })
#         if_term Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..12 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..12, id: Name("_dp_target_2"), ctx: Load }) }):
#             then:
#                 block bb2:
#                     Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_target_2"), ctx: Store }), value: Load(Load { _meta: Meta { node_index: NodeIndex(6), range: 11..12 }, name: ExprName(ExprName { node_index: NodeIndex(6), range: 11..12, id: Name("b"), ctx: Load }) }) })
#                     jump bb4
#             else:
#                 block bb3:
#                     jump bb4
#         block bb4:
#             Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_target_1"), ctx: Store }), value: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..12 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..12, id: Name("_dp_target_2"), ctx: Load }) }) })
#             if_term UnaryOp(UnaryOp { _meta: Meta { node_index: NodeIndex(None), range: 0..31 }, kind: Not, operand: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..12 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..12, id: Name("_dp_target_1"), ctx: Load }) }) }):
#                 then:
#                     block bb5:
#                         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_target_1"), ctx: Store }), value: Load(Load { _meta: Meta { node_index: NodeIndex(7), range: 16..17 }, name: ExprName(ExprName { node_index: NodeIndex(7), range: 16..17, id: Name("c"), ctx: Load }) }) })
#                         jump bb7
#                 else:
#                     block bb6:
#                         jump bb7
#             block bb7:
#                 Store(Store { _meta: Meta { node_index: NodeIndex(8), range: 1..2 }, name: ExprName(ExprName { node_index: NodeIndex(8), range: 1..2, id: Name("x"), ctx: Store }), value: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..12 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..12, id: Name("_dp_target_1"), ctx: Load }) }) })
#                 return Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) })

# compare_lt

x = a < b

# ==

# function _dp_module_init():
#     function_id: 0
#     block bb1:
#         Store(Store { _meta: Meta { node_index: NodeIndex(6), range: 1..2 }, name: ExprName(ExprName { node_index: NodeIndex(6), range: 1..2, id: Name("x"), ctx: Store }), value: BinOp(BinOp { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, kind: Lt, left: Load(Load { _meta: Meta { node_index: NodeIndex(4), range: 5..6 }, name: ExprName(ExprName { node_index: NodeIndex(4), range: 5..6, id: Name("a"), ctx: Load }) }), right: Load(Load { _meta: Meta { node_index: NodeIndex(5), range: 9..10 }, name: ExprName(ExprName { node_index: NodeIndex(5), range: 9..10, id: Name("b"), ctx: Load }) }) }) })
#         return Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) })

# compare_chain

x = a < b < c

# ==

# function _dp_module_init():
#     function_id: 0
#     block bb1:
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_compare_1"), ctx: Store }), value: Load(Load { _meta: Meta { node_index: NodeIndex(4), range: 5..6 }, name: ExprName(ExprName { node_index: NodeIndex(4), range: 5..6, id: Name("a"), ctx: Load }) }) })
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_compare_3"), ctx: Store }), value: Load(Load { _meta: Meta { node_index: NodeIndex(5), range: 9..10 }, name: ExprName(ExprName { node_index: NodeIndex(5), range: 9..10, id: Name("b"), ctx: Load }) }) })
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_target_2"), ctx: Store }), value: BinOp(BinOp { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, kind: Lt, left: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..13 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..13, id: Name("_dp_compare_1"), ctx: Load }) }), right: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..13 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..13, id: Name("_dp_compare_3"), ctx: Load }) }) }) })
#         if_term Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..12 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..12, id: Name("_dp_target_2"), ctx: Load }) }):
#             then:
#                 block bb2:
#                     Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_target_2"), ctx: Store }), value: BinOp(BinOp { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, kind: Lt, left: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..13 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..13, id: Name("_dp_compare_3"), ctx: Load }) }), right: Load(Load { _meta: Meta { node_index: NodeIndex(6), range: 13..14 }, name: ExprName(ExprName { node_index: NodeIndex(6), range: 13..14, id: Name("c"), ctx: Load }) }) }) })
#                     jump bb4
#             else:
#                 block bb3:
#                     jump bb4
#         block bb4:
#             Store(Store { _meta: Meta { node_index: NodeIndex(7), range: 1..2 }, name: ExprName(ExprName { node_index: NodeIndex(7), range: 1..2, id: Name("x"), ctx: Store }), value: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..12 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..12, id: Name("_dp_target_2"), ctx: Load }) }) })
#             return Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) })

# compare_not_in

x = a not in b

# ==

# function _dp_module_init():
#     function_id: 0
#     block bb1:
#         Store(Store { _meta: Meta { node_index: NodeIndex(6), range: 1..2 }, name: ExprName(ExprName { node_index: NodeIndex(6), range: 1..2, id: Name("x"), ctx: Store }), value: UnaryOp(UnaryOp { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, kind: Not, operand: BinOp(BinOp { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, kind: Contains, left: Load(Load { _meta: Meta { node_index: NodeIndex(5), range: 14..15 }, name: ExprName(ExprName { node_index: NodeIndex(5), range: 14..15, id: Name("b"), ctx: Load }) }), right: Load(Load { _meta: Meta { node_index: NodeIndex(4), range: 5..6 }, name: ExprName(ExprName { node_index: NodeIndex(4), range: 5..6, id: Name("a"), ctx: Load }) }) }) }) })
#         return Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) })

# if_expr

x = a if cond else b

# ==

# function _dp_module_init():
#     function_id: 0
#     block bb1:
#         if_term Load(Load { _meta: Meta { node_index: NodeIndex(4), range: 10..14 }, name: ExprName(ExprName { node_index: NodeIndex(4), range: 10..14, id: Name("cond"), ctx: Load }) }):
#             then:
#                 block bb2:
#                     Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_tmp_1"), ctx: Store }), value: Load(Load { _meta: Meta { node_index: NodeIndex(5), range: 5..6 }, name: ExprName(ExprName { node_index: NodeIndex(5), range: 5..6, id: Name("a"), ctx: Load }) }) })
#                     jump bb4
#             else:
#                 block bb3:
#                     Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_tmp_1"), ctx: Store }), value: Load(Load { _meta: Meta { node_index: NodeIndex(6), range: 20..21 }, name: ExprName(ExprName { node_index: NodeIndex(6), range: 20..21, id: Name("b"), ctx: Load }) }) })
#                     jump bb4
#         block bb4:
#             Store(Store { _meta: Meta { node_index: NodeIndex(7), range: 1..2 }, name: ExprName(ExprName { node_index: NodeIndex(7), range: 1..2, id: Name("x"), ctx: Store }), value: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..9 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..9, id: Name("_dp_tmp_1"), ctx: Load }) }) })
#             return Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) })

# named_expr

x = (y := f())

# ==

# function _dp_module_init():
#     function_id: 0
#     block bb1:
#         Store(Store { _meta: Meta { node_index: NodeIndex(6), range: 6..7 }, name: ExprName(ExprName { node_index: NodeIndex(6), range: 6..7, id: Name("y"), ctx: Store }), value: Call(Call { _meta: Meta { node_index: NodeIndex(4), range: 11..14 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(5), range: 11..12 }, name: ExprName(ExprName { node_index: NodeIndex(5), range: 11..12, id: Name("f"), ctx: Load }) }), args: [], keywords: [] }) })
#         Store(Store { _meta: Meta { node_index: NodeIndex(7), range: 1..2 }, name: ExprName(ExprName { node_index: NodeIndex(7), range: 1..2, id: Name("x"), ctx: Store }), value: Load(Load { _meta: Meta { node_index: NodeIndex(6), range: 6..7 }, name: ExprName(ExprName { node_index: NodeIndex(6), range: 6..7, id: Name("y"), ctx: Load }) }) })
#         return Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) })

# lambda_simple

x = lambda y: y + 1

# ==

# function <lambda>(y):
#     function_id: 0
#     display_name: <lambda>
#     block bb1:
#         return BinOp(BinOp { _meta: Meta { node_index: NodeIndex(4), range: 15..20 }, kind: Add, left: Load(Load { _meta: Meta { node_index: NodeIndex(5), range: 15..16 }, name: ExprName(ExprName { node_index: NodeIndex(5), range: 15..16, id: Name("y"), ctx: Load }) }), right: Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(6), range: 19..20 }, literal: NumberLiteral(CoreNumberLiteral { node_index: NodeIndex(6), range: 19..20, value: Int(1) }) }) })

# function _dp_module_init():
#     function_id: 1
#     block bb1:
#         Store(Store { _meta: Meta { node_index: NodeIndex(7), range: 1..2 }, name: ExprName(ExprName { node_index: NodeIndex(7), range: 1..2, id: Name("x"), ctx: Store }), value: MakeFunction(MakeFunction { _meta: Meta { node_index: NodeIndex(None), range: 0..200 }, function_id: FunctionId(0), kind: Function, param_defaults: Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 0..23 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..21 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..21, value: "tuple_values" }) }), args: [], keywords: [] }), annotate_fn: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) }) }) })
#         return Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) })

# generator_expr

x = (i for i in it)

# ==

# generator <genexpr>(_dp_iter_2):
#     function_id: 0
#     display_name: <genexpr>
#     block bb2:
#         Store(Store { _meta: Meta { node_index: NodeIndex(5), range: 0..10 }, name: ExprName(ExprName { node_index: NodeIndex(5), range: 0..10, id: Name("_dp_iter_3"), ctx: Load }), value: Load(Load { _meta: Meta { node_index: NodeIndex(4), range: 0..10 }, name: ExprName(ExprName { node_index: NodeIndex(4), range: 0..10, id: Name("_dp_iter_2"), ctx: Load }) }) })
#         jump bb1
#         block bb1:
#             if_term Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "TRUE" }) }):
#                 then:
#                     block bb3:
#                         Store(Store { _meta: Meta { node_index: NodeIndex(13), range: 0..9 }, name: ExprName(ExprName { node_index: NodeIndex(13), range: 0..9, id: Name("_dp_tmp_4"), ctx: Load }), value: Call(Call { _meta: Meta { node_index: NodeIndex(9), range: 112..169 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(10), range: 112..137 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(10), range: 112..137, value: "next_or_sentinel" }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(12), range: 0..10 }, name: ExprName(ExprName { node_index: NodeIndex(12), range: 0..10, id: Name("_dp_iter_3"), ctx: Load }) }))], keywords: [] }) })
#                         if_term BinOp(BinOp { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, kind: Is, left: Load(Load { _meta: Meta { node_index: NodeIndex(16), range: 0..9 }, name: ExprName(ExprName { node_index: NodeIndex(16), range: 0..9, id: Name("_dp_tmp_4"), ctx: Load }) }), right: Load(Load { _meta: Meta { node_index: NodeIndex(17), range: 212..234 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(17), range: 212..234, value: "ITER_COMPLETE" }) }) }):
#                             then:
#                                 block bb4:
#                                     return Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) })
#                             else:
#                                 block bb5:
#                                     Store(Store { _meta: Meta { node_index: NodeIndex(22), range: 12..13 }, name: ExprName(ExprName { node_index: NodeIndex(22), range: 12..13, id: Name("i"), ctx: Store }), value: Load(Load { _meta: Meta { node_index: NodeIndex(21), range: 0..9 }, name: ExprName(ExprName { node_index: NodeIndex(21), range: 0..9, id: Name("_dp_tmp_4"), ctx: Load }) }) })
#                                     Yield(CoreBlockPyYield { _meta: Meta { node_index: NodeIndex(24), range: 0..34 }, value: Load(Load { _meta: Meta { node_index: NodeIndex(25), range: 6..7 }, name: ExprName(ExprName { node_index: NodeIndex(25), range: 6..7, id: Name("i"), ctx: Load }) }) })
#                                     jump bb1
#                 else:
#                     block bb0:
#                         return Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) })

# function _dp_module_init():
#     function_id: 1
#     block bb1:
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..13 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..13, id: Name("_dp_genexpr_1"), ctx: Load }), value: MakeFunction(MakeFunction { _meta: Meta { node_index: NodeIndex(None), range: 0..200 }, function_id: FunctionId(0), kind: Generator, param_defaults: Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 0..23 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..21 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..21, value: "tuple_values" }) }), args: [], keywords: [] }), annotate_fn: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) }) }) })
#         Store(Store { _meta: Meta { node_index: NodeIndex(33), range: 1..2 }, name: ExprName(ExprName { node_index: NodeIndex(33), range: 1..2, id: Name("x"), ctx: Store }), value: Call(Call { _meta: Meta { node_index: NodeIndex(27), range: 5..20 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(28), range: 0..13 }, name: ExprName(ExprName { node_index: NodeIndex(28), range: 0..13, id: Name("_dp_genexpr_1"), ctx: Load }) }), args: [Positional(Call(Call { _meta: Meta { node_index: NodeIndex(29), range: 0..42 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(30), range: 0..13 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(30), range: 0..13, value: "iter" }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(32), range: 17..19 }, name: ExprName(ExprName { node_index: NodeIndex(32), range: 17..19, id: Name("it"), ctx: Load }) }))], keywords: [] }))], keywords: [] }) })
#         return Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) })

# list_literal

x = [a, b]

# ==

# function _dp_module_init():
#     function_id: 0
#     block bb1:
#         Store(Store { _meta: Meta { node_index: NodeIndex(6), range: 1..2 }, name: ExprName(ExprName { node_index: NodeIndex(6), range: 1..2, id: Name("x"), ctx: Store }), value: Call(Call { _meta: Meta { node_index: NodeIndex(3), range: 5..11 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(3), range: 5..11 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(3), range: 5..11, value: "list" }) }), args: [Positional(Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 0..23 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..21 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..21, value: "tuple_values" }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(4), range: 6..7 }, name: ExprName(ExprName { node_index: NodeIndex(4), range: 6..7, id: Name("a"), ctx: Load }) })), Positional(Load(Load { _meta: Meta { node_index: NodeIndex(5), range: 9..10 }, name: ExprName(ExprName { node_index: NodeIndex(5), range: 9..10, id: Name("b"), ctx: Load }) }))], keywords: [] }))], keywords: [] }) })
#         return Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) })

# list_literal_splat

x = [a, *b]

# ==

# function _dp_module_init():
#     function_id: 0
#     block bb1:
#         Store(Store { _meta: Meta { node_index: NodeIndex(7), range: 1..2 }, name: ExprName(ExprName { node_index: NodeIndex(7), range: 1..2, id: Name("x"), ctx: Store }), value: Call(Call { _meta: Meta { node_index: NodeIndex(3), range: 5..12 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(3), range: 5..12 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(3), range: 5..12, value: "list" }) }), args: [Positional(BinOp(BinOp { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, kind: Add, left: Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "tuple_values" }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(4), range: 6..7 }, name: ExprName(ExprName { node_index: NodeIndex(4), range: 6..7, id: Name("a"), ctx: Load }) }))], keywords: [] }), right: Call(Call { _meta: Meta { node_index: NodeIndex(5), range: 9..11 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(5), range: 9..11 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(5), range: 9..11, value: "tuple_from_iter" }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(6), range: 10..11 }, name: ExprName(ExprName { node_index: NodeIndex(6), range: 10..11, id: Name("b"), ctx: Load }) }))], keywords: [] }) }))], keywords: [] }) })
#         return Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) })

# tuple_splat

x = (a, *b)

# ==

# function _dp_module_init():
#     function_id: 0
#     block bb1:
#         Store(Store { _meta: Meta { node_index: NodeIndex(7), range: 1..2 }, name: ExprName(ExprName { node_index: NodeIndex(7), range: 1..2, id: Name("x"), ctx: Store }), value: BinOp(BinOp { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, kind: Add, left: Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "tuple_values" }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(4), range: 6..7 }, name: ExprName(ExprName { node_index: NodeIndex(4), range: 6..7, id: Name("a"), ctx: Load }) }))], keywords: [] }), right: Call(Call { _meta: Meta { node_index: NodeIndex(5), range: 9..11 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(5), range: 9..11 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(5), range: 9..11, value: "tuple_from_iter" }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(6), range: 10..11 }, name: ExprName(ExprName { node_index: NodeIndex(6), range: 10..11, id: Name("b"), ctx: Load }) }))], keywords: [] }) }) })
#         return Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) })

# set_literal

x = {a, b}

# ==

# function _dp_module_init():
#     function_id: 0
#     block bb1:
#         Store(Store { _meta: Meta { node_index: NodeIndex(6), range: 1..2 }, name: ExprName(ExprName { node_index: NodeIndex(6), range: 1..2, id: Name("x"), ctx: Store }), value: Call(Call { _meta: Meta { node_index: NodeIndex(3), range: 5..11 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(3), range: 5..11 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(3), range: 5..11, value: "set" }) }), args: [Positional(Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 0..23 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..21 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..21, value: "tuple_values" }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(4), range: 6..7 }, name: ExprName(ExprName { node_index: NodeIndex(4), range: 6..7, id: Name("a"), ctx: Load }) })), Positional(Load(Load { _meta: Meta { node_index: NodeIndex(5), range: 9..10 }, name: ExprName(ExprName { node_index: NodeIndex(5), range: 9..10, id: Name("b"), ctx: Load }) }))], keywords: [] }))], keywords: [] }) })
#         return Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) })

# dict_literal

x = {"a": 1, "b": 2}

# ==

# function _dp_module_init():
#     function_id: 0
#     block bb1:
#         Store(Store { _meta: Meta { node_index: NodeIndex(8), range: 1..2 }, name: ExprName(ExprName { node_index: NodeIndex(8), range: 1..2, id: Name("x"), ctx: Store }), value: Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 0..43 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..13 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..13, value: "dict" }) }), args: [Positional(Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 0..23 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..21 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..21, value: "tuple_values" }) }), args: [Positional(Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 0..23 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..21 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..21, value: "tuple_values" }) }), args: [Positional(Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(4), range: 6..9 }, literal: StringLiteral(CoreStringLiteral { node_index: NodeIndex(4), range: 6..9, value: "a" }) })), Positional(Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(5), range: 11..12 }, literal: NumberLiteral(CoreNumberLiteral { node_index: NodeIndex(5), range: 11..12, value: Int(1) }) }))], keywords: [] })), Positional(Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 0..23 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..21 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..21, value: "tuple_values" }) }), args: [Positional(Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(6), range: 14..17 }, literal: StringLiteral(CoreStringLiteral { node_index: NodeIndex(6), range: 14..17, value: "b" }) })), Positional(Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(7), range: 19..20 }, literal: NumberLiteral(CoreNumberLiteral { node_index: NodeIndex(7), range: 19..20, value: Int(2) }) }))], keywords: [] }))], keywords: [] }))], keywords: [] }) })
#         return Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) })

# dict_literal_unpack

x = {"a": 1, **m, "b": 2}

# ==

# function _dp_module_init():
#     function_id: 0
#     block bb1:
#         Store(Store { _meta: Meta { node_index: NodeIndex(9), range: 1..2 }, name: ExprName(ExprName { node_index: NodeIndex(9), range: 1..2, id: Name("x"), ctx: Store }), value: BinOp(BinOp { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, kind: Or, left: BinOp(BinOp { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, kind: Or, left: Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 0..43 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..13 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..13, value: "dict" }) }), args: [Positional(Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 0..23 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..21 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..21, value: "tuple_values" }) }), args: [Positional(Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 0..23 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..21 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..21, value: "tuple_values" }) }), args: [Positional(Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(4), range: 6..9 }, literal: StringLiteral(CoreStringLiteral { node_index: NodeIndex(4), range: 6..9, value: "a" }) })), Positional(Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(5), range: 11..12 }, literal: NumberLiteral(CoreNumberLiteral { node_index: NodeIndex(5), range: 11..12, value: Int(1) }) }))], keywords: [] }))], keywords: [] }))], keywords: [] }), right: Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 0..45 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..13 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..13, value: "dict" }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(6), range: 16..17 }, name: ExprName(ExprName { node_index: NodeIndex(6), range: 16..17, id: Name("m"), ctx: Load }) }))], keywords: [] }) }), right: Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 0..43 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..13 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..13, value: "dict" }) }), args: [Positional(Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 0..23 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..21 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..21, value: "tuple_values" }) }), args: [Positional(Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 0..23 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..21 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..21, value: "tuple_values" }) }), args: [Positional(Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(7), range: 19..22 }, literal: StringLiteral(CoreStringLiteral { node_index: NodeIndex(7), range: 19..22, value: "b" }) })), Positional(Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(8), range: 24..25 }, literal: NumberLiteral(CoreNumberLiteral { node_index: NodeIndex(8), range: 24..25, value: Int(2) }) }))], keywords: [] }))], keywords: [] }))], keywords: [] }) }) })
#         return Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) })

# list_comp

x = [i for i in it]

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
#                         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..1 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..1, id: Name("i"), ctx: Load }), value: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..11 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..11, id: Name("_dp_tmp_0_1"), ctx: Load }) }) })
#                         Del(Del { _meta: Meta { node_index: NodeIndex(None), range: 0..11 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..11, id: Name("_dp_tmp_0_1"), ctx: Load }), quietly: false })
#                         jump bb5
#                         block bb5:
#                             Call(Call { _meta: Meta { node_index: NodeIndex(10), range: 0..64 }, func: GetAttr(GetAttr { _meta: Meta { node_index: NodeIndex(11), range: 0..34 }, value: Load(Load { _meta: Meta { node_index: NodeIndex(12), range: 0..9 }, name: ExprName(ExprName { node_index: NodeIndex(12), range: 0..9, id: Name("_dp_tmp_1"), ctx: Load }) }), attr: Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(11), range: 0..34 }, literal: StringLiteral(CoreStringLiteral { node_index: NodeIndex(11), range: 0..34, value: "append" }) }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(13), range: 6..7 }, name: ExprName(ExprName { node_index: NodeIndex(13), range: 6..7, id: Name("i"), ctx: Load }) }))], keywords: [] })
#                             jump bb1

# function _dp_module_init():
#     function_id: 1
#     block bb1:
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..14 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..14, id: Name("_dp_listcomp_3"), ctx: Load }), value: MakeFunction(MakeFunction { _meta: Meta { node_index: NodeIndex(None), range: 0..200 }, function_id: FunctionId(0), kind: Function, param_defaults: Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 0..23 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..21 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..21, value: "tuple_values" }) }), args: [], keywords: [] }), annotate_fn: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) }) }) })
#         Store(Store { _meta: Meta { node_index: NodeIndex(20), range: 1..2 }, name: ExprName(ExprName { node_index: NodeIndex(20), range: 1..2, id: Name("x"), ctx: Store }), value: Call(Call { _meta: Meta { node_index: NodeIndex(17), range: 5..20 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(18), range: 0..14 }, name: ExprName(ExprName { node_index: NodeIndex(18), range: 0..14, id: Name("_dp_listcomp_3"), ctx: Load }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(19), range: 17..19 }, name: ExprName(ExprName { node_index: NodeIndex(19), range: 17..19, id: Name("it"), ctx: Load }) }))], keywords: [] }) })
#         return Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) })

# set_comp

x = {i for i in it}

# ==

# function _dp_setcomp_3(_dp_iter_2):
#     function_id: 0
#     display_name: <setcomp>
#     block bb3:
#         Store(Store { _meta: Meta { node_index: NodeIndex(6), range: 0..9 }, name: ExprName(ExprName { node_index: NodeIndex(6), range: 0..9, id: Name("_dp_tmp_1"), ctx: Load }), value: Call(Call { _meta: Meta { node_index: NodeIndex(4), range: 28..33 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(5), range: 28..31 }, name: ExprName(ExprName { node_index: NodeIndex(5), range: 28..31, id: Name("set"), ctx: Load }) }), args: [], keywords: [] }) })
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
#                         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..1 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..1, id: Name("i"), ctx: Load }), value: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..11 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..11, id: Name("_dp_tmp_0_1"), ctx: Load }) }) })
#                         Del(Del { _meta: Meta { node_index: NodeIndex(None), range: 0..11 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..11, id: Name("_dp_tmp_0_1"), ctx: Load }), quietly: false })
#                         jump bb5
#                         block bb5:
#                             Call(Call { _meta: Meta { node_index: NodeIndex(11), range: 0..61 }, func: GetAttr(GetAttr { _meta: Meta { node_index: NodeIndex(12), range: 0..31 }, value: Load(Load { _meta: Meta { node_index: NodeIndex(13), range: 0..9 }, name: ExprName(ExprName { node_index: NodeIndex(13), range: 0..9, id: Name("_dp_tmp_1"), ctx: Load }) }), attr: Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(12), range: 0..31 }, literal: StringLiteral(CoreStringLiteral { node_index: NodeIndex(12), range: 0..31, value: "add" }) }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(14), range: 6..7 }, name: ExprName(ExprName { node_index: NodeIndex(14), range: 6..7, id: Name("i"), ctx: Load }) }))], keywords: [] })
#                             jump bb1

# function _dp_module_init():
#     function_id: 1
#     block bb1:
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..13 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..13, id: Name("_dp_setcomp_3"), ctx: Load }), value: MakeFunction(MakeFunction { _meta: Meta { node_index: NodeIndex(None), range: 0..200 }, function_id: FunctionId(0), kind: Function, param_defaults: Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 0..23 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..21 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..21, value: "tuple_values" }) }), args: [], keywords: [] }), annotate_fn: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) }) }) })
#         Store(Store { _meta: Meta { node_index: NodeIndex(21), range: 1..2 }, name: ExprName(ExprName { node_index: NodeIndex(21), range: 1..2, id: Name("x"), ctx: Store }), value: Call(Call { _meta: Meta { node_index: NodeIndex(18), range: 5..20 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(19), range: 0..13 }, name: ExprName(ExprName { node_index: NodeIndex(19), range: 0..13, id: Name("_dp_setcomp_3"), ctx: Load }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(20), range: 17..19 }, name: ExprName(ExprName { node_index: NodeIndex(20), range: 17..19, id: Name("it"), ctx: Load }) }))], keywords: [] }) })
#         return Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) })

# dict_comp

x = {k: v for k, v in it}

# ==

# function _dp_dictcomp_5(_dp_iter_4):
#     function_id: 0
#     display_name: <dictcomp>
#     block bb3:
#         Store(Store { _meta: Meta { node_index: NodeIndex(5), range: 0..9 }, name: ExprName(ExprName { node_index: NodeIndex(5), range: 0..9, id: Name("_dp_tmp_1"), ctx: Load }), value: Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "dict" }) }), args: [], keywords: [] }) })
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..12 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..12, id: Name("_dp_iter_0_0"), ctx: Load }), value: Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 28..74 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 28..41 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 28..41, value: "iter" }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(7), range: 0..10 }, name: ExprName(ExprName { node_index: NodeIndex(7), range: 0..10, id: Name("_dp_iter_4"), ctx: Load }) }))], keywords: [] }) })
#         jump bb1
#         block bb1:
#             Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..11 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..11, id: Name("_dp_tmp_0_1"), ctx: Load }), value: Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 27..81 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 27..52 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 27..52, value: "next_or_sentinel" }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..12 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..12, id: Name("_dp_iter_0_0"), ctx: Load }) }))], keywords: [] }) })
#             if_term BinOp(BinOp { _meta: Meta { node_index: NodeIndex(None), range: 0..54 }, kind: Is, left: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..11 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..11, id: Name("_dp_tmp_0_1"), ctx: Load }) }), right: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 32..54 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 32..54, value: "ITER_COMPLETE" }) }) }):
#                 then:
#                     block bb4:
#                         return Load(Load { _meta: Meta { node_index: NodeIndex(23), range: 0..9 }, name: ExprName(ExprName { node_index: NodeIndex(23), range: 0..9, id: Name("_dp_tmp_1"), ctx: Load }) })
#                 else:
#                     block bb2:
#                         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..11 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..11, id: Name("_dp_tmp_0_1"), ctx: Load }), value: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..11 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..11, id: Name("_dp_tmp_0_1"), ctx: Load }) }) })
#                         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..11 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..11, id: Name("_dp_tmp_0_2"), ctx: Load }), value: Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 27..101 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 27..42 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 27..42, value: "unpack" }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..11 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..11, id: Name("_dp_tmp_0_1"), ctx: Load }) })), Positional(Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 0..23 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..21 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..21, value: "tuple_values" }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "TRUE" }) })), Positional(Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "TRUE" }) }))], keywords: [] }))], keywords: [] }) })
#                         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..1 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..1, id: Name("k"), ctx: Load }), value: GetItem(GetItem { _meta: Meta { node_index: NodeIndex(None), range: 0..57 }, value: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..11 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..11, id: Name("_dp_tmp_0_2"), ctx: Load }) }), index: Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(None), range: 0..1 }, literal: NumberLiteral(CoreNumberLiteral { node_index: NodeIndex(None), range: 0..1, value: Int(0) }) }) }) })
#                         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..1 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..1, id: Name("v"), ctx: Load }), value: GetItem(GetItem { _meta: Meta { node_index: NodeIndex(None), range: 0..57 }, value: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..11 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..11, id: Name("_dp_tmp_0_2"), ctx: Load }) }), index: Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(None), range: 0..1 }, literal: NumberLiteral(CoreNumberLiteral { node_index: NodeIndex(None), range: 0..1, value: Int(1) }) }) }) })
#                         Del(Del { _meta: Meta { node_index: NodeIndex(None), range: 0..11 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..11, id: Name("_dp_tmp_0_2"), ctx: Load }), quietly: false })
#                         Del(Del { _meta: Meta { node_index: NodeIndex(None), range: 0..11 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..11, id: Name("_dp_tmp_0_1"), ctx: Load }), quietly: false })
#                         jump bb5
#                         block bb5:
#                             Store(Store { _meta: Meta { node_index: NodeIndex(13), range: 0..18 }, name: ExprName(ExprName { node_index: NodeIndex(13), range: 0..18, id: Name("_dp_dictcomp_key_2"), ctx: Load }), value: Load(Load { _meta: Meta { node_index: NodeIndex(12), range: 6..7 }, name: ExprName(ExprName { node_index: NodeIndex(12), range: 6..7, id: Name("k"), ctx: Load }) }) })
#                             Store(Store { _meta: Meta { node_index: NodeIndex(16), range: 0..20 }, name: ExprName(ExprName { node_index: NodeIndex(16), range: 0..20, id: Name("_dp_dictcomp_value_3"), ctx: Load }), value: Load(Load { _meta: Meta { node_index: NodeIndex(15), range: 9..10 }, name: ExprName(ExprName { node_index: NodeIndex(15), range: 9..10, id: Name("v"), ctx: Load }) }) })
#                             Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_assign_value_6"), ctx: Store }), value: Load(Load { _meta: Meta { node_index: NodeIndex(18), range: 0..20 }, name: ExprName(ExprName { node_index: NodeIndex(18), range: 0..20, id: Name("_dp_dictcomp_value_3"), ctx: Load }) }) })
#                             Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_assign_obj_7"), ctx: Store }), value: Call(Call { _meta: Meta { node_index: NodeIndex(20), range: 0..9 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(20), range: 0..9 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(20), range: 0..9, value: "load_deleted_name" }) }), args: [Positional(Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(None), range: 0..11 }, literal: StringLiteral(CoreStringLiteral { node_index: NodeIndex(None), range: 0..11, value: "_dp_tmp_1" }) })), Positional(Load(Load { _meta: Meta { node_index: NodeIndex(20), range: 0..9 }, name: ExprName(ExprName { node_index: NodeIndex(20), range: 0..9, id: Name("_dp_tmp_1"), ctx: Load }) }))], keywords: [] }) })
#                             Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_assign_index_8"), ctx: Store }), value: Load(Load { _meta: Meta { node_index: NodeIndex(21), range: 0..18 }, name: ExprName(ExprName { node_index: NodeIndex(21), range: 0..18, id: Name("_dp_dictcomp_key_2"), ctx: Load }) }) })
#                             SetItem(SetItem { _meta: Meta { node_index: NodeIndex(19), range: 0..53 }, value: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_assign_obj_7"), ctx: Load }) }), index: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_assign_index_8"), ctx: Load }) }), replacement: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..0, id: Name("_dp_assign_value_6"), ctx: Load }) }) })
#                             jump bb1

# function _dp_module_init():
#     function_id: 1
#     block bb1:
#         Store(Store { _meta: Meta { node_index: NodeIndex(None), range: 0..14 }, name: ExprName(ExprName { node_index: NodeIndex(None), range: 0..14, id: Name("_dp_dictcomp_5"), ctx: Load }), value: MakeFunction(MakeFunction { _meta: Meta { node_index: NodeIndex(None), range: 0..200 }, function_id: FunctionId(0), kind: Function, param_defaults: Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 0..23 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..21 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..21, value: "tuple_values" }) }), args: [], keywords: [] }), annotate_fn: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) }) }) })
#         Store(Store { _meta: Meta { node_index: NodeIndex(28), range: 1..2 }, name: ExprName(ExprName { node_index: NodeIndex(28), range: 1..2, id: Name("x"), ctx: Store }), value: Call(Call { _meta: Meta { node_index: NodeIndex(25), range: 5..26 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(26), range: 0..14 }, name: ExprName(ExprName { node_index: NodeIndex(26), range: 0..14, id: Name("_dp_dictcomp_5"), ctx: Load }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(27), range: 23..25 }, name: ExprName(ExprName { node_index: NodeIndex(27), range: 23..25, id: Name("it"), ctx: Load }) }))], keywords: [] }) })
#         return Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) })

# attribute_non_chain

x = f().y

# ==

# function _dp_module_init():
#     function_id: 0
#     block bb1:
#         Store(Store { _meta: Meta { node_index: NodeIndex(6), range: 1..2 }, name: ExprName(ExprName { node_index: NodeIndex(6), range: 1..2, id: Name("x"), ctx: Store }), value: GetAttr(GetAttr { _meta: Meta { node_index: NodeIndex(3), range: 5..10 }, value: Call(Call { _meta: Meta { node_index: NodeIndex(4), range: 5..8 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(5), range: 5..6 }, name: ExprName(ExprName { node_index: NodeIndex(5), range: 5..6, id: Name("f"), ctx: Load }) }), args: [], keywords: [] }), attr: Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(3), range: 5..10 }, literal: StringLiteral(CoreStringLiteral { node_index: NodeIndex(3), range: 5..10, value: "y" }) }) }) })
#         return Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) })

# fstring_simple

x = f"{a}"

# ==

# function _dp_module_init():
#     function_id: 0
#     block bb1:
#         Store(Store { _meta: Meta { node_index: NodeIndex(5), range: 1..2 }, name: ExprName(ExprName { node_index: NodeIndex(5), range: 1..2, id: Name("x"), ctx: Store }), value: Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 0..45 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..15 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..15, value: "format" }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(4), range: 8..9 }, name: ExprName(ExprName { node_index: NodeIndex(4), range: 8..9, id: Name("a"), ctx: Load }) }))], keywords: [] }) })
#         return Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) })

# tstring_simple

x = t"{a}"

# ==

# function _dp_module_init():
#     function_id: 0
#     block bb1:
#         Store(Store { _meta: Meta { node_index: NodeIndex(5), range: 1..2 }, name: ExprName(ExprName { node_index: NodeIndex(5), range: 1..2, id: Name("x"), ctx: Store }), value: Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 0..60 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..29 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..29, value: "templatelib_Template" }) }), args: [Starred(Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 0..23 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..21 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..21, value: "tuple_values" }) }), args: [Positional(Call(Call { _meta: Meta { node_index: NodeIndex(None), range: 0..172 }, func: Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..34 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..34, value: "templatelib_Interpolation" }) }), args: [Positional(Load(Load { _meta: Meta { node_index: NodeIndex(4), range: 8..9 }, name: ExprName(ExprName { node_index: NodeIndex(4), range: 8..9, id: Name("a"), ctx: Load }) })), Positional(Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(None), range: 0..3 }, literal: StringLiteral(CoreStringLiteral { node_index: NodeIndex(None), range: 0..3, value: "a" }) })), Positional(Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) })), Positional(Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(None), range: 0..2 }, literal: StringLiteral(CoreStringLiteral { node_index: NodeIndex(None), range: 0..2, value: "" }) }))], keywords: [] }))], keywords: [] }))], keywords: [] }) })
#         return Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) })

# complex_literal

x = 1j

# ==

# snapshot regeneration failed
# panic: complex literal reached late core BlockPy boundary

# float_literal_long

x = 1.234567890123456789

# ==

# function _dp_module_init():
#     function_id: 0
#     block bb1:
#         Store(Store { _meta: Meta { node_index: NodeIndex(4), range: 1..2 }, name: ExprName(ExprName { node_index: NodeIndex(4), range: 1..2, id: Name("x"), ctx: Store }), value: Literal(LiteralValue { _meta: Meta { node_index: NodeIndex(3), range: 5..25 }, literal: NumberLiteral(CoreNumberLiteral { node_index: NodeIndex(3), range: 5..25, value: Float(1.2345678901234567) }) }) })
#         return Load(Load { _meta: Meta { node_index: NodeIndex(None), range: 0..0 }, name: RuntimeName(CoreStringLiteral { node_index: NodeIndex(None), range: 0..0, value: "NONE" }) })

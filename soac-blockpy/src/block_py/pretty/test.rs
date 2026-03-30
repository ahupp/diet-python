use super::*;
use crate::block_py::{BlockParam, BlockParamRole};
use crate::block_py::{
    BlockPyAssign, ClosureInit, ClosureSlot, LocatedName, NameLocation, RuffExpr, StorageLayout,
};
use crate::lower_python_to_blockpy_for_testing;
use crate::passes::{ResolvedStorageBlockPyPass, RuffBlockPyPass};
use ruff_python_ast as ast;
use ruff_python_parser::parse_expression;

#[derive(Debug, Clone)]
struct StructuredExprPass;

impl BlockPyPass for StructuredExprPass {
    type Name = ruff_python_ast::ExprName;
    type Expr = Expr;
    type Stmt = StructuredBlockPyStmt<Self::Expr>;
}

fn wrapped_blockpy(source: &str) -> BlockPyModule<RuffBlockPyPass> {
    lower_python_to_blockpy_for_testing(source)
        .expect("expected lowered semantic BlockPy module")
        .pass_tracker
        .pass_semantic_blockpy()
        .expect("semantic_blockpy pass should be tracked")
        .clone()
}

fn parse_blockpy_expr(source: &str) -> Expr {
    (*parse_expression(source).unwrap().into_syntax().body).into()
}

fn parse_ruff_blockpy_expr(source: &str) -> RuffExpr {
    parse_blockpy_expr(source).into()
}

fn empty_param_spec() -> ParamSpec {
    ParamSpec::default()
}

fn test_name_gen() -> crate::block_py::FunctionNameGen {
    let mut module_name_gen = crate::block_py::ModuleNameGen::new(0);
    module_name_gen.next_function_name_gen()
}

fn label(index: u32) -> BlockPyLabel {
    BlockPyLabel::from(index)
}

fn located_name(id: &str, location: NameLocation) -> LocatedName {
    LocatedName {
        id: id.into(),
        ctx: ast::ExprContext::Load,
        range: Default::default(),
        node_index: Default::default(),
        location,
    }
}
fn function_by_bind_name<'a, P>(
    module: &'a BlockPyModule<P>,
    bind_name: &str,
) -> &'a BlockPyFunction<P>
where
    P: BlockPyPrettyPrinter,
{
    module
        .callable_defs
        .iter()
        .find(|function| function.names.bind_name == bind_name)
        .unwrap_or_else(|| panic!("missing function {bind_name}"))
}

#[test]
fn renders_blockpy_module_with_module_init_and_nested_blocks() {
    let blockpy = wrapped_blockpy(
        r#"
seed = 1

def classify(a, /, b: int = 1, *args, c=2, **kwargs):
    if a:
        return "yes"
    return "no"
"#,
    );
    let classify = function_by_bind_name(&blockpy, "classify");
    let rendered = blockpy_module_to_string(&blockpy);

    assert!(
        rendered.contains("function classify(a, /, b, *args, c, **kwargs):"),
        "{rendered}"
    );
    assert!(rendered.contains("function_id: "), "{rendered}");
    assert!(rendered.contains("function _dp_module_init():"));
    assert!(!rendered.contains("module_init: _dp_module_init"));
    assert!(
        rendered.contains(format!("block {}:", classify.entry_block().label_str()).as_str())
            || rendered.contains(format!("block {}(", classify.entry_block().label_str()).as_str()),
        "{rendered}"
    );
    assert!(rendered.contains("if_term a:"));
    assert!(rendered.contains("return \"yes\""));
}

#[test]
fn renders_empty_module_marker() {
    let empty_module: BlockPyModule<RuffBlockPyPass> = BlockPyModule {
        callable_defs: Vec::new(),
    };
    let rendered = blockpy_module_to_string(&empty_module);
    assert_eq!(rendered, "; empty BlockPy module\n");
}

#[test]
fn bb_text_renders_located_names_with_resolved_locations() {
    let closure_expr = CoreBlockPyExpr::Name(located_name(
        "captured",
        NameLocation::ClosureCell { slot: 2 },
    ));
    let assign_stmt = BlockPyStmt::Assign(BlockPyAssign {
        target: located_name("temp", NameLocation::Local { slot: 1 }),
        value: closure_expr.clone(),
    });
    let global_expr = CoreBlockPyExpr::Name(located_name("answer", NameLocation::Global));

    assert_eq!(bb_expr_text(&closure_expr), "closure slot 2");
    assert_eq!(
        core_bb_stmt_text(&assign_stmt),
        "local slot 1 = closure slot 2"
    );
    assert_eq!(bb_expr_text(&global_expr), "answer");
}

#[test]
fn transformed_lowering_result_exposes_module_init_blockpy() {
    let blockpy = lower_python_to_blockpy_for_testing(
        r#"
def classify(n):
    if n < 0:
        return "neg"
    return "pos"
"#,
    )
    .unwrap()
    .pass_tracker
    .pass_semantic_blockpy()
    .expect("semantic_blockpy pass should be tracked")
    .clone();
    let rendered = blockpy_module_to_string(&blockpy);

    assert!(blockpy
        .callable_defs
        .iter()
        .any(|function| function.names.bind_name == "_dp_module_init"));
    assert!(rendered.contains("function _dp_module_init():"));
    assert!(rendered.contains("function_id: "), "{rendered}");
}

#[test]
fn debug_blockpy_render_uses_blockpy_expr_text_for_core_ops() {
    let blockpy = lower_python_to_blockpy_for_testing(
        r#"
def tweak(x):
    x += 1
    return x
"#,
    )
    .unwrap()
    .pass_tracker
    .pass_core_blockpy()
    .expect("core_blockpy pass should be tracked")
    .clone();
    let rendered = blockpy_module_to_debug_string(&blockpy);

    assert!(rendered.contains("InplaceBinOp(Add,"), "{rendered}");
    assert!(!rendered.contains("__dp_iadd"), "{rendered}");
}

#[test]
fn renders_generator_kind_without_internal_metadata() {
    let blockpy = wrapped_blockpy(
        r#"
def gen():
    yield 1
"#,
    );
    let rendered = blockpy_module_to_string(&blockpy);

    assert!(rendered.contains("generator gen():"));
    assert!(rendered.contains("function_id: "), "{rendered}");
    assert!(!rendered.contains("generator_state:"));
}

#[test]
fn renders_referenced_non_inlined_blocks_for_async_generator_shape() {
    let blockpy = wrapped_blockpy(
        r#"
async def a():
    return 3

async def no_lying():
    for i in range((await a()) + 2):
        yield i
"#,
    );
    let function = function_by_bind_name(&blockpy, "no_lying");
    let rendered = blockpy_module_to_string(&BlockPyModule {
        callable_defs: vec![function.clone()],
    });
    let layout = BlockRenderLayout::new(function);
    let inlined_labels = layout
        .inlined_blocks
        .iter()
        .map(|index| function.blocks[*index].label.to_string())
        .collect::<HashSet<_>>();

    let missing_labels = collect_referenced_labels_from_blocks::<RuffBlockPyPass>(&function.blocks)
        .into_iter()
        .map(|label| label.to_string())
        .filter(|label| !inlined_labels.contains(label))
        .filter(|label| {
            !rendered.contains(format!("block {label}:").as_str())
                && !rendered.contains(format!("block {label}(").as_str())
        })
        .collect::<Vec<_>>();

    assert!(missing_labels.is_empty(), "{rendered}");
}

#[test]
fn renders_public_closure_metadata_in_function_header() {
    let rendered = blockpy_module_to_string(&BlockPyModule {
        callable_defs: vec![BlockPyFunction::<RuffBlockPyPass> {
            function_id: crate::block_py::FunctionId(0),
            name_gen: test_name_gen(),
            names: crate::block_py::FunctionName::new("gen", "gen", "gen", "gen"),
            kind: BlockPyFunctionKind::Function,
            params: empty_param_spec(),
            blocks: vec![CfgBlock {
                label: label(0),
                body: vec![],
                term: BlockPyTerm::Return(parse_ruff_blockpy_expr("__dp_NONE")),
                params: Vec::new(),
                exc_edge: None,
            }],
            doc: None,
            storage_layout: Some(StorageLayout {
                freevars: vec![ClosureSlot {
                    logical_name: "factor".to_string(),
                    storage_name: "factor".to_string(),
                    init: ClosureInit::InheritedCapture,
                }],
                cellvars: vec![ClosureSlot {
                    logical_name: "total".to_string(),
                    storage_name: "_dp_cell_total".to_string(),
                    init: ClosureInit::Deferred,
                }],
                runtime_cells: vec![ClosureSlot {
                    logical_name: "_dp_pc".to_string(),
                    storage_name: "_dp_cell__dp_pc".to_string(),
                    init: ClosureInit::RuntimePcUnstarted,
                }],
                stack_slots: Vec::new(),
            }),
            semantic: crate::block_py::BlockPyCallableSemanticInfo::default(),
        }],
    });

    assert!(rendered.contains(
            "function gen():\n    function_id: 0\n    freevars: [factor->factor@inherited]\n    cellvars: [total->_dp_cell_total@deferred]\n    runtime_cells: [_dp_pc->_dp_cell__dp_pc@pc_unstarted]"
        ));
    assert!(!rendered.contains("entry:"));
}

#[test]
fn renders_followup_blocks_under_their_owning_entry_block() {
    let function: BlockPyFunction<RuffBlockPyPass> = BlockPyFunction {
        function_id: crate::block_py::FunctionId(0),
        name_gen: test_name_gen(),
        names: crate::block_py::FunctionName::new("f", "f", "f", "f"),
        kind: BlockPyFunctionKind::Function,
        params: empty_param_spec(),
        blocks: vec![
            CfgBlock {
                label: label(0),
                body: vec![],
                term: BlockPyTerm::IfTerm(BlockPyIfTerm {
                    test: parse_ruff_blockpy_expr("cond"),
                    then_label: label(1),
                    else_label: label(2),
                }),
                params: Vec::new(),
                exc_edge: None,
            },
            CfgBlock {
                label: label(1),
                body: vec![StructuredBlockPyStmt::Expr(parse_ruff_blockpy_expr(
                    "then_side_effect()",
                ))
                .into()],
                term: BlockPyTerm::Jump(label(3).into()),
                params: Vec::new(),
                exc_edge: None,
            },
            CfgBlock {
                label: label(2),
                body: vec![StructuredBlockPyStmt::Expr(parse_ruff_blockpy_expr(
                    "else_side_effect()",
                ))
                .into()],
                term: BlockPyTerm::Jump(label(3).into()),
                params: Vec::new(),
                exc_edge: None,
            },
            CfgBlock {
                label: label(3),
                body: vec![StructuredBlockPyStmt::Expr(parse_ruff_blockpy_expr("finish()")).into()],
                term: BlockPyTerm::Return(parse_ruff_blockpy_expr("__dp_NONE")),
                params: Vec::new(),
                exc_edge: None,
            },
        ],
        doc: None,
        storage_layout: None,
        semantic: crate::block_py::BlockPyCallableSemanticInfo::default(),
    };
    let rendered = blockpy_module_to_string(&BlockPyModule {
        callable_defs: vec![function],
    });

    assert!(rendered.contains("    block bb0:\n"));
    assert!(rendered.contains("        block bb3:\n"));
    assert!(rendered.contains(
            "        if_term cond:\n            then:\n                block bb1:\n                    then_side_effect()\n                    jump bb3\n            else:\n                block bb2:\n                    else_side_effect()\n                    jump bb3\n        block bb3:\n            finish()\n            return __dp_NONE\n"
        ));
}

#[test]
fn elides_trivial_if_term_jump_wrappers_when_rendering() {
    let blockpy = wrapped_blockpy(
        r#"
def choose(a, b):
    total = a + b
    if total > 5:
        return a
    else:
        return b
"#,
    );
    let rendered = blockpy_module_to_string(&blockpy);

    assert!(rendered.contains("return a"), "{rendered}");
    assert!(rendered.contains("return b"), "{rendered}");
    assert!(!rendered.contains("block _dp_bb_choose_1_then"));
    assert!(!rendered.contains("block _dp_bb_choose_1_else"));
}

#[test]
fn sorts_rendered_root_and_child_blocks_by_label() {
    let function: BlockPyFunction<RuffBlockPyPass> = BlockPyFunction {
        function_id: crate::block_py::FunctionId(0),
        name_gen: test_name_gen(),
        names: crate::block_py::FunctionName::new("f", "f", "f", "f"),
        kind: BlockPyFunctionKind::Function,
        params: empty_param_spec(),
        blocks: vec![
            CfgBlock {
                label: label(0),
                body: vec![],
                term: BlockPyTerm::Jump(label(4).into()),
                params: Vec::new(),
                exc_edge: Some(BlockPyEdge::new(label(1))),
            },
            CfgBlock {
                label: label(4),
                body: vec![],
                term: BlockPyTerm::Return(parse_ruff_blockpy_expr("__dp_NONE")),
                params: Vec::new(),
                exc_edge: None,
            },
            CfgBlock {
                label: label(1),
                body: vec![],
                term: BlockPyTerm::Return(parse_ruff_blockpy_expr("__dp_NONE")),
                params: Vec::new(),
                exc_edge: None,
            },
            CfgBlock {
                label: label(3),
                body: vec![],
                term: BlockPyTerm::Return(parse_ruff_blockpy_expr("__dp_NONE")),
                params: Vec::new(),
                exc_edge: None,
            },
            CfgBlock {
                label: label(2),
                body: vec![],
                term: BlockPyTerm::Return(parse_ruff_blockpy_expr("__dp_NONE")),
                params: Vec::new(),
                exc_edge: None,
            },
        ],
        doc: None,
        storage_layout: None,
        semantic: crate::block_py::BlockPyCallableSemanticInfo::default(),
    };
    let rendered = blockpy_module_to_string(&BlockPyModule {
        callable_defs: vec![function],
    });

    let alpha_pos = rendered.find("block bb1:").expect("bb1 block");
    let zeta_pos = rendered.find("block bb4:").expect("bb4 block");
    let beta_pos = rendered.find("block bb2:").expect("bb2 block");
    let omega_pos = rendered.find("block bb3:").expect("bb3 block");

    assert!(zeta_pos < alpha_pos, "{rendered}");
    assert!(beta_pos < omega_pos, "{rendered}");
}

#[test]
fn collects_referenced_labels_from_nested_if_fragments_via_visitor() {
    let referenced = collect_referenced_labels_from_blocks::<StructuredExprPass>(&[CfgBlock {
        label: label(0),
        body: vec![StructuredBlockPyStmt::If(crate::block_py::BlockPyIf {
            test: parse_blockpy_expr("cond"),
            body: BlockPyCfgFragment {
                body: Vec::new(),
                term: Some(BlockPyTerm::Jump(label(1).into())),
            },
            orelse: BlockPyCfgFragment {
                body: Vec::new(),
                term: Some(BlockPyTerm::BranchTable(super::super::BlockPyBranchTable {
                    index: parse_blockpy_expr("index"),
                    targets: vec![label(2), label(3)],
                    default_label: label(4),
                })),
            },
        })],
        term: BlockPyTerm::Jump(label(5).into()),
        params: Vec::new(),
        exc_edge: Some(BlockPyEdge::new(label(6))),
    }]);

    let expected = [label(1), label(2), label(3), label(4), label(5), label(6)]
        .into_iter()
        .collect::<HashSet<_>>();

    assert_eq!(referenced, expected);
}

#[test]
fn renders_bb_block_metadata_with_shared_layout() {
    let rendered = blockpy_module_to_string(&BlockPyModule {
        callable_defs: vec![BlockPyFunction::<ResolvedStorageBlockPyPass> {
            function_id: crate::block_py::FunctionId(0),
            name_gen: test_name_gen(),
            names: crate::block_py::FunctionName::new("f", "f", "f", "f"),
            kind: BlockPyFunctionKind::Function,
            params: empty_param_spec(),
            blocks: vec![
                PassBlock::<ResolvedStorageBlockPyPass> {
                    label: label(0),
                    body: vec![],
                    term: BlockPyTerm::Jump(label(1).into()),
                    params: vec![
                        BlockParam {
                            name: "err".to_string(),
                            role: BlockParamRole::Exception,
                        },
                        BlockParam {
                            name: "x".to_string(),
                            role: BlockParamRole::AbruptPayload,
                        },
                    ],
                    exc_edge: Some(BlockPyEdge::new(label(1))),
                },
                PassBlock::<ResolvedStorageBlockPyPass> {
                    label: label(1),
                    body: vec![],
                    term: BlockPyTerm::Return(
                        <crate::block_py::LocatedCoreBlockPyExpr as crate::block_py::ImplicitNoneExpr>::implicit_none_expr(
                        ),
                    ),
                    params: vec![BlockParam {
                        name: "err".to_string(),
                        role: BlockParamRole::Exception,
                    }],
                    exc_edge: None,
                },
            ],
            doc: None,
            storage_layout: None,
            semantic: crate::block_py::BlockPyCallableSemanticInfo::default(),
        }],
    });

    assert!(rendered.contains("function f():"), "{rendered}");
    assert!(rendered.contains("function_id: 0"), "{rendered}");
    assert!(
        rendered.contains("block bb0(err: Exception, x: AbruptPayload):"),
        "{rendered}"
    );
    assert!(rendered.contains("exc_target: bb1"), "{rendered}");
    assert!(rendered.contains("exc_name: err"), "{rendered}");
    assert!(rendered.contains("jump bb1"), "{rendered}");
}

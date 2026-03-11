use super::super::bb_ir::BindingTarget;
use super::{
    BlockPyBlock, BlockPyExpr, BlockPyFunction, BlockPyFunctionKind, BlockPyIfTerm, BlockPyLabel,
    BlockPyModule, BlockPyRaise, BlockPyStmt, BlockPyTerm, BlockPyTryJump,
};
use crate::ruff_ast_to_string;
use ruff_python_ast::{self as ast, Expr};
use std::collections::HashSet;

pub fn blockpy_module_to_string(module: &BlockPyModule) -> String {
    let mut formatter = BlockPyFormatter::default();
    formatter.write_module(module);
    formatter.finish()
}

#[derive(Default)]
struct BlockPyFormatter {
    out: String,
    indent: usize,
}

impl BlockPyFormatter {
    fn finish(mut self) -> String {
        if self.out.is_empty() {
            self.line("; empty BlockPy module");
        }
        self.out
    }

    fn write_module(&mut self, module: &BlockPyModule) {
        if let Some(module_init) = &module.module_init {
            self.line(format!("module_init: {module_init}"));
        }

        for function in &module.functions {
            if !self.out.is_empty() {
                self.out.push('\n');
            }
            self.write_function(function);
        }
    }

    fn write_function(&mut self, function: &BlockPyFunction) {
        let params = format_parameters(&function.params);
        let referenced_labels = collect_referenced_labels_from_blocks(&function.blocks);
        self.line(format!(
            "function {}({params}) [kind={}, bind={}, target={}, qualname={}]",
            function.bind_name,
            function_kind_name(function.kind),
            function.bind_name,
            binding_target_name(function.binding_target),
            function.qualname,
        ));
        self.with_indent(|this| {
            if function.blocks.is_empty() {
                this.line("pass");
            } else {
                for block in &function.blocks {
                    this.write_block(block, &referenced_labels);
                }
            }
        });
    }

    fn write_block(&mut self, block: &BlockPyBlock, referenced_labels: &HashSet<BlockPyLabel>) {
        if referenced_labels.contains(&block.label) {
            self.line(format!("block {}:", block.label.as_str()));
            self.with_indent(|this| this.write_block_contents(block, referenced_labels));
        } else {
            self.write_block_contents(block, referenced_labels);
        }
    }

    fn write_block_contents(
        &mut self,
        block: &BlockPyBlock,
        referenced_labels: &HashSet<BlockPyLabel>,
    ) {
        if block.body.is_empty() {
            self.write_term(&block.term, referenced_labels);
            return;
        }
        self.write_stmt_list(&block.body, referenced_labels, false);
        self.write_term(&block.term, referenced_labels);
    }

    fn write_stmt_list(
        &mut self,
        stmts: &[BlockPyStmt],
        referenced_labels: &HashSet<BlockPyLabel>,
        allow_terminals: bool,
    ) {
        for stmt in stmts {
            self.write_stmt(stmt, referenced_labels, allow_terminals);
        }
    }

    fn write_block_list(
        &mut self,
        stmts: &[BlockPyStmt],
        referenced_labels: &HashSet<BlockPyLabel>,
    ) {
        if stmts.is_empty() {
            self.line("pass");
            return;
        }
        self.write_stmt_list(stmts, referenced_labels, true);
    }

    fn write_stmt(
        &mut self,
        stmt: &BlockPyStmt,
        referenced_labels: &HashSet<BlockPyLabel>,
        allow_terminals: bool,
    ) {
        match stmt {
            BlockPyStmt::Pass => self.line("pass"),
            BlockPyStmt::Assign(assign) => self.line(format!(
                "{} = {}",
                assign.target.id,
                render_inline_expr(&assign.value)
            )),
            BlockPyStmt::Expr(expr) => self.line(render_inline_expr(expr)),
            BlockPyStmt::Delete(delete) => self.line(format!("del {}", delete.target.id)),
            BlockPyStmt::FunctionDef(func) => {
                let params = format_parameters(&func.parameters);
                self.line(format!("def {}({params}): ...", func.name.id));
            }
            BlockPyStmt::If(if_stmt) => {
                self.line(format!("if {}:", render_inline_expr(&if_stmt.test)));
                self.with_indent(|this| this.write_block_list(&if_stmt.body, referenced_labels));
                if !if_stmt.orelse.is_empty() {
                    self.line("else:");
                    self.with_indent(|this| {
                        this.write_block_list(&if_stmt.orelse, referenced_labels)
                    });
                }
            }
            BlockPyStmt::Jump(label) => {
                if allow_terminals {
                    self.line(format!("jump {}", label.as_str()));
                } else {
                    panic!("terminal BlockPyStmt leaked into block body: {stmt:?}");
                }
            }
            BlockPyStmt::BranchTable(branch) => {
                if allow_terminals {
                    self.line(format!(
                        "branch_table {} -> [{}] default {}",
                        render_inline_expr(&branch.index),
                        join_labels(&branch.targets),
                        branch.default_label.as_str(),
                    ));
                } else {
                    panic!("terminal BlockPyStmt leaked into block body: {stmt:?}");
                }
            }
            BlockPyStmt::Return(value) => {
                if allow_terminals {
                    match value {
                        Some(value) => self.line(format!("return {}", render_inline_expr(value))),
                        None => self.line("return"),
                    }
                } else {
                    panic!("terminal BlockPyStmt leaked into block body: {stmt:?}");
                }
            }
            BlockPyStmt::Raise(raise_stmt) => {
                if allow_terminals {
                    self.write_raise(raise_stmt);
                } else {
                    panic!("terminal BlockPyStmt leaked into block body: {stmt:?}");
                }
            }
            BlockPyStmt::TryJump(try_jump) => {
                if allow_terminals {
                    self.write_try_jump(try_jump);
                } else {
                    panic!("terminal BlockPyStmt leaked into block body: {stmt:?}");
                }
            }
        }
    }

    fn write_term(&mut self, term: &BlockPyTerm, referenced_labels: &HashSet<BlockPyLabel>) {
        match term {
            BlockPyTerm::Jump(label) => self.line(format!("jump {}", label.as_str())),
            BlockPyTerm::IfTerm(BlockPyIfTerm { test, body, orelse }) => {
                self.line(format!("if_term {}:", render_inline_expr(test)));
                self.with_indent(|this| {
                    this.line("then:");
                    this.with_indent(|this| this.write_block(body, referenced_labels));
                    this.line("else:");
                    this.with_indent(|this| this.write_block(orelse, referenced_labels));
                });
            }
            BlockPyTerm::BranchTable(branch) => self.line(format!(
                "branch_table {} -> [{}] default {}",
                render_inline_expr(&branch.index),
                join_labels(&branch.targets),
                branch.default_label.as_str(),
            )),
            BlockPyTerm::Raise(raise_stmt) => self.write_raise(raise_stmt),
            BlockPyTerm::TryJump(try_jump) => self.write_try_jump(try_jump),
            BlockPyTerm::Return(value) => match value {
                Some(value) => self.line(format!("return {}", render_inline_expr(value))),
                None => self.line("return"),
            },
        }
    }

    fn write_raise(&mut self, raise_stmt: &BlockPyRaise) {
        match &raise_stmt.exc {
            Some(exc) => self.line(format!("raise {}", render_inline_expr(exc))),
            None => self.line("raise"),
        }
    }

    fn write_try_jump(&mut self, try_jump: &BlockPyTryJump) {
        self.line("try_jump:");
        self.with_indent(|this| {
            this.line(format!("body_label: {}", try_jump.body_label.as_str()));
            this.line(format!("except_label: {}", try_jump.except_label.as_str()));
        });
    }

    fn with_indent(&mut self, f: impl FnOnce(&mut Self)) {
        self.indent += 1;
        f(self);
        self.indent -= 1;
    }

    fn line(&mut self, line: impl AsRef<str>) {
        for _ in 0..self.indent {
            self.out.push_str("    ");
        }
        self.out.push_str(line.as_ref());
        self.out.push('\n');
    }
}

fn binding_target_name(target: BindingTarget) -> &'static str {
    match target {
        BindingTarget::Local => "local",
        BindingTarget::ModuleGlobal => "module_global",
        BindingTarget::ClassNamespace => "class_namespace",
    }
}

fn function_kind_name(kind: BlockPyFunctionKind) -> &'static str {
    match kind {
        BlockPyFunctionKind::Function => "function",
        BlockPyFunctionKind::Coroutine => "coroutine",
        BlockPyFunctionKind::Generator => "generator",
        BlockPyFunctionKind::AsyncGenerator => "async_generator",
    }
}

fn render_inline_expr(expr: &BlockPyExpr) -> String {
    ruff_ast_to_string(&expr.to_expr())
        .lines()
        .map(str::trim)
        .collect::<Vec<_>>()
        .join(" ")
}

fn render_annotation(annotation: Option<&Expr>) -> String {
    annotation
        .map(|expr| {
            format!(
                ": {}",
                ruff_ast_to_string(expr)
                    .lines()
                    .map(str::trim)
                    .collect::<Vec<_>>()
                    .join(" ")
            )
        })
        .unwrap_or_default()
}

fn render_default(default: Option<&Expr>) -> String {
    default
        .map(|expr| {
            format!(
                " = {}",
                ruff_ast_to_string(expr)
                    .lines()
                    .map(str::trim)
                    .collect::<Vec<_>>()
                    .join(" ")
            )
        })
        .unwrap_or_default()
}

fn format_named_parameter(parameter: &ast::Parameter) -> String {
    format!(
        "{}{}",
        parameter.name.id,
        render_annotation(parameter.annotation.as_deref())
    )
}

fn format_parameter_with_default(parameter: &ast::ParameterWithDefault) -> String {
    format!(
        "{}{}",
        format_named_parameter(&parameter.parameter),
        render_default(parameter.default.as_deref())
    )
}

fn format_parameters(parameters: &ast::Parameters) -> String {
    let mut parts = Vec::new();

    for param in &parameters.posonlyargs {
        parts.push(format_parameter_with_default(param));
    }
    if !parameters.posonlyargs.is_empty() {
        parts.push("/".to_string());
    }

    for param in &parameters.args {
        parts.push(format_parameter_with_default(param));
    }

    if let Some(vararg) = &parameters.vararg {
        parts.push(format!("*{}", format_named_parameter(vararg)));
    } else if !parameters.kwonlyargs.is_empty() {
        parts.push("*".to_string());
    }

    for param in &parameters.kwonlyargs {
        parts.push(format_parameter_with_default(param));
    }

    if let Some(kwarg) = &parameters.kwarg {
        parts.push(format!("**{}", format_named_parameter(kwarg)));
    }

    parts.join(", ")
}

fn join_labels(labels: &[BlockPyLabel]) -> String {
    labels
        .iter()
        .map(|label| label.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

fn collect_referenced_labels_from_blocks(blocks: &[BlockPyBlock]) -> HashSet<BlockPyLabel> {
    let mut referenced = HashSet::new();
    for block in blocks {
        collect_referenced_labels_from_stmts(&block.body, &mut referenced);
        collect_referenced_labels_from_term(&block.term, &mut referenced);
    }
    referenced
}

fn collect_referenced_labels_from_stmts(
    stmts: &[BlockPyStmt],
    referenced: &mut HashSet<BlockPyLabel>,
) {
    for stmt in stmts {
        match stmt {
            BlockPyStmt::If(if_stmt) => {
                collect_referenced_labels_from_stmts(&if_stmt.body, referenced);
                collect_referenced_labels_from_stmts(&if_stmt.orelse, referenced);
            }
            BlockPyStmt::BranchTable(branch) => {
                referenced.extend(branch.targets.iter().cloned());
                referenced.insert(branch.default_label.clone());
            }
            BlockPyStmt::Jump(label) => {
                referenced.insert(label.clone());
            }
            BlockPyStmt::TryJump(try_jump) => {
                referenced.insert(try_jump.body_label.clone());
                referenced.insert(try_jump.except_label.clone());
            }
            _ => {}
        }
    }
}

fn collect_referenced_labels_from_term(term: &BlockPyTerm, referenced: &mut HashSet<BlockPyLabel>) {
    match term {
        BlockPyTerm::Jump(label) => {
            referenced.insert(label.clone());
        }
        BlockPyTerm::IfTerm(if_term) => {
            referenced.insert(if_term.body.label.clone());
            referenced.insert(if_term.orelse.label.clone());
            collect_referenced_labels_from_blocks_into(
                std::slice::from_ref(if_term.body.as_ref()),
                referenced,
            );
            collect_referenced_labels_from_blocks_into(
                std::slice::from_ref(if_term.orelse.as_ref()),
                referenced,
            );
        }
        BlockPyTerm::BranchTable(branch) => {
            referenced.extend(branch.targets.iter().cloned());
            referenced.insert(branch.default_label.clone());
        }
        BlockPyTerm::TryJump(try_jump) => {
            referenced.insert(try_jump.body_label.clone());
            referenced.insert(try_jump.except_label.clone());
        }
        BlockPyTerm::Raise(_) | BlockPyTerm::Return(_) => {}
    }
}

fn collect_referenced_labels_from_blocks_into(
    blocks: &[BlockPyBlock],
    referenced: &mut HashSet<BlockPyLabel>,
) {
    for block in blocks {
        collect_referenced_labels_from_stmts(&block.body, referenced);
        collect_referenced_labels_from_term(&block.term, referenced);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::basic_block::collect_function_identity_by_node;
    use crate::basic_block::ruff_to_blockpy::rewrite_ast_to_blockpy_module;

    fn wrapped_blockpy(source: &str) -> BlockPyModule {
        let mut module = ruff_python_parser::parse_module(source)
            .unwrap()
            .into_syntax()
            .body;
        crate::driver::wrap_module_init(&mut module);
        let scope = crate::analyze_module_scope(&mut module);
        let identities = collect_function_identity_by_node(&mut module, scope);
        rewrite_ast_to_blockpy_module(&module, &identities).unwrap()
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
        let rendered = blockpy_module_to_string(&blockpy);

        assert!(rendered.contains("module_init: _dp_module_init"));
        assert!(rendered.contains(
            "function classify(a, /, b: int = 1, *args, c = 2, **kwargs) [kind=function, bind=classify, target=module_global, qualname=classify]"
        ));
        assert!(rendered.contains("function _dp_module_init()"));
        assert!(!rendered.contains("block start:"));
        assert!(rendered.contains("if_term a:"));
        assert!(rendered.contains("return \"yes\""));
    }

    #[test]
    fn renders_empty_module_marker() {
        let rendered = blockpy_module_to_string(&BlockPyModule {
            functions: Vec::new(),
            module_init: None,
        });
        assert_eq!(rendered, "; empty BlockPy module\n");
    }

    #[test]
    fn transformed_lowering_result_exposes_module_init_blockpy() {
        let lowered = crate::transform_str_to_ruff_with_options(
            r#"
def classify(n):
    if n < 0:
        return "neg"
    return "pos"
"#,
            crate::Options::default(),
        )
        .unwrap();
        let blockpy = lowered.blockpy_module.expect("expected BlockPy module");
        let rendered = blockpy_module_to_string(&blockpy);

        assert_eq!(blockpy.module_init.as_deref(), Some("_dp_module_init"));
        assert!(rendered.contains("function _dp_module_init()"));
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

        assert!(rendered.contains("function gen() [kind=generator"));
        assert!(!rendered.contains("generator_state:"));
    }
}

use super::super::bb_ir::BindingTarget;
use super::{
    BlockPyBlock, BlockPyBrIf, BlockPyExpr, BlockPyFunction, BlockPyFunctionKind, BlockPyLabel,
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
        if !module.prelude.is_empty() {
            let referenced_labels = HashSet::new();
            self.line("prelude:");
            self.with_indent(|this| this.write_stmt_list(&module.prelude, &referenced_labels));
        }

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
            self.write_term(&block.term);
            return;
        }
        self.write_stmt_list(&block.body, referenced_labels);
        self.write_term(&block.term);
    }

    fn write_stmt_list(
        &mut self,
        stmts: &[BlockPyStmt],
        referenced_labels: &HashSet<BlockPyLabel>,
    ) {
        for stmt in stmts {
            self.write_stmt(stmt, referenced_labels);
        }
    }

    fn write_block_list(
        &mut self,
        blocks: &[BlockPyBlock],
        referenced_labels: &HashSet<BlockPyLabel>,
    ) {
        if blocks.is_empty() {
            self.line("pass");
            return;
        }
        for block in blocks {
            self.write_block(block, referenced_labels);
        }
    }

    fn write_stmt(&mut self, stmt: &BlockPyStmt, referenced_labels: &HashSet<BlockPyLabel>) {
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
            BlockPyStmt::BranchTable(_)
            | BlockPyStmt::Jump(_)
            | BlockPyStmt::Return(_)
            | BlockPyStmt::Raise(_)
            | BlockPyStmt::TryJump(_) => {
                panic!("terminal BlockPyStmt leaked into block body: {stmt:?}")
            }
        }
    }

    fn write_term(&mut self, term: &BlockPyTerm) {
        match term {
            BlockPyTerm::Jump(label) => self.line(format!("jump {}", label.as_str())),
            BlockPyTerm::BrIf(BlockPyBrIf {
                test,
                then_label,
                else_label,
            }) => self.line(format!(
                "br_if {} ? {} : {}",
                render_inline_expr(test),
                then_label.as_str(),
                else_label.as_str(),
            )),
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
                collect_referenced_labels_from_blocks_into(&if_stmt.body, referenced);
                collect_referenced_labels_from_blocks_into(&if_stmt.orelse, referenced);
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
        BlockPyTerm::BrIf(br_if) => {
            referenced.insert(br_if.then_label.clone());
            referenced.insert(br_if.else_label.clone());
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::basic_block::bb_ir::BindingTarget;
    use crate::basic_block::ruff_to_blockpy::rewrite_ast_to_blockpy_module;
    use crate::basic_block::FunctionIdentityByNode;

    fn function_identity(stmt: &ast::StmtFunctionDef) -> FunctionIdentityByNode {
        FunctionIdentityByNode::from([(
            stmt.node_index.load(),
            (
                stmt.name.id.to_string(),
                stmt.name.id.to_string(),
                stmt.name.id.to_string(),
                BindingTarget::ModuleGlobal,
            ),
        )])
    }

    #[test]
    fn renders_blockpy_module_with_prelude_and_nested_blocks() {
        let module = ruff_python_parser::parse_module(
            r#"
seed = 1

def classify(a, /, b: int = 1, *args, c=2, **kwargs):
    if a:
        return "yes"
    return "no"
"#,
        )
        .unwrap()
        .into_syntax()
        .body;
        let ast::Stmt::FunctionDef(func) = module.body[1].as_ref() else {
            panic!("expected function def");
        };
        let blockpy = rewrite_ast_to_blockpy_module(&module, &function_identity(func)).unwrap();
        let rendered = blockpy_module_to_string(&blockpy);

        assert!(rendered.contains("prelude:\n    seed = 1\n"));
        assert!(rendered.contains(
            "function classify(a, /, b: int = 1, *args, c = 2, **kwargs) [kind=function, bind=classify, target=module_global, qualname=classify]"
        ));
        assert!(!rendered.contains("block start:"));
        assert!(rendered.contains("br_if a ?"));
        assert!(rendered.contains("return \"yes\""));
    }

    #[test]
    fn renders_empty_module_marker() {
        let rendered = blockpy_module_to_string(&BlockPyModule {
            prelude: Vec::new(),
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
        let module = ruff_python_parser::parse_module(
            r#"
def gen():
    yield 1
"#,
        )
        .unwrap()
        .into_syntax()
        .body;
        let ast::Stmt::FunctionDef(func) = module.body[0].as_ref() else {
            panic!("expected function def");
        };
        let blockpy = rewrite_ast_to_blockpy_module(&module, &function_identity(func)).unwrap();
        let rendered = blockpy_module_to_string(&blockpy);

        assert!(rendered.contains("function gen() [kind=generator"));
        assert!(!rendered.contains("generator_state:"));
    }
}

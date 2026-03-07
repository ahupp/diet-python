use super::*;
use crate::transform::util::{
    strip_synthetic_class_namespace_qualname, strip_synthetic_module_init_qualname,
};
use ruff_python_codegen::{Generator, Indentation};
use ruff_source_file::LineEnding;

#[derive(Clone)]
pub(super) struct FunctionIdentity {
    pub(super) bind_name: String,
    pub(super) display_name: String,
    pub(super) qualname: String,
    pub(super) binding_target: BindingTarget,
}

pub(super) fn display_name_for_function(raw_name: &str) -> &str {
    if raw_name.starts_with("_dp_lambda_") {
        "<lambda>"
    } else if raw_name.starts_with("_dp_genexpr_") {
        "<genexpr>"
    } else if raw_name.starts_with("_dp_listcomp_") {
        "<listcomp>"
    } else if raw_name.starts_with("_dp_setcomp_") {
        "<setcomp>"
    } else if raw_name.starts_with("_dp_dictcomp_") {
        "<dictcomp>"
    } else {
        raw_name
    }
}

fn normalize_qualname(raw_qualname: &str, raw_name: &str, display_name: &str) -> String {
    let raw_qualname = strip_synthetic_module_init_qualname(raw_qualname);
    let raw_qualname = strip_synthetic_class_namespace_qualname(&raw_qualname);
    let should_replace_tail = matches!(display_name, "<lambda>" | "<genexpr>");
    if raw_name == display_name || !should_replace_tail {
        return raw_qualname;
    }
    match raw_qualname.rsplit_once('.') {
        Some((prefix, _)) => format!("{prefix}.{display_name}"),
        None => display_name.to_string(),
    }
}

pub(super) fn collect_function_identity_private(
    module: &mut StmtBody,
    module_scope: Arc<Scope>,
) -> HashMap<NodeIndex, FunctionIdentity> {
    fn binding_target_for_scope(scope: &Scope, bind_name: &str) -> BindingTarget {
        if is_internal_symbol(bind_name) {
            return BindingTarget::Local;
        }
        let binding = scope.binding_in_scope(bind_name, BindingUse::Load);
        match (scope.kind(), binding) {
            (ScopeKind::Class, BindingKind::Local) => BindingTarget::ClassNamespace,
            (_, BindingKind::Global) => BindingTarget::ModuleGlobal,
            _ => BindingTarget::Local,
        }
    }

    struct Collector {
        scope_stack: Vec<Arc<Scope>>,
        out: HashMap<NodeIndex, FunctionIdentity>,
    }

    impl Transformer for Collector {
        fn visit_stmt(&mut self, stmt: &mut Stmt) {
            match stmt {
                Stmt::FunctionDef(func) => {
                    let node_index = func.node_index.load();
                    if node_index != NodeIndex::NONE {
                        let raw_bind_name = func.name.id.to_string();
                        let bind_name = if is_module_init_temp_name(raw_bind_name.as_str()) {
                            "_dp_module_init".to_string()
                        } else {
                            raw_bind_name.clone()
                        };
                        let display_name =
                            display_name_for_function(bind_name.as_str()).to_string();
                        let parent_scope = self
                            .scope_stack
                            .last()
                            .expect("missing scope while collecting function identity");
                        let child_scope = parent_scope.tree.scope_for_def(func).ok();
                        let qualname = if is_module_init_temp_name(raw_bind_name.as_str()) {
                            "_dp_module_init".to_string()
                        } else {
                            child_scope
                                .as_ref()
                                .map(|scope| {
                                    normalize_qualname(
                                        scope.qualnamer.qualname.as_str(),
                                        bind_name.as_str(),
                                        display_name.as_str(),
                                    )
                                })
                                .unwrap_or_else(|| bind_name.clone())
                        };
                        self.out.insert(
                            node_index,
                            FunctionIdentity {
                                bind_name: bind_name.clone(),
                                display_name,
                                qualname,
                                binding_target: binding_target_for_scope(
                                    parent_scope.as_ref(),
                                    raw_bind_name.as_str(),
                                ),
                            },
                        );
                        if let Some(child_scope) = child_scope {
                            self.scope_stack.push(child_scope);
                            walk_stmt(self, stmt);
                            self.scope_stack.pop();
                            return;
                        }
                    }
                    walk_stmt(self, stmt);
                }
                Stmt::ClassDef(class_def) => {
                    let parent_scope = self
                        .scope_stack
                        .last()
                        .expect("missing scope while collecting class scope");
                    if let Ok(child_scope) = parent_scope.tree.scope_for_def(class_def) {
                        self.scope_stack.push(child_scope);
                        walk_stmt(self, stmt);
                        self.scope_stack.pop();
                        return;
                    }
                    walk_stmt(self, stmt);
                }
                _ => walk_stmt(self, stmt),
            }
        }
    }

    let mut module = module.clone();
    let mut collector = Collector {
        scope_stack: vec![module_scope.clone()],
        out: HashMap::new(),
    };
    collector.visit_body(&mut module);
    collector.out
}

pub(super) fn split_docstring(body: &StmtBody) -> (Option<Stmt>, Vec<Box<Stmt>>) {
    let mut rest = body.body.clone();
    let Some(first) = rest.first() else {
        return (None, rest);
    };
    if matches!(
        first.as_ref(),
        Stmt::Expr(ast::StmtExpr { value, .. }) if matches!(value.as_ref(), Expr::StringLiteral(_))
    ) {
        let first_stmt = *rest.remove(0);
        return (Some(first_stmt), rest);
    }
    (None, rest)
}

pub(super) fn function_docstring_expr(func: &ast::StmtFunctionDef) -> Option<Expr> {
    let (docstring, _) = split_docstring(&func.body);
    let Some(Stmt::Expr(expr_stmt)) = docstring else {
        return None;
    };
    Some(*expr_stmt.value)
}

pub(super) fn function_annotation_entries(
    func: &ast::StmtFunctionDef,
) -> Vec<(String, Expr, String)> {
    let mut entries = Vec::new();
    let parameters = func.parameters.as_ref();

    for param in &parameters.posonlyargs {
        if let Some(annotation) = param.parameter.annotation.as_ref() {
            entries.push((
                param.parameter.name.id.to_string(),
                *annotation.clone(),
                annotation_expr_string(annotation),
            ));
        }
    }
    for param in &parameters.args {
        if let Some(annotation) = param.parameter.annotation.as_ref() {
            entries.push((
                param.parameter.name.id.to_string(),
                *annotation.clone(),
                annotation_expr_string(annotation),
            ));
        }
    }
    if let Some(vararg) = &parameters.vararg {
        if let Some(annotation) = vararg.annotation.as_ref() {
            entries.push((
                vararg.name.id.to_string(),
                *annotation.clone(),
                annotation_expr_string(annotation),
            ));
        }
    }
    for param in &parameters.kwonlyargs {
        if let Some(annotation) = param.parameter.annotation.as_ref() {
            entries.push((
                param.parameter.name.id.to_string(),
                *annotation.clone(),
                annotation_expr_string(annotation),
            ));
        }
    }
    if let Some(kwarg) = &parameters.kwarg {
        if let Some(annotation) = kwarg.annotation.as_ref() {
            entries.push((
                kwarg.name.id.to_string(),
                *annotation.clone(),
                annotation_expr_string(annotation),
            ));
        }
    }
    if let Some(returns) = func.returns.as_ref() {
        entries.push((
            "return".to_string(),
            *returns.clone(),
            annotation_expr_string(returns),
        ));
    }

    entries
}

fn annotation_expr_string(expr: &Expr) -> String {
    Generator::new(&Indentation::new("    ".to_string()), LineEnding::default()).expr(expr)
}

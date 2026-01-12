pub mod rewrite_annotation;
pub mod rewrite_class_vars;
pub mod rewrite_method;
pub mod rewrite_private;

use crate::template::make_tuple;
use crate::{py_expr, py_stmt};
use ruff_python_ast::{
    self as ast, Arguments, Expr, Identifier, Stmt, StmtClassDef, TypeParam, TypeParamParamSpec,
    TypeParamTypeVar, TypeParamTypeVarTuple, TypeParams,
};
use ruff_text_size::TextRange;

use crate::body_transform::{walk_expr, walk_stmt, Transformer};
use crate::template::py_stmt_single;
use crate::transform::class_def::rewrite_annotation::AnnotationCollector;
use crate::transform::class_def::rewrite_class_vars::rewrite_class_scope;
use crate::transform::class_def::rewrite_method::rewrite_method;
use crate::transform::context::ScopeKind;
use crate::transform::driver::{ExprRewriter, Rewrite};

use std::mem::take;

fn rewrite_methods_in_class_body(
    body: &mut Vec<Stmt>,
    class_qualname: &str,
    rewriter: &mut ExprRewriter,
) -> bool {
    let mut rewriter = MethodRewriter {
        class_qualname: class_qualname.to_string(),
        expr_rewriter: rewriter,
        needs_class_cell: false,
    };
    rewriter.visit_body(body);
    rewriter.needs_class_cell
}

struct MethodQualnameRewriter {
    class_qualname: String,
}

impl Transformer for MethodQualnameRewriter {
    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        match stmt {
            Stmt::FunctionDef(_) | Stmt::ClassDef(_) => return,
            _ => walk_stmt(self, stmt),
        }
    }

    fn visit_expr(&mut self, expr: &mut Expr) {
        if let Expr::Call(ast::ExprCall {
            func, arguments, ..
        }) = expr
        {
            let is_update_fn = match func.as_ref() {
                Expr::Attribute(ast::ExprAttribute { value, attr, .. }) => {
                    if let Expr::Name(ast::ExprName { id, .. }) = value.as_ref() {
                        id.as_str() == "__dp__" && attr.as_str() == "update_fn"
                    } else {
                        false
                    }
                }
                Expr::Name(ast::ExprName { id, .. }) => id.as_str() == "update_fn",
                _ => false,
            };

            if is_update_fn && arguments.args.len() >= 2 {
                arguments.args[1] = py_expr!(
                    "{qualname:literal}",
                    qualname = self.class_qualname.as_str()
                );
            }
        }
        walk_expr(self, expr);
    }
}

struct MethodRewriter<'a> {
    class_qualname: String,
    expr_rewriter: &'a mut ExprRewriter,
    needs_class_cell: bool,
}

impl<'a> Transformer for MethodRewriter<'a> {
    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        match stmt {
            Stmt::FunctionDef(func_def) => {
                let fn_name = func_def.name.id.to_string();
                assert!(
                    func_def.decorator_list.is_empty(),
                    "decorators should be gone by now"
                );
                assert!(fn_name.starts_with("_dp_"), "function name should start with _dp_");
                if let Some(original_name) =
                    fn_name.strip_prefix("_dp_fn_")
                {
                    self.needs_class_cell |= rewrite_method(
                        func_def,
                        &self.class_qualname,
                        original_name,
                        self.expr_rewriter,
                    );
                }
            }
            Stmt::ClassDef(_) => {}
            _ => walk_stmt(self, stmt),
        }
    }
}

pub struct NestedClassCollector<'a> {
    rewriter: &'a mut ExprRewriter,
    nested: Vec<Stmt>,
}

impl<'a> NestedClassCollector<'a> {
    pub fn new(rewriter: &'a mut ExprRewriter) -> Self {
        Self {
            rewriter,
            nested: Vec::new(),
        }
    }

    pub fn into_nested(self) -> Vec<Stmt> {
        self.nested
    }
}

impl<'a> Transformer for NestedClassCollector<'a> {
    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        if let Stmt::FunctionDef(_) = stmt {
            // Don't recurse into functions
            return;
        }

        *stmt = if let Stmt::ClassDef(ast::StmtClassDef {
            name,
            body,
            arguments,
            type_params,
            decorator_list,
            ..
        }) = stmt
        {
            let class_name = name.id.to_string();
            let class_qualname = self.rewriter.context().make_qualname(&class_name);
            let class_ident = class_ident_from_qualname(&class_qualname);

            let mut decorated = py_expr!(
                "_dp_create_class_{class_ident:id}()",
                class_ident = class_ident,
            );
            let to_apply = take(decorator_list)
                .into_iter()
                .map(|decorator| self.rewriter.maybe_placeholder(decorator.expression))
                .collect::<Vec<_>>();
            for decorator in to_apply.into_iter().rev() {
                decorated = py_expr!(
                    "{decorator:expr}({decorated:expr})",
                    decorator = decorator,
                    decorated = decorated
                );
            }
            let ns_fn_stmt = py_stmt!(
                "{class_name:id} = {decorated:expr}",
                class_name = class_name.as_str(),
                decorated = decorated,
            );

            // TODO: make better
            let create_stmt = Stmt::If(ast::StmtIf {
                node_index: ast::AtomicNodeIndex::default(),
                range: TextRange::default(),
                test: Box::new(py_expr!("True")),
                body: ns_fn_stmt,
                elif_else_clauses: Vec::new(),
            });

            let create_class_fn = class_def_to_create_class_fn(
                name,
                take(body),
                take(arguments),
                take(type_params),
                class_qualname,
                self.rewriter,
            );
            self.nested.extend(create_class_fn);

            create_stmt
        } else {
            walk_stmt(self, stmt);
            return;
        }
    }
}

pub fn rewrite<'a>(class_def: StmtClassDef, rewriter: &'a mut ExprRewriter) -> Rewrite {
    let mut class_def_stmt = Stmt::ClassDef(class_def);

    let mut nested_classes = {
        let mut nested_collector = NestedClassCollector::new(rewriter);
        nested_collector.visit_stmt(&mut class_def_stmt);
        nested_collector.into_nested()
    };
    nested_classes.push(class_def_stmt);

    Rewrite::Visit(nested_classes)
}

fn class_def_to_create_class_fn<'a>(
    name: &Identifier,
    mut body: Vec<Stmt>,
    arguments: Option<Box<Arguments>>,
    type_params: Option<Box<TypeParams>>,
    class_qualname: String,
    rewriter: &'a mut ExprRewriter,
) -> Vec<Stmt> {
    let class_name = name.id.to_string();
    let class_ident = class_ident_from_qualname(&class_qualname);

    let mut class_scope = rewriter
        .context()
        .analyze_class_scope(&class_qualname, &body);

    rewrite_private::rewrite_class_body(&mut body, &class_name, &mut class_scope);

    let body = rewriter.with_function_scope(class_scope.clone(), |rewriter| {
        /*
        If the first statement is a string literal, assign it to  __doc__
        */
        if let Some(first_stmt) = body.first_mut() {
            if let Stmt::Expr(ast::StmtExpr { value, .. }) = first_stmt {
                if let Expr::StringLiteral(_) = value.as_ref() {
                    let doc_expr = (*value).clone();
                    *first_stmt =
                        py_stmt_single(py_stmt!("__doc__ = {value:expr}", value = doc_expr));
                }
            }
        }

        /*
        Collect all AnnAssign statements, rewriting them to bare Assign (if there's a value)
        or removing (if not).  Assign the annotations to __annotations__
        */
        let annotations = AnnotationCollector::collect(&mut body);

        let mut annotation_stmt = py_stmt!("__annotations__ = {}");
        for (name, annotation) in annotations {
            annotation_stmt.extend(py_stmt!(
                "__annotations__[{name:literal}] = {annotation:expr}",
                name = name.as_str(),
                annotation = annotation,
            ));
        }

        let type_param_statements = if let Some(type_params) = type_params {
            make_type_param_statements(*type_params, rewriter)
        } else {
            vec![]
        };

        let ns_builder = py_stmt!(
            r#"
__module__ = __name__
__qualname__ = {class_qualname:literal}
{type_param_statements:stmt}
{annotations:stmt}
{ns_body:stmt}
"#,
            class_ident = class_ident.as_str(),
            class_qualname = class_qualname.as_str(),
            ns_body = body,
            type_param_statements = type_param_statements,
            annotations = annotation_stmt,
        );

        let mut ns_builder = rewriter.rewrite_block(ns_builder);

        let needs_class_cell = rewrite_methods_in_class_body(
            &mut ns_builder,
            &class_qualname,
            rewriter,
        );

        let mut method_qualname_rewriter = MethodQualnameRewriter {
            class_qualname: class_qualname.clone(),
        };
        method_qualname_rewriter.visit_body(&mut ns_builder);

        rewrite_class_scope(&mut ns_builder, class_scope);

        if needs_class_cell {
            ns_builder = py_stmt!(
                r#"
__class__ = __dp__.make_classcell()
_dp_class_ns.__classcell__ = __class__
{ns_builder:stmt}
"#, ns_builder = ns_builder,
            );
        }


        ns_builder
    });

    let in_class_scope = matches!(
        rewriter.context().current_qualname(),
        Some((_, ScopeKind::Class))
    );
    let (bases_tuple, prepare_dict) = class_call_arguments(arguments, in_class_scope);

    py_stmt!(
        r#"
def _dp_create_class_{class_ident:id}():
    def _dp_ns_builder(_dp_class_ns):
        {ns_body:stmt}
    return __dp__.create_class({class_name:literal}, _dp_ns_builder, {bases:expr}, {prepare_dict:expr})
"#,
        class_ident = class_ident.as_str(),
        ns_body = body,
        class_name = class_name.as_str(),
        bases = bases_tuple.clone(),
        prepare_dict = prepare_dict.clone(),
    )
}

fn make_type_param_statements(
    mut type_params: ast::TypeParams,
    rewriter: &mut ExprRewriter,
) -> Vec<Stmt> {
    rewriter.visit_type_params(&mut type_params);

    let mut statements = Vec::new();
    let mut param_names = Vec::new();

    for type_param in type_params.type_params {
        match type_param {
            TypeParam::TypeVar(TypeParamTypeVar {
                name,
                bound,
                default,
                ..
            }) => {
                let param_name = name.as_str().to_string();
                let (constraints, bound_expr) = match bound {
                    Some(expr) => match *expr {
                        Expr::Tuple(ast::ExprTuple { elts, .. }) => (Some(make_tuple(elts)), None),
                        other => (None, Some(other)),
                    },
                    None => (None, None),
                };
                let default_expr = default.map(|expr| *expr);

                let bound_expr = bound_expr.unwrap_or_else(|| py_expr!("None"));
                let default_expr = default_expr.unwrap_or_else(|| py_expr!("None"));
                let constraints_expr = constraints.unwrap_or_else(|| py_expr!("None"));

                statements.extend(py_stmt!(
                    "{name:id} = __dp__.type_param_typevar({name_literal:literal}, {bound:expr}, {default:expr}, {constraints:expr})",
                    name = param_name.as_str(),
                    name_literal = param_name.as_str(),
                    bound = bound_expr,
                    default = default_expr,
                    constraints = constraints_expr,
                ));
                param_names.push(param_name);
            }
            TypeParam::TypeVarTuple(TypeParamTypeVarTuple { name, default, .. }) => {
                let param_name = name.as_str().to_string();
                let default_expr = default
                    .map(|expr| *expr)
                    .unwrap_or_else(|| py_expr!("None"));

                statements.extend(py_stmt!(
                    "{name:id} = __dp__.type_param_typevar_tuple({name_literal:literal}, {default:expr})",
                    name = param_name.as_str(),
                    name_literal = param_name.as_str(),
                    default = default_expr,
                ));
                param_names.push(param_name);
            }
            TypeParam::ParamSpec(TypeParamParamSpec { name, default, .. }) => {
                let param_name = name.as_str().to_string();
                let default_expr = default
                    .map(|expr| *expr)
                    .unwrap_or_else(|| py_expr!("None"));

                statements.extend(py_stmt!(
                    "{name:id} = __dp__.type_param_param_spec({name_literal:literal}, {default:expr})",
                    name = param_name.as_str(),
                    name_literal = param_name.as_str(),
                    default = default_expr,
                ));
                param_names.push(param_name);
            }
        }
    }

    if !param_names.is_empty() {
        let tuple_expr = make_tuple(
            param_names
                .iter()
                .map(|name| py_expr!("{name:id}", name = name.as_str()))
                .collect(),
        );
        statements.extend(py_stmt!(
            "__type_params__ = {tuple:expr}",
            tuple = tuple_expr
        ));
    }

    statements
}

pub fn class_ident_from_qualname(qualname: &str) -> String {
    qualname
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

pub fn class_call_arguments(
    arguments: Option<Box<ast::Arguments>>,
    in_class_scope: bool,
) -> (Expr, Expr) {
    let mut bases = Vec::new();
    let mut kw_items = Vec::new();
    if let Some(args) = arguments {
        let args = *args;
        for base in args.args.into_vec() {
            let base_expr = if in_class_scope {
                if let Expr::Name(ast::ExprName { id, .. }) = &base {
                    py_expr!(
                        "__dp__.class_lookup({name:literal}, _dp_class_ns, lambda: {name:id})",
                        name = id.as_str()
                    )
                } else {
                    base
                }
            } else {
                base
            };
            bases.push(base_expr);
        }
        for kw in args.keywords.into_vec() {
            let value = if in_class_scope {
                if let Expr::Name(ast::ExprName { id, .. }) = &kw.value {
                    py_expr!(
                        "__dp__.class_lookup({name:literal}, _dp_class_ns, lambda: {name:id})",
                        name = id.as_str()
                    )
                } else {
                    kw.value
                }
            } else {
                kw.value
            };
            let key = kw
                .arg
                .map(|arg| py_expr!("{arg:literal}", arg = arg.as_str()));
            kw_items.push(ast::DictItem { key, value });
        }
    }

    let has_kw = !kw_items.is_empty();

    let prepare_dict = if has_kw {
        Expr::Dict(ast::ExprDict {
            node_index: ast::AtomicNodeIndex::default(),
            range: TextRange::default(),
            items: kw_items,
        })
    } else {
        py_expr!("None")
    };

    (make_tuple(bases), prepare_dict)
}

#[cfg(test)]
mod tests {
    crate::transform_fixture_test!("tests_rewrite_class_def.txt");
}

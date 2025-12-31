pub mod rewrite_annotation;
pub mod rewrite_class_vars;
pub mod rewrite_method;
pub mod rewrite_nested_class;
pub mod rewrite_private;

use crate::template::make_tuple;
use crate::{py_expr, py_stmt};
use ruff_python_ast::{
    self as ast, Arguments, Expr, Identifier, Stmt, StmtClassDef, TypeParam, TypeParamParamSpec,
    TypeParamTypeVar, TypeParamTypeVarTuple, TypeParams,
};
use ruff_text_size::TextRange;

use crate::body_transform::Transformer;
use crate::template::py_stmt_single;
use crate::transform::class_def::rewrite_annotation::AnnotationCollector;
use crate::transform::class_def::rewrite_class_vars::rewrite_class_scope;
use crate::transform::class_def::rewrite_method::rewrite_method;
use crate::transform::driver::{ExprRewriter, Rewrite};

use std::mem::take;

use crate::{body_transform::walk_stmt, transform::rewrite_decorator};

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

            let create_class_call = py_stmt!(
                "{class_name:id} = _dp_create_class_{class_ident:id}()",
                class_ident = class_ident,
                class_name = class_name.as_str(),
            );

            let ns_fn_stmt = rewrite_decorator::rewrite(
                take(decorator_list),
                &class_name,
                create_class_call,
                self.rewriter,
            )
            .into_statements();

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

    let class_scope = rewriter
        .context()
        .analyze_class_scope(&class_qualname, &body);

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

        rewrite_private::rewrite_class_body(&mut body, &class_name);

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

        let ns_builder = rewriter.rewrite_block(ns_builder);

        let mut ns_builder = ns_builder
            .into_iter()
            .map(|stmt| match stmt {
                Stmt::FunctionDef(mut func_def) => {
                    let fn_name = func_def.name.id.to_string();

                    rewrite_method(
                        &mut func_def,
                        &class_name,
                        fn_name.as_str(),
                        &class_scope.locals,
                        rewriter,
                    );

                    assert!(
                        func_def.decorator_list.is_empty(),
                        "decorators should be gone by now"
                    );

                    Stmt::FunctionDef(func_def)
                }
                other => other,
            })
            .collect();

        rewrite_class_scope(&mut ns_builder, class_scope);

        ns_builder
    });

    let (bases_tuple, prepare_dict) = class_call_arguments(arguments);

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

pub fn class_call_arguments(arguments: Option<Box<ast::Arguments>>) -> (Expr, Expr) {
    let mut bases = Vec::new();
    let mut kw_keys = Vec::new();
    let mut kw_vals = Vec::new();
    if let Some(args) = arguments {
        let args = *args;
        bases.extend(args.args.into_vec());
        for kw in args.keywords.into_vec() {
            if let Some(arg) = kw.arg {
                kw_keys.push(py_expr!("{arg:literal}", arg = arg.as_str()));
                kw_vals.push(kw.value);
            }
        }
    }

    let has_kw = !kw_keys.is_empty();

    let prepare_dict = if has_kw {
        let items: Vec<ast::DictItem> = kw_keys
            .into_iter()
            .zip(kw_vals.into_iter())
            .map(|(k, v)| ast::DictItem {
                key: Some(k),
                value: v,
            })
            .collect();
        Expr::Dict(ast::ExprDict {
            node_index: ast::AtomicNodeIndex::default(),
            range: TextRange::default(),
            items,
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

pub mod rewrite_annotation;
pub mod rewrite_class_vars;
pub mod rewrite_method;
pub mod rewrite_nested_class;

use crate::template::make_tuple;
use crate::{py_expr, py_stmt};
use ruff_python_ast::{self as ast, Expr, Stmt};
use ruff_text_size::TextRange;

use crate::body_transform::Transformer;
use crate::template::py_stmt_single;
use crate::transform::class_def::rewrite_annotation::AnnotationCollector;
use crate::transform::class_def::rewrite_class_vars::ClassVarRenamer;
use crate::transform::class_def::rewrite_method::rewrite_method;
use crate::transform::class_def::rewrite_nested_class::NestedClassCollector;
use crate::transform::driver::{ExprRewriter, Rewrite};
use crate::transform::rewrite_decorator;

pub fn rewrite(
    ast::StmtClassDef {
        name,
        mut body,
        arguments,
        ..
    }: ast::StmtClassDef,
    decorators: Vec<ast::Decorator>,
    rewriter: &mut ExprRewriter,
    qualname: Option<String>,
) -> Rewrite {
    let class_name = name.id.as_str().to_string();
    let class_qualname = qualname.unwrap_or_else(|| class_name.clone());
    let dp_class_name = class_ident_from_qualname(&class_qualname);
    let class_ident = dp_class_name
        .strip_prefix("_dp_class_")
        .expect("dp class names are prefixed")
        .to_string();

    /*
     Lift nested classes out of the class body
    */
    let mut nested_collector = NestedClassCollector::new(class_qualname.clone());
    nested_collector.visit_body(&mut body);
    let nested_classes = nested_collector.into_nested();

    /*
    If the first statement is a string literal, assign it to  __doc__
    */
    if let Some(first_stmt) = body.first_mut() {
        if let Stmt::Expr(ast::StmtExpr { value, .. }) = first_stmt {
            if let Expr::StringLiteral(_) = value.as_ref() {
                let doc_expr = (*value).clone();
                *first_stmt = py_stmt_single(py_stmt!("__doc__ = {value:expr}", value = doc_expr));
            }
        }
    }

    body.extend(py_stmt!(
        r#"
__module__ = __name__
__qualname__ = {class_qualname:literal}
"#,
        class_qualname = class_qualname.as_str(),
    ));

    let mut body = rewriter.rewrite_block(body);

    /*
    Collect all AnnAssign statements, rewriting them to bare Assign (if there's a value)
    or removing (if not).  Assign the annotations to __annotations__
    */
    let annotations = AnnotationCollector::collect(&mut body);
    if !annotations.is_empty() {
        body.extend(py_stmt!(
            r#"
_dp_class_annotations = _dp_class_ns.get("__annotations__")
if _dp_class_annotations is None:
    _dp_class_annotations = __dp__.dict()
__annotations__ = _dp_class_annotations
"#
        ));

        for (_, name, annotation) in annotations {
            body.extend(py_stmt!(
                "_dp_class_annotations[{name:literal}] = {annotation:expr}",
                name = name.as_str(),
                annotation = annotation,
            ));
        }
    }

    let mut body = body
        .into_iter()
        .map(|stmt| match stmt {
            Stmt::FunctionDef(mut func_def) => {
                let fn_name = func_def.name.id.to_string();

                rewrite_method(
                    &mut func_def,
                    &class_name,
                    &class_qualname,
                    fn_name.as_str(),
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

    let mut renamer = ClassVarRenamer::new();
    renamer.visit_body(&mut body);

    let (bases_tuple, prepare_dict) = class_call_arguments(arguments);

    let mut class_statements = py_stmt!(
        r#"
def _dp_ns_{class_ident:id}(_dp_class_ns):
    {ns_body:stmt}
"#,
        class_ident = class_ident.as_str(),
        ns_body = body,
    );

    class_statements.push(py_stmt_single(py_stmt!(
        "{class_name:id} = __dp__.create_class({class_name:literal}, _dp_ns_{class_ident:id}, {bases:expr}, {prepare_dict:expr})",
        class_ident = class_ident.as_str(),
        class_name = class_name.as_str(),
        bases = bases_tuple.clone(),
        prepare_dict = prepare_dict.clone(),
    )));

    let mut ns_fn_stmt =
        rewrite_decorator::rewrite(decorators, &class_name.as_str(), class_statements, rewriter)
            .into_statements();

    let class_assignment = ns_fn_stmt
        .pop()
        .expect("class creation statement should be last");

    let mut pending_dels = Vec::new();

    for (_, nested_class_def) in nested_classes {
        let nested_name = nested_class_def.name.id.to_string();
        let nested_qualname = format!("{class_qualname}.{nested_name}");

        let mut nested_stmts = rewrite(
            nested_class_def,
            Vec::new(),
            rewriter,
            Some(nested_qualname),
        )
        .into_statements();

        let mut nested_dels = Vec::new();
        while matches!(nested_stmts.last(), Some(Stmt::Delete(_))) {
            nested_dels.push(nested_stmts.pop().expect("expected delete statement"));
        }

        nested_dels.reverse();
        pending_dels.extend(nested_dels);

        nested_stmts.retain(|stmt| match stmt {
            Stmt::Assign(ast::StmtAssign { value, .. }) => !is_create_class_call(value),
            _ => true,
        });

        ns_fn_stmt.extend(nested_stmts);
    }

    ns_fn_stmt.push(class_assignment);
    ns_fn_stmt.extend(pending_dels);
    ns_fn_stmt.extend(py_stmt!(
        "del _dp_ns_{class_ident:id}",
        class_ident = class_ident.as_str()
    ));

    Rewrite::Visit(ns_fn_stmt)
}

pub fn class_ident_from_qualname(qualname: &str) -> String {
    let sanitized: String = qualname
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect();
    format!("_dp_class_{}", sanitized)
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

fn is_create_class_call(expr: &Expr) -> bool {
    if let Expr::Call(ast::ExprCall { func, .. }) = expr {
        if let Expr::Attribute(ast::ExprAttribute { value, attr, .. }) = func.as_ref() {
            if let Expr::Name(ast::ExprName { id, .. }) = value.as_ref() {
                return id.as_str() == "__dp__" && attr.as_str() == "create_class";
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use crate::test_util::assert_transform_eq;

    #[test]
    fn rewrites_without_first_parameter_for_super() {
        assert_transform_eq(
            r#"
class C:
    def m():
        return super().m()
"#,
            r#"
def _dp_ns_C(_dp_class_ns):
    def m():
        return super(C, None).m()
    __dp__.setitem(_dp_class_ns, "m", m)
    __dp__.setitem(_dp_class_ns, "__module__", __name__)
    __dp__.setitem(_dp_class_ns, "__qualname__", "C")
C = __dp__.create_class("C", _dp_ns_C, (), None)
del _dp_ns_C
"#,
        );
    }

    crate::transform_fixture_test!("tests_rewrite_class_def.txt");
}

pub mod rewrite_annotation;
pub mod rewrite_class_vars;
pub mod rewrite_method;
pub mod rewrite_private;

use crate::transform::{rewrite_decorator, rewrite_stmt};
use crate::transform::rewrite_expr::make_tuple;
use crate::{py_expr, py_stmt};
use ruff_python_ast::{
    self as ast, Arguments, Expr, Identifier, Stmt, StmtClassDef, TypeParam, TypeParamParamSpec,
    TypeParamTypeVar, TypeParamTypeVarTuple, TypeParams,
};
use ruff_text_size::TextRange;

use crate::body_transform::{walk_stmt, Transformer};
use crate::template::py_stmt_single;
use crate::transform::class_def::rewrite_annotation::AnnotationCollector;
use crate::transform::class_def::rewrite_class_vars::rewrite_class_scope;
use crate::transform::class_def::rewrite_method::rewrite_method;
use crate::transform::context::ScopeKind;
use crate::transform::driver::{ExprRewriter, Rewrite};

use std::collections::HashSet;
use std::mem::take;

pub fn class_lookup_literal_name(name: &str) -> &str {
    if let Some((base, suffix)) = name.rsplit_once('$') {
        if !base.is_empty() && suffix.chars().all(|ch| ch.is_ascii_digit()) {
            return base;
        }
    }
    name
}

fn rewrite_methods_in_class_body(
    body: &mut Vec<Stmt>,
    class_qualname: &str,
    rewriter: &mut ExprRewriter,
    class_cell_name: &str,
) -> bool {
    let mut rewriter = MethodRewriter {
        class_qualname: class_qualname.to_string(),
        expr_rewriter: rewriter,
        needs_class_cell: false,
        class_cell_name: class_cell_name.to_string(),
    };
    rewriter.visit_body(body);
    rewriter.needs_class_cell
}


struct MethodRewriter<'a> {
    class_qualname: String,
    expr_rewriter: &'a mut ExprRewriter,
    needs_class_cell: bool,
    class_cell_name: String,
}

impl<'a> Transformer for MethodRewriter<'a> {
    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        match stmt {
            Stmt::FunctionDef(func_def) => {
                assert!(
                    func_def.decorator_list.is_empty(),
                    "decorators should be gone by now"
                );
                self.needs_class_cell |= rewrite_method(
                    func_def,
                    &self.class_qualname,
                    self.expr_rewriter,
                    &self.class_cell_name,
                );
            }
            Stmt::ClassDef(_) => {}
            _ => walk_stmt(self, stmt),
        }
    }
}



pub fn rewrite(ast::StmtClassDef {
    name,
    mut body,
    mut arguments,
    mut type_params,
    decorator_list,
    ..
}: StmtClassDef, rewriter: &mut ExprRewriter) -> Rewrite {
    let class_name = name.id.to_string();
    let class_qualname = rewriter.context().make_qualname(&class_name);


    let create_class_fn = class_def_to_create_class_fn(
        &name,
        take(&mut body),
        take(&mut arguments),
        take(&mut type_params),
        class_qualname.to_owned(),
        rewriter,
    );

    rewrite_stmt::decorator::rewrite(decorator_list, class_name.as_str(), create_class_fn, rewriter)
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

    let mut class_scope = rewriter
        .context()
        .analyze_class_scope(&class_qualname, &body);

    rewrite_private::rewrite_class_body(&mut body, &class_name, &mut class_scope);
    let class_locals = class_scope.local_names();
    let has_enclosing_class_cell = rewriter.context().has_enclosing_method_class_cell();
    let class_cell_name = if has_enclosing_class_cell
        || class_scope.nonlocal_names().contains("__class__")
    {
        rewriter.context().fresh("_dp_classcell")
    } else {
        "_dp_classcell".to_string()
    };

    let (body, type_param_info) = rewriter.with_scope(class_scope.clone(), |rewriter| {
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
        or removing (if not). Build a deferred __annotate__ function for class annotations.
        */
        let annotation_entries = AnnotationCollector::rewrite(&mut body);
        let annotation_stmt = build_class_annotate_stmt(annotation_entries);

        let type_param_info = type_params.map(|type_params| {
            make_type_param_info(*type_params, rewriter)
        });
        let type_param_statements = type_param_info
            .as_ref()
            .and_then(|info| info.type_params_tuple.as_ref())
            .map(|tuple_expr| {
                py_stmt!(
                    "__type_params__ = {tuple:expr}",
                    tuple = tuple_expr.clone()
                )
            })
            .unwrap_or_default();

        let ns_builder = py_stmt!(
            r#"
__module__ = __name__
__qualname__ = {class_qualname:literal}
{type_param_statements:stmt}
{annotations:stmt}
{ns_body:stmt}
"#,
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
            &class_cell_name,
        );

        let type_param_skip = type_param_info
            .as_ref()
            .map(|info| {
                info.param_names
                    .iter()
                    .filter(|name| !class_locals.contains(*name))
                    .cloned()
                    .collect::<HashSet<String>>()
            })
            .unwrap_or_default();

        rewrite_class_scope(
            class_qualname.clone(),
            &mut ns_builder,
            class_scope,
            type_param_skip,
            has_enclosing_class_cell,
        );

        if needs_class_cell {
            let mut insert_pos = 0;
            for (idx, stmt) in ns_builder.iter().enumerate() {
                let names = match stmt {
                    Stmt::Global(ast::StmtGlobal { names, .. })
                    | Stmt::Nonlocal(ast::StmtNonlocal { names, .. }) => names,
                    _ => continue,
                };
                if names.iter().any(|name| name.id.as_str() == "__class__") {
                    insert_pos = idx + 1;
                }
            }
            let classcell = py_stmt!(
                r#"
{class_cell:id} = __dp__.empty_classcell()
_dp_class_ns.__classcell__ = (lambda: {class_cell:id}).__closure__[0]
"#,
                class_cell = class_cell_name,
            );
            ns_builder.splice(insert_pos..insert_pos, classcell);
        }


        (ns_builder, type_param_info)
    });

    let type_param_bindings = type_param_info
        .as_ref()
        .map_or_else(Vec::new, |info| info.bindings.clone());
    let in_class_scope = matches!(
        rewriter.context().current_qualname(),
        Some((_, ScopeKind::Class))
    );
    let has_generic_base = arguments_has_generic(arguments.as_deref());
    let generic_base = type_param_info
        .as_ref()
        .filter(|_| !has_generic_base)
        .and_then(make_generic_base);
    let extra_bases = generic_base.into_iter().collect::<Vec<_>>();
    let (bases_tuple, prepare_dict) =
        class_call_arguments(arguments, in_class_scope, extra_bases);

    py_stmt!(
        r#"
def _dp_ns_{class_name:id}(_dp_class_ns):
    {type_param_bindings:stmt}
    {ns_body:stmt}
{class_name:id} = __dp__.create_class({class_name:literal}, _dp_ns_{class_name:id}, {bases:expr}, {prepare_dict:expr})
"#,
        class_name = class_name.as_str(),
        type_param_bindings = type_param_bindings,
        ns_body = body,
        bases = bases_tuple.clone(),
        prepare_dict = prepare_dict.clone(),
    )
}

fn build_class_annotate_stmt(entries: Vec<(String, Expr)>) -> Vec<Stmt> {
    if entries.is_empty() {
        return Vec::new();
    }

    let mut annotation_writes = Vec::new();
    for (name, expr) in entries {
        annotation_writes.extend(py_stmt!(
            "_dp_annotations[{name:literal}] = {value:expr}",
            name = name.as_str(),
            value = expr,
        ));
    }

    py_stmt!(
        r#"
def _dp_annotate(_dp_format):
    if _dp_format > 2:
        raise NotImplementedError
    _dp_annotations = {}
    {annotation_writes:stmt}
    return _dp_annotations
__annotate__ = _dp_annotate
"#,
        annotation_writes = annotation_writes,
    )
}

struct TypeParamInfo {
    bindings: Vec<Stmt>,
    param_names: Vec<String>,
    type_params_tuple: Option<Expr>,
    generic_params: Vec<Expr>,
}

fn make_type_param_info(
    mut type_params: ast::TypeParams,
    rewriter: &mut ExprRewriter,
) -> TypeParamInfo {
    rewriter.visit_type_params(&mut type_params);

    let mut bindings = Vec::new();
    let mut param_names = Vec::new();
    let mut type_param_exprs = Vec::new();
    let mut generic_params = Vec::new();

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

                bindings.extend(py_stmt!(
                    "{name:id} = __dp__.type_param_typevar({name_literal:literal}, {bound:expr}, {default:expr}, {constraints:expr})",
                    name = param_name.as_str(),
                    name_literal = param_name.as_str(),
                    bound = bound_expr,
                    default = default_expr,
                    constraints = constraints_expr,
                ));
                type_param_exprs.push(py_expr!("{name:id}", name = param_name.as_str()));
                generic_params.push(py_expr!("{name:id}", name = param_name.as_str()));
                param_names.push(param_name);
            }
            TypeParam::TypeVarTuple(TypeParamTypeVarTuple { name, default, .. }) => {
                let param_name = name.as_str().to_string();
                let default_expr = default
                    .map(|expr| *expr)
                    .unwrap_or_else(|| py_expr!("None"));

                bindings.extend(py_stmt!(
                    "{name:id} = __dp__.type_param_typevar_tuple({name_literal:literal}, {default:expr})",
                    name = param_name.as_str(),
                    name_literal = param_name.as_str(),
                    default = default_expr,
                ));
                type_param_exprs.push(py_expr!("{name:id}", name = param_name.as_str()));
                generic_params.push(py_expr!(
                    "__dp__.getitem(__import__(\"typing\").Unpack, {name:id})",
                    name = param_name.as_str()
                ));
                param_names.push(param_name);
            }
            TypeParam::ParamSpec(TypeParamParamSpec { name, default, .. }) => {
                let param_name = name.as_str().to_string();
                let default_expr = default
                    .map(|expr| *expr)
                    .unwrap_or_else(|| py_expr!("None"));

                bindings.extend(py_stmt!(
                    "{name:id} = __dp__.type_param_param_spec({name_literal:literal}, {default:expr})",
                    name = param_name.as_str(),
                    name_literal = param_name.as_str(),
                    default = default_expr,
                ));
                type_param_exprs.push(py_expr!("{name:id}", name = param_name.as_str()));
                generic_params.push(py_expr!("{name:id}", name = param_name.as_str()));
                param_names.push(param_name);
            }
        }
    }

    let type_params_tuple = if type_param_exprs.is_empty() {
        None
    } else {
        Some(make_tuple(type_param_exprs))
    };

    TypeParamInfo {
        bindings,
        param_names,
        type_params_tuple,
        generic_params,
    }
}

fn make_generic_base(info: &TypeParamInfo) -> Option<Expr> {
    if info.generic_params.is_empty() {
        return None;
    }
    let params_expr = if info.generic_params.len() == 1 {
        info.generic_params[0].clone()
    } else {
        make_tuple(info.generic_params.clone())
    };
    let generic_expr = py_expr!("__import__(\"typing\").Generic");
    Some(py_expr!(
        "__dp__.getitem({generic:expr}, {params:expr})",
        generic = generic_expr,
        params = params_expr,
    ))
}

fn arguments_has_generic(arguments: Option<&ast::Arguments>) -> bool {
    arguments.map_or(false, |arguments| {
        arguments
            .args
            .iter()
            .any(|expr| is_generic_expr(expr))
    })
}

fn is_generic_expr(expr: &Expr) -> bool {
    match expr {
        Expr::Name(ast::ExprName { id, .. }) => id.as_str() == "Generic",
        Expr::Attribute(ast::ExprAttribute { attr, .. }) => attr.as_str() == "Generic",
        Expr::Subscript(ast::ExprSubscript { value, .. }) => is_generic_expr(value),
        _ => false,
    }
}


pub fn class_call_arguments(
    arguments: Option<Box<ast::Arguments>>,
    in_class_scope: bool,
    mut extra_bases: Vec<Expr>,
) -> (Expr, Expr) {
    let mut bases = Vec::new();
    let mut kw_items = Vec::new();
    if let Some(args) = arguments {
        let args = *args;
        for base in args.args.into_vec() {
                    let base_expr = if in_class_scope {
                    if let Expr::Name(ast::ExprName { id, .. }) = &base {
                        let name = id.as_str();
                        let literal_name = class_lookup_literal_name(name);
                        py_expr!(
                            "__dp__.class_lookup(_dp_class_ns, {literal_name:literal}, lambda: {name:id})",
                            literal_name = literal_name,
                            name = name,
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
                    let name = id.as_str();
                    let literal_name = class_lookup_literal_name(name);
                    py_expr!(
                        "__dp__.class_lookup(_dp_class_ns, {literal_name:literal}, lambda: {name:id})",
                        literal_name = literal_name,
                        name = name,
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

    if !extra_bases.is_empty() {
        bases.append(&mut extra_bases);
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

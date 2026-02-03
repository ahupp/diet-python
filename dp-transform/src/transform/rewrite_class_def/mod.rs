
pub mod method;
pub mod private;
pub mod class_body;


use crate::template::empty_body;
use crate::transform::rewrite_expr::make_tuple;
use crate::transform::context::Context;
use crate::{py_expr, py_stmt, py_stmt_typed};
use ruff_python_ast::{
    self as ast, Expr, Stmt, TypeParam, TypeParamParamSpec,
    TypeParamTypeVar, TypeParamTypeVarTuple,
};
use ruff_text_size::TextRange;


use std::mem::take;


fn class_def_to_create_class_fn<'a>(
    context: &Context,
    class_def: &mut ast::StmtClassDef,
    class_qualname: String,
    needs_class_cell: bool,
) -> (ast::StmtFunctionDef, ast::StmtFunctionDef) {


    let ast::StmtClassDef {
        name,
        body,
        arguments,
        type_params,
        ..
    } = class_def;

    let type_params = take(type_params);
    let arguments = take(arguments);
    let mut body = std::mem::replace(
        body,
        empty_body().into(),
    );

    let class_name = name.id.to_string();
    let class_firstlineno = context.line_number_at(class_def.range.start().to_usize());

    // If the first (non-empty) statement is a string literal, assign it to __doc__ in the class dict.
    fn is_empty_body(stmt: &Stmt) -> bool {
        matches!(stmt, Stmt::BodyStmt(ast::StmtBody { body, .. }) if body.is_empty())
    }

    fn first_non_empty_stmt<'a>(body: &'a [Box<Stmt>]) -> Option<&'a Stmt> {
        for stmt in body {
            if is_empty_body(stmt.as_ref()) {
                continue;
            }
            return Some(stmt.as_ref());
        }
        None
    }

    fn class_doc_expr(stmt: &Stmt) -> Option<Expr> {
        match stmt {
            Stmt::Expr(ast::StmtExpr { value, .. }) => {
                if let Expr::StringLiteral(_) = value.as_ref() {
                    Some((**value).clone())
                } else {
                    None
                }
            }
            Stmt::BodyStmt(ast::StmtBody { body, .. }) => {
                first_non_empty_stmt(body).and_then(class_doc_expr)
            }
            _ => None,
        }
    }

    if let Some(first_stmt) = first_non_empty_stmt(&body.body) {
        if let Some(doc_expr) = class_doc_expr(first_stmt) {
            body.body.insert(0, Box::new(py_stmt!(
                "_dp_class_ns[{name:literal}] = {value:expr}",
                name = "__doc__",
                value = doc_expr
            )));
        }
    }

    let mut orig_bases_expr: Option<Expr> = None;
    let original_bases: Vec<Expr> = arguments
        .as_ref()
        .map(|args| args.args.clone().into_vec())
        .unwrap_or_default();

    let mut type_param_cleanup: Vec<Stmt> = Vec::new();
    let (type_param_bindings, mut type_param_statements, extra_bases) = if let Some(type_params) = type_params {
        context.require_typing_import();
        let type_param_info = make_type_param_info(*type_params);
        let has_generic_base = arguments_has_generic(arguments.as_deref());
        let generic_param_base = make_generic_base(&type_param_info);
        let mut extra_bases = Vec::new();
        if !has_generic_base {
            extra_bases.push(py_expr!("_dp_typing.Generic"));
        }

        if let Some(generic_param_base) = generic_param_base {
            let mut orig_bases = Vec::new();
            if has_generic_base {
                for base in &original_bases {
                    if is_generic_expr(base) {
                        orig_bases.push(generic_param_base.clone());
                    } else {
                        orig_bases.push(base.clone());
                    }
                }
            } else {
                orig_bases.extend(original_bases.clone());
                orig_bases.push(generic_param_base);
            }
            if !orig_bases.is_empty() {
                orig_bases_expr = Some(make_tuple(orig_bases));
            }
        }
        let mut type_param_statements = type_param_info.type_params_tuple.map(|tuple_expr| {
            vec![py_stmt!(
                "_dp_class_ns[{name:literal}] = {tuple:expr}",
                name = "__type_params__",
                tuple = tuple_expr.clone()
            )]
        }).unwrap_or_default();

        for name in &type_param_info.param_names {
            type_param_statements.push(py_stmt!(
                "_dp_class_ns[{name:literal}] = {name:id}",
                name = name.as_str(),
            ));
            type_param_cleanup.push(py_stmt!(
                r#"
if _dp_class_ns.get({name:literal}) is {name:id}:
    del _dp_class_ns[{name:literal}]
"#,
                name = name.as_str(),
            ));
        }

        (type_param_info.bindings, type_param_statements, extra_bases)
    } else {
        (vec![], vec![], vec![])
    };

    if let Some(orig_bases_expr) = orig_bases_expr {
        type_param_statements.push(py_stmt!(
            "_dp_class_ns[{name:literal}] = {value:expr}",
            name = "__orig_bases__",
            value = orig_bases_expr
        ));
    }


    let (bases_tuple, prepare_dict) = class_call_arguments(
        arguments,
        extra_bases,
    );

    // TODO: bases probably depends on the type params too

    // type params are written as regular locals rather than direct assignments to _dp_class_ns
    // so they are visible to inner scopes
    let class_ns_def: ast::StmtFunctionDef = py_stmt_typed!(
        r#"
def _dp_class_ns_{class_name:id}(_dp_class_ns, _dp_classcell):
    _dp_class_ns["__module__"] = __name__
    _dp_class_ns["__qualname__"] = {class_qualname:literal}
    {type_param_bindings:stmt}
    {type_param_statements:stmt}
    {ns_body:stmt}
    {type_param_cleanup:stmt}"#,

    class_name = class_name.as_str(),
        class_qualname = class_qualname.as_str(),
        ns_body = body,
        type_param_statements = type_param_statements,
        type_param_bindings = type_param_bindings.clone(),
        type_param_cleanup = type_param_cleanup,
    );

    let define_class_fn: ast::StmtFunctionDef = py_stmt_typed!(r#"
def _dp_define_class_{class_name:id}():
    {type_param_bindings:stmt}
    return __dp__.create_class(
      {class_name:literal}, 
      _dp_class_ns_{class_name:id}, 
      {bases:expr}, 
      {prepare_dict:expr}, 
      {requires_class_cell:literal},
      {firstlineno:literal},
      ()
    )
"#,
        class_name = class_name.as_str(),
        type_param_bindings = type_param_bindings.clone(),
        requires_class_cell = needs_class_cell,
        type_param_bindings = type_param_bindings,
        firstlineno = class_firstlineno,
        bases = bases_tuple.clone(),
        prepare_dict = prepare_dict.clone(),
    );

    (class_ns_def, define_class_fn)
}


struct TypeParamInfo {
    bindings: Vec<Stmt>,
    param_names: Vec<String>,
    type_params_tuple: Option<Expr>,
    generic_params: Vec<Expr>,
}

fn make_type_param_info(
    type_params: ast::TypeParams,
) -> TypeParamInfo {
    // TODO
//    rewriter.visit_type_params(&mut type_params);

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

                bindings.push(py_stmt!(
                    "{name:id} = _dp_typing.TypeVar({name_literal:literal}, {bound:expr}, {default:expr}, {constraints:expr})",
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
                let binding = match default.map(|expr| *expr) {
                    Some(default_expr) => py_stmt!(
                        "{name:id} = _dp_typing.TypeVarTuple({name_literal:literal}, default={default:expr})",
                        name = param_name.as_str(),
                        name_literal = param_name.as_str(),
                        default = default_expr,
                    ),
                    None => py_stmt!(
                        "{name:id} = _dp_typing.TypeVarTuple({name_literal:literal})",
                        name = param_name.as_str(),
                        name_literal = param_name.as_str(),
                    ),
                };

                bindings.push(binding);
                type_param_exprs.push(py_expr!("{name:id}", name = param_name.as_str()));
                generic_params.push(py_expr!(
                    "_dp_typing.Unpack[{name:id}]",
                    name = param_name.as_str()
                ));
                param_names.push(param_name);
            }
            TypeParam::ParamSpec(TypeParamParamSpec { name, default, .. }) => {
                let param_name = name.as_str().to_string();
                let binding = match default.map(|expr| *expr) {
                    Some(default_expr) => py_stmt!(
                        "{name:id} = _dp_typing.ParamSpec({name_literal:literal}, default={default:expr})",
                        name = param_name.as_str(),
                        name_literal = param_name.as_str(),
                        default = default_expr,
                    ),
                    None => py_stmt!(
                        "{name:id} = _dp_typing.ParamSpec({name_literal:literal})",
                        name = param_name.as_str(),
                        name_literal = param_name.as_str(),
                    ),
                };

                bindings.push(binding);
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
    Some(py_expr!(
        "_dp_typing.Generic[{params:expr}]",
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
    mut extra_bases: Vec<Expr>,
) -> (Expr, Expr) {
    let mut bases = Vec::new();
    let mut kw_items = Vec::new();
    if let Some(args) = arguments {
        let args = *args;
        for base in args.args.into_vec() {
            bases.push(base);
        }
        for kw in args.keywords.into_vec() {
            let value = kw.value;
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

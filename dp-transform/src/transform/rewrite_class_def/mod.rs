pub mod class_vars;
pub mod method;
pub mod private;

use crate::transform::ast_rewrite::Rewrite;
use crate::transform::scope::{BindingKind, Scope, ScopeKind};
use crate::transform::{rewrite_stmt};
use crate::transform::rewrite_expr::make_tuple;
use crate::{py_expr, py_stmt};
use ruff_python_ast::{
    self as ast, Arguments, Expr, Identifier, Stmt, StmtClassDef, TypeParam, TypeParamParamSpec,
    TypeParamTypeVar, TypeParamTypeVarTuple, TypeParams,
};
use ruff_text_size::TextRange;

use crate::template::py_stmt_single;
use crate::transform::rewrite_class_def::class_vars::rewrite_class_scope;
use crate::transform::context::{Context};

use log::{log_enabled, trace, Level};

use std::collections::HashSet;
use std::mem::take;

pub fn class_lookup_literal_name(name: &str) -> &str {
    if let Some((base, _suffix)) = name.rsplit_once('$') {
        base    
    } else {
        name
    }
}

pub fn class_body_load(name: &str) -> Expr {
    let literal_name = class_lookup_literal_name(name);
    py_expr!(
        "__dp__.class_lookup_cell(_dp_class_ns, {literal_name:literal}, {name:id})",
        literal_name = literal_name,
        name = name,
    )
}

pub fn class_body_load_global(name: &str) -> Expr {
    let literal_name = class_lookup_literal_name(name);
    py_expr!(
        "__dp__.class_lookup_global(_dp_class_ns, {literal_name:literal}, globals())",
        literal_name = literal_name,
    )
}



pub fn rewrite<'a>(
    context: &Context,
    scope: &Scope,
    mut class_def: ast::StmtClassDef,
) -> Rewrite {
    let class_name = class_def.name.id.to_string();
    let class_qualname = scope.make_qualname(&class_name);

    let decorator_list = take(&mut class_def.decorator_list);

    let create_class_fn = class_def_to_create_class_fn(
        &mut class_def,
        class_qualname.to_owned(),
        context, 
        scope,
    );

    rewrite_stmt::decorator::rewrite(
        context,
        decorator_list,
        class_name.as_str(),
        create_class_fn,
    )

}

fn class_def_to_create_class_fn<'a>(
    class_def: &mut ast::StmtClassDef,
    class_qualname: String,
    context: &Context,
    scope: &Scope,
) -> Vec<Stmt> {

    let needs_class_cell = method::rewrite_methods_in_class_body(
        class_def,
    );

    let ast::StmtClassDef {
        name,
        body,
        arguments,
        type_params,
        ..
    } = class_def;

    let type_params = take(type_params);
    let arguments = take(arguments);
    let mut body = take(body);

    let class_name = name.id.to_string();

    // If the first statement is a string literal, assign it to  __doc__
    if let Some(first_stmt) = body.first_mut() {
        if let Stmt::Expr(ast::StmtExpr { value, .. }) = first_stmt {
            if let Expr::StringLiteral(_) = value.as_ref() {
                let doc_expr = (*value).clone();
                *first_stmt =
                    py_stmt_single(py_stmt!("__doc__ = {value:expr}", value = doc_expr));
            }
        }
    }

    let type_param_info = type_params.map(|type_params| {
        make_type_param_info(*type_params)
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

    let mut ns_builder = py_stmt!(
        r#"
__module__ = __name__
__qualname__ = {class_qualname:literal}
{type_param_statements:stmt}
{ns_body:stmt}
"#,
        class_qualname = class_qualname.as_str(),
        ns_body = body,
        type_param_statements = type_param_statements,
    );

    let class_locals: HashSet<String> = scope
        .scope_bindings()
        .iter()
        .filter_map(|(name, kind)| {
            if *kind == BindingKind::Local {
                Some(name.clone())
            } else {
                None
            }
        })
        .collect();


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


    let type_param_bindings = type_param_info
        .as_ref()
        .map_or_else(Vec::new, |info| info.bindings.clone());
    let has_generic_base = arguments_has_generic(arguments.as_deref());
    let generic_base = type_param_info
        .as_ref()
        .filter(|_| !has_generic_base)
        .and_then(make_generic_base);
    let extra_bases = generic_base.into_iter().collect::<Vec<_>>();
    let (bases_tuple, prepare_dict) = class_call_arguments(
        arguments,
        extra_bases,
    );

    rewrite_class_scope(
        &mut ns_builder,
        type_param_skip,
    );

    let out = py_stmt!(
        r#"
def _dp_class_create_{class_name:id}():
    {type_param_bindings:stmt}

    def _dp_class_ns_{class_name:id}(_dp_class_ns, __classcell__):
        {ns_body:stmt}

    return __dp__.create_class(
      {class_name:literal}, 
      _dp_class_ns_{class_name:id}, 
      {bases:expr}, 
      {prepare_dict:expr}, 
      {requires_class_cell:literal}
    )
{class_name:id} = _dp_class_create_{class_name:id}()
"#,
        class_name = class_name.as_str(),
        requires_class_cell = needs_class_cell,
        type_param_bindings = type_param_bindings,
        ns_body = ns_builder,
        bases = bases_tuple.clone(),
        prepare_dict = prepare_dict.clone(),
    );
    out
}

fn rewrite_nested_class_def(
    context: &Context,
    class_name: &str,
    decorators: Vec<ast::Decorator>,
    mut create_class_fn: Vec<Stmt>,
) -> Rewrite {
    if create_class_fn.is_empty() {
        return Rewrite::Walk(create_class_fn);
    }

    let _ = create_class_fn.pop();
    if decorators.is_empty() {
        let assign = py_stmt!(
            "_dp_class_ns[{name:literal}] = _dp_class_create_{name:id}()",
            name = class_name,
        );
        create_class_fn.extend(assign);
        return Rewrite::Walk(create_class_fn);
    }

    let mut prefix = Vec::with_capacity(decorators.len());
    let mut decorator_names = Vec::with_capacity(decorators.len());
    for decorator in decorators {
        let temp = context.fresh("decorator");
        prefix.extend(py_stmt!(
            "{temp:id} = {decorator:expr}",
            temp = temp.as_str(),
            decorator = decorator.expression
        ));
        decorator_names.push(temp);
    }

    let class_tmp = context.fresh("class");
    let mut decorated = py_expr!("{class_tmp:id}", class_tmp = class_tmp.as_str());
    for decorator in decorator_names.iter().rev() {
        decorated = py_expr!(
            "{decorator:id}({decorated:expr})",
            decorator = decorator.as_str(),
            decorated = decorated
        );
    }

    let mut out = Vec::new();
    out.extend(prefix);
    out.extend(create_class_fn);
    out.extend(py_stmt!(
        "{class_tmp:id} = _dp_class_create_{name:id}()",
        class_tmp = class_tmp.as_str(),
        name = class_name
    ));
    out.extend(py_stmt!(
        "_dp_class_ns[{name:literal}] = {decorated:expr}",
        name = class_name,
        decorated = decorated
    ));

    Rewrite::Walk(out)
}


struct TypeParamInfo {
    bindings: Vec<Stmt>,
    param_names: Vec<String>,
    type_params_tuple: Option<Expr>,
    generic_params: Vec<Expr>,
}

fn make_type_param_info(
    mut type_params: ast::TypeParams,
//    rewriter: &mut ExprRewriter,
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

                bindings.extend(py_stmt!(
                    "{name:id} = __dp__.typing.TypeVar({name_literal:literal}, {bound:expr}, {default:expr}, {constraints:expr})",
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
                        "{name:id} = __dp__.typing.TypeVarTuple({name_literal:literal}, default={default:expr})",
                        name = param_name.as_str(),
                        name_literal = param_name.as_str(),
                        default = default_expr,
                    ),
                    None => py_stmt!(
                        "{name:id} = __dp__.typing.TypeVarTuple({name_literal:literal})",
                        name = param_name.as_str(),
                        name_literal = param_name.as_str(),
                    ),
                };

                bindings.extend(binding);
                type_param_exprs.push(py_expr!("{name:id}", name = param_name.as_str()));
                generic_params.push(py_expr!(
                    "__dp__.typing.Unpack[{name:id}]",
                    name = param_name.as_str()
                ));
                param_names.push(param_name);
            }
            TypeParam::ParamSpec(TypeParamParamSpec { name, default, .. }) => {
                let param_name = name.as_str().to_string();
                let binding = match default.map(|expr| *expr) {
                    Some(default_expr) => py_stmt!(
                        "{name:id} = __dp__.typing.ParamSpec({name_literal:literal}, default={default:expr})",
                        name = param_name.as_str(),
                        name_literal = param_name.as_str(),
                        default = default_expr,
                    ),
                    None => py_stmt!(
                        "{name:id} = __dp__.typing.ParamSpec({name_literal:literal})",
                        name = param_name.as_str(),
                        name_literal = param_name.as_str(),
                    ),
                };

                bindings.extend(binding);
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
        "__dp__.typing.Generic[{params:expr}]",
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

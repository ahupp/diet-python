use proc_macro::TokenStream;
use std::str::FromStr;

use diet_python::min_ast::{
    Arg, ExprNode, FunctionDef, Number, OuterScopeVars, Parameter, StmtNode,
};
use ruff_python_parser::parse_module;
use syn::{parse_macro_input, LitStr};

#[proc_macro]
pub fn py_stmt_match(input: TokenStream) -> TokenStream {
    let literal = parse_macro_input!(input as LitStr);
    let span = literal.span();
    match build_stmt_literal(&literal.value()) {
        Ok(expr) => match TokenStream::from_str(&expr) {
            Ok(tokens) => tokens,
            Err(err) => syn::Error::new(span, format!("failed to parse generated tokens: {err}"))
                .to_compile_error()
                .into(),
        },
        Err(message) => syn::Error::new(span, message).to_compile_error().into(),
    }
}

fn build_stmt_literal(source: &str) -> Result<String, String> {
    let module = parse_module(source)
        .map_err(|err| format!("failed to parse Python source: {err}"))?
        .into_syntax();
    let module = diet_python::min_ast::Module::from(module);
    match module.body.as_slice() {
        [stmt] => Ok(stmt_to_rust_literal(stmt)),
        [] => Err("expected at least one statement".to_string()),
        _ => Err("expected exactly one statement".to_string()),
    }
}

fn info_to_literal(info: &()) -> String {
    format!("{:?}", info)
}

fn string_literal(value: &str) -> String {
    format!("{:?}", value)
}

fn number_to_rust_literal(number: &Number) -> String {
    match number {
        Number::Int(value) => format!("Number::Int({:?})", value),
        Number::Float(value) => format!("Number::Float({:?})", value),
    }
}

fn vec_literal(elements: Vec<String>) -> String {
    if elements.is_empty() {
        "vec![]".to_string()
    } else {
        format!("vec![{}]", elements.join(", "))
    }
}

fn option_literal(value: Option<String>) -> String {
    match value {
        Some(expr) => format!("Some({expr})"),
        None => "None".to_string(),
    }
}

fn outer_scope_vars_to_literal(vars: &OuterScopeVars) -> String {
    let globals = vec_literal(
        vars.globals
            .iter()
            .map(|name| string_literal(name))
            .collect(),
    );
    let nonlocals = vec_literal(
        vars.nonlocals
            .iter()
            .map(|name| string_literal(name))
            .collect(),
    );
    format!("OuterScopeVars {{ globals: {globals}, nonlocals: {nonlocals} }}")
}

fn parameter_to_rust_literal(parameter: &Parameter) -> String {
    match parameter {
        Parameter::Positional { name, default } => format!(
            "Parameter::Positional {{ name: {name}, default: {default} }}",
            name = string_literal(name),
            default = option_literal(default.as_ref().map(expr_to_rust_literal))
        ),
        Parameter::VarArg { name } => {
            format!("Parameter::VarArg {{ name: {} }}", string_literal(name))
        }
        Parameter::KwOnly { name, default } => format!(
            "Parameter::KwOnly {{ name: {name}, default: {default} }}",
            name = string_literal(name),
            default = option_literal(default.as_ref().map(expr_to_rust_literal))
        ),
        Parameter::KwArg { name } => {
            format!("Parameter::KwArg {{ name: {} }}", string_literal(name))
        }
    }
}

fn parameters_to_literal(params: &[Parameter]) -> String {
    vec_literal(params.iter().map(parameter_to_rust_literal).collect())
}

fn args_to_literal(args: &[Arg]) -> String {
    vec_literal(args.iter().map(arg_to_rust_literal).collect())
}

fn arg_to_rust_literal(arg: &Arg) -> String {
    match arg {
        Arg::Positional(expr) => {
            format!("Arg::Positional({})", expr_to_rust_literal(expr))
        }
        Arg::Starred(expr) => format!("Arg::Starred({})", expr_to_rust_literal(expr)),
        Arg::Keyword { name, value } => format!(
            "Arg::Keyword {{ name: {}, value: {} }}",
            string_literal(name),
            expr_to_rust_literal(value)
        ),
        Arg::KwStarred(expr) => {
            format!("Arg::KwStarred({})", expr_to_rust_literal(expr))
        }
    }
}

fn expr_vec_to_literal(exprs: &[ExprNode]) -> String {
    vec_literal(exprs.iter().map(expr_to_rust_literal).collect())
}

fn stmt_vec_to_literal(stmts: &[StmtNode]) -> String {
    vec_literal(stmts.iter().map(stmt_to_rust_literal).collect())
}

fn bytes_literal(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        "vec![]".to_string()
    } else {
        let values: Vec<String> = bytes.iter().map(|byte| byte.to_string()).collect();
        format!("vec![{}]", values.join(", "))
    }
}

fn expr_to_rust_literal(expr: &ExprNode) -> String {
    match expr {
        ExprNode::Name { info, id } => format!(
            "ExprNode::Name {{ info: {info}, id: {id} }}",
            info = info_to_literal(info),
            id = string_literal(id)
        ),
        ExprNode::Number { info, value } => format!(
            "ExprNode::Number {{ info: {info}, value: {value} }}",
            info = info_to_literal(info),
            value = number_to_rust_literal(value)
        ),
        ExprNode::String { info, value } => format!(
            "ExprNode::String {{ info: {info}, value: {value} }}",
            info = info_to_literal(info),
            value = string_literal(value)
        ),
        ExprNode::Bytes { info, value } => format!(
            "ExprNode::Bytes {{ info: {info}, value: {value} }}",
            info = info_to_literal(info),
            value = bytes_literal(value)
        ),
        ExprNode::Tuple { info, elts } => format!(
            "ExprNode::Tuple {{ info: {info}, elts: {elts} }}",
            info = info_to_literal(info),
            elts = expr_vec_to_literal(elts)
        ),
        ExprNode::Await { info, value } => format!(
            "ExprNode::Await {{ info: {info}, value: Box::new({value}) }}",
            info = info_to_literal(info),
            value = expr_to_rust_literal(value)
        ),
        ExprNode::Yield { info, value } => format!(
            "ExprNode::Yield {{ info: {info}, value: {value} }}",
            info = info_to_literal(info),
            value = option_literal(
                value
                    .as_ref()
                    .map(|expr| format!("Box::new({})", expr_to_rust_literal(expr)))
            )
        ),
        ExprNode::Call { info, func, args } => format!(
            "ExprNode::Call {{ info: {info}, func: Box::new({func}), args: {args} }}",
            info = info_to_literal(info),
            func = expr_to_rust_literal(func),
            args = args_to_literal(args)
        ),
    }
}

fn function_def_to_literal(func: &FunctionDef) -> String {
    format!(
        "FunctionDef {{ info: {info}, name: {name}, params: {params}, body: {body}, is_async: {is_async}, scope_vars: {scope} }}",
        info = info_to_literal(&func.info),
        name = string_literal(&func.name),
        params = parameters_to_literal(&func.params),
        body = stmt_vec_to_literal(&func.body),
        is_async = format!("{:?}", func.is_async),
        scope = outer_scope_vars_to_literal(&func.scope_vars)
    )
}

fn stmt_to_rust_literal(stmt: &StmtNode) -> String {
    match stmt {
        StmtNode::FunctionDef(func) => {
            format!("StmtNode::FunctionDef({})", function_def_to_literal(func))
        }
        StmtNode::While {
            info,
            test,
            body,
            orelse,
        } => format!(
            "StmtNode::While {{ info: {info}, test: {test}, body: {body}, orelse: {orelse} }}",
            info = info_to_literal(info),
            test = expr_to_rust_literal(test),
            body = stmt_vec_to_literal(body),
            orelse = stmt_vec_to_literal(orelse)
        ),
        StmtNode::If {
            info,
            test,
            body,
            orelse,
        } => format!(
            "StmtNode::If {{ info: {info}, test: {test}, body: {body}, orelse: {orelse} }}",
            info = info_to_literal(info),
            test = expr_to_rust_literal(test),
            body = stmt_vec_to_literal(body),
            orelse = stmt_vec_to_literal(orelse)
        ),
        StmtNode::Try {
            info,
            body,
            handler,
            orelse,
            finalbody,
        } => format!(
            "StmtNode::Try {{ info: {info}, body: {body}, handler: {handler}, orelse: {orelse}, finalbody: {finalbody} }}",
            info = info_to_literal(info),
            body = stmt_vec_to_literal(body),
            handler = option_literal(
                handler
                    .as_ref()
                    .map(|stmts| stmt_vec_to_literal(stmts))
            ),
            orelse = stmt_vec_to_literal(orelse),
            finalbody = stmt_vec_to_literal(finalbody)
        ),
        StmtNode::Raise { info, exc } => format!(
            "StmtNode::Raise {{ info: {info}, exc: {exc} }}",
            info = info_to_literal(info),
            exc = option_literal(
                exc.as_ref().map(|expr| expr_to_rust_literal(expr))
            )
        ),
        StmtNode::Break(info) => {
            format!("StmtNode::Break({})", info_to_literal(info))
        }
        StmtNode::Continue(info) => {
            format!("StmtNode::Continue({})", info_to_literal(info))
        }
        StmtNode::Return { info, value } => format!(
            "StmtNode::Return {{ info: {info}, value: {value} }}",
            info = info_to_literal(info),
            value = option_literal(
                value.as_ref().map(|expr| expr_to_rust_literal(expr))
            )
        ),
        StmtNode::Expr { info, value } => format!(
            "StmtNode::Expr {{ info: {info}, value: {value} }}",
            info = info_to_literal(info),
            value = expr_to_rust_literal(value)
        ),
        StmtNode::Assign { info, target, value } => format!(
            "StmtNode::Assign {{ info: {info}, target: {target}, value: {value} }}",
            info = info_to_literal(info),
            target = string_literal(target),
            value = expr_to_rust_literal(value)
        ),
        StmtNode::Delete { info, target } => format!(
            "StmtNode::Delete {{ info: {info}, target: {target} }}",
            info = info_to_literal(info),
            target = string_literal(target)
        ),
        StmtNode::Pass(info) => format!("StmtNode::Pass({})", info_to_literal(info)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_assign_literal() {
        let literal = build_stmt_literal("a = 1").expect("should build literal");
        assert_eq!(
            literal,
            "StmtNode::Assign { info: (), target: \"a\", value: ExprNode::Number { info: (), value: Number::Int(\"1\") } }"
        );
    }
}

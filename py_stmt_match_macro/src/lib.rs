use proc_macro::TokenStream;
use std::collections::HashMap;
use std::fmt::Debug;
use std::str::FromStr;

use diet_python::min_ast::{
    Arg, ExprNode, FunctionDef, Number, OuterScopeVars, Parameter, StmtNode,
};
use ruff_python_parser::parse_module;
use syn::{parse_macro_input, Ident, LitStr};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BindingKind {
    Move,
    Ref,
    RefMut,
}

impl BindingKind {
    fn from_prefix(prefix: Option<&str>) -> Result<Self, String> {
        match prefix {
            None => Ok(BindingKind::Move),
            Some("ref") => Ok(BindingKind::Ref),
            Some("mut") => Ok(BindingKind::RefMut),
            Some(other) => Err(format!("unsupported binding prefix `{other}`")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PlaceholderBinding {
    ident: String,
    kind: BindingKind,
}

impl PlaceholderBinding {
    fn pattern(&self) -> String {
        match self.kind {
            BindingKind::Move => self.ident.clone(),
            BindingKind::Ref => format!("ref {}", self.ident),
            BindingKind::RefMut => format!("ref mut {}", self.ident),
        }
    }
}

fn parse_placeholders(
    source: &str,
) -> Result<(String, HashMap<String, PlaceholderBinding>), String> {
    let mut result = String::with_capacity(source.len());
    let mut markers = HashMap::new();
    let mut index = 0;

    while let Some(start_rel) = source[index..].find('{') {
        let start = index + start_rel;
        result.push_str(&source[index..start]);
        let end_rel = source[start + 1..]
            .find('}')
            .ok_or_else(|| "unclosed placeholder".to_string())?;
        let end = start + 1 + end_rel;
        let content = &source[start + 1..end];
        let trimmed = content.trim();

        if trimmed.is_empty() {
            return Err("empty placeholder".to_string());
        }

        let mut parts = trimmed.split_whitespace();
        let first = parts
            .next()
            .ok_or_else(|| "expected placeholder name".to_string())?;

        let (kind, ident_str) = if first == "ref" || first == "mut" {
            let name = parts
                .next()
                .ok_or_else(|| format!("expected name for placeholder `{first}`"))?;
            (BindingKind::from_prefix(Some(first))?, name)
        } else {
            (BindingKind::from_prefix(None)?, first)
        };

        if parts.next().is_some() {
            return Err(format!("unexpected tokens in placeholder `{{{trimmed}}}`"));
        }

        let ident = syn::parse_str::<Ident>(ident_str)
            .map_err(|err| format!("invalid placeholder identifier `{ident_str}`: {err}"))?;
        let marker = format!("_dp_stmt_placeholder_{}__", markers.len());
        markers.insert(
            marker.clone(),
            PlaceholderBinding {
                ident: ident.to_string(),
                kind,
            },
        );
        result.push_str(&marker);
        index = end + 1;
    }

    result.push_str(&source[index..]);
    Ok((result, markers))
}

struct LiteralBuilder {
    placeholders: HashMap<String, PlaceholderBinding>,
}

impl LiteralBuilder {
    fn new(placeholders: HashMap<String, PlaceholderBinding>) -> Self {
        LiteralBuilder { placeholders }
    }

    fn placeholder_pattern(&self, marker: &str) -> Option<String> {
        self.placeholders
            .get(marker)
            .map(|binding| binding.pattern())
    }

    fn info_to_literal<T: Debug>(&self, info: &T) -> String {
        format!("{:?}", info)
    }

    fn string_literal(&self, value: &str) -> String {
        if let Some(pattern) = self.placeholder_pattern(value) {
            pattern
        } else {
            format!("{:?}", value)
        }
    }

    fn number_to_rust_literal(&self, number: &Number) -> String {
        match number {
            Number::Int(value) => format!("Number::Int({:?})", value),
            Number::Float(value) => format!("Number::Float({:?})", value),
        }
    }

    fn vec_literal(&self, elements: Vec<String>) -> String {
        if elements.is_empty() {
            "vec![]".to_string()
        } else {
            format!("vec![{}]", elements.join(", "))
        }
    }

    fn option_literal(&self, value: Option<String>) -> String {
        match value {
            Some(expr) => format!("Some({expr})"),
            None => "None".to_string(),
        }
    }

    fn outer_scope_vars_to_literal(&self, vars: &OuterScopeVars) -> String {
        let globals = self.vec_literal(
            vars.globals
                .iter()
                .map(|name| self.string_literal(name))
                .collect(),
        );
        let nonlocals = self.vec_literal(
            vars.nonlocals
                .iter()
                .map(|name| self.string_literal(name))
                .collect(),
        );
        format!("OuterScopeVars {{ globals: {globals}, nonlocals: {nonlocals} }}")
    }

    fn parameter_to_rust_literal(&self, parameter: &Parameter) -> String {
        match parameter {
            Parameter::Positional { name, default } => format!(
                "Parameter::Positional {{ name: {name}, default: {default} }}",
                name = self.string_literal(name),
                default = self
                    .option_literal(default.as_ref().map(|expr| self.expr_to_rust_literal(expr)))
            ),
            Parameter::VarArg { name } => {
                format!(
                    "Parameter::VarArg {{ name: {} }}",
                    self.string_literal(name)
                )
            }
            Parameter::KwOnly { name, default } => format!(
                "Parameter::KwOnly {{ name: {name}, default: {default} }}",
                name = self.string_literal(name),
                default = self
                    .option_literal(default.as_ref().map(|expr| self.expr_to_rust_literal(expr)))
            ),
            Parameter::KwArg { name } => {
                format!("Parameter::KwArg {{ name: {} }}", self.string_literal(name))
            }
        }
    }

    fn parameters_to_literal(&self, params: &[Parameter]) -> String {
        self.vec_literal(
            params
                .iter()
                .map(|param| self.parameter_to_rust_literal(param))
                .collect(),
        )
    }

    fn args_to_literal(&self, args: &[Arg]) -> String {
        self.vec_literal(
            args.iter()
                .map(|arg| self.arg_to_rust_literal(arg))
                .collect(),
        )
    }

    fn arg_to_rust_literal(&self, arg: &Arg) -> String {
        match arg {
            Arg::Positional(expr) => {
                format!("Arg::Positional({})", self.expr_to_rust_literal(expr))
            }
            Arg::Starred(expr) => format!("Arg::Starred({})", self.expr_to_rust_literal(expr)),
            Arg::Keyword { name, value } => format!(
                "Arg::Keyword {{ name: {}, value: {} }}",
                self.string_literal(name),
                self.expr_to_rust_literal(value)
            ),
            Arg::KwStarred(expr) => {
                format!("Arg::KwStarred({})", self.expr_to_rust_literal(expr))
            }
        }
    }

    fn expr_vec_to_literal(&self, exprs: &[ExprNode]) -> String {
        self.vec_literal(
            exprs
                .iter()
                .map(|expr| self.expr_to_rust_literal(expr))
                .collect(),
        )
    }

    fn stmt_vec_to_literal(&self, stmts: &[StmtNode]) -> String {
        self.vec_literal(
            stmts
                .iter()
                .map(|stmt| self.stmt_to_rust_literal(stmt))
                .collect(),
        )
    }

    fn bytes_literal(&self, bytes: &[u8]) -> String {
        if bytes.is_empty() {
            "vec![]".to_string()
        } else {
            let values: Vec<String> = bytes.iter().map(|byte| byte.to_string()).collect();
            format!("vec![{}]", values.join(", "))
        }
    }

    fn expr_to_rust_literal(&self, expr: &ExprNode) -> String {
        if let ExprNode::Name { id, .. } = expr {
            if let Some(pattern) = self.placeholder_pattern(id) {
                return pattern;
            }
        }

        match expr {
            ExprNode::Name { info, id } => format!(
                "ExprNode::Name {{ info: {info}, id: {id} }}",
                info = self.info_to_literal(info),
                id = self.string_literal(id)
            ),
            ExprNode::Number { info, value } => format!(
                "ExprNode::Number {{ info: {info}, value: {value} }}",
                info = self.info_to_literal(info),
                value = self.number_to_rust_literal(value)
            ),
            ExprNode::String { info, value } => format!(
                "ExprNode::String {{ info: {info}, value: {value} }}",
                info = self.info_to_literal(info),
                value = self.string_literal(value)
            ),
            ExprNode::Bytes { info, value } => format!(
                "ExprNode::Bytes {{ info: {info}, value: {value} }}",
                info = self.info_to_literal(info),
                value = self.bytes_literal(value)
            ),
            ExprNode::Tuple { info, elts } => format!(
                "ExprNode::Tuple {{ info: {info}, elts: {elts} }}",
                info = self.info_to_literal(info),
                elts = self.expr_vec_to_literal(elts)
            ),
            ExprNode::Await { info, value } => format!(
                "ExprNode::Await {{ info: {info}, value: Box::new({value}) }}",
                info = self.info_to_literal(info),
                value = self.expr_to_rust_literal(value)
            ),
            ExprNode::Yield { info, value } => format!(
                "ExprNode::Yield {{ info: {info}, value: {value} }}",
                info = self.info_to_literal(info),
                value = self.option_literal(
                    value
                        .as_ref()
                        .map(|expr| format!("Box::new({})", self.expr_to_rust_literal(expr)))
                )
            ),
            ExprNode::Call { info, func, args } => format!(
                "ExprNode::Call {{ info: {info}, func: Box::new({func}), args: {args} }}",
                info = self.info_to_literal(info),
                func = self.expr_to_rust_literal(func),
                args = self.args_to_literal(args)
            ),
        }
    }

    fn function_def_to_literal(&self, func: &FunctionDef) -> String {
        format!(
            "FunctionDef {{ info: {info}, name: {name}, params: {params}, body: {body}, is_async: {is_async}, scope_vars: {scope} }}",
            info = self.info_to_literal(&func.info),
            name = self.string_literal(&func.name),
            params = self.parameters_to_literal(&func.params),
            body = self.stmt_vec_to_literal(&func.body),
            is_async = format!("{:?}", func.is_async),
            scope = self.outer_scope_vars_to_literal(&func.scope_vars)
        )
    }

    fn stmt_to_rust_literal(&self, stmt: &StmtNode) -> String {
        match stmt {
            StmtNode::FunctionDef(func) => {
                format!("StmtNode::FunctionDef({})", self.function_def_to_literal(func))
            }
            StmtNode::While {
                info,
                test,
                body,
                orelse,
            } => format!(
                "StmtNode::While {{ info: {info}, test: {test}, body: {body}, orelse: {orelse} }}",
                info = self.info_to_literal(info),
                test = self.expr_to_rust_literal(test),
                body = self.stmt_vec_to_literal(body),
                orelse = self.stmt_vec_to_literal(orelse)
            ),
            StmtNode::If {
                info,
                test,
                body,
                orelse,
            } => format!(
                "StmtNode::If {{ info: {info}, test: {test}, body: {body}, orelse: {orelse} }}",
                info = self.info_to_literal(info),
                test = self.expr_to_rust_literal(test),
                body = self.stmt_vec_to_literal(body),
                orelse = self.stmt_vec_to_literal(orelse)
            ),
            StmtNode::Try {
                info,
                body,
                handler,
                orelse,
                finalbody,
            } => format!(
                "StmtNode::Try {{ info: {info}, body: {body}, handler: {handler}, orelse: {orelse}, finalbody: {finalbody} }}",
                info = self.info_to_literal(info),
                body = self.stmt_vec_to_literal(body),
                handler = self.option_literal(
                    handler
                        .as_ref()
                        .map(|stmts| self.stmt_vec_to_literal(stmts))
                ),
                orelse = self.stmt_vec_to_literal(orelse),
                finalbody = self.stmt_vec_to_literal(finalbody)
            ),
            StmtNode::Raise { info, exc } => format!(
                "StmtNode::Raise {{ info: {info}, exc: {exc} }}",
                info = self.info_to_literal(info),
                exc = self.option_literal(
                    exc.as_ref().map(|expr| self.expr_to_rust_literal(expr))
                )
            ),
            StmtNode::Break(info) => {
                format!("StmtNode::Break({})", self.info_to_literal(info))
            }
            StmtNode::Continue(info) => {
                format!("StmtNode::Continue({})", self.info_to_literal(info))
            }
            StmtNode::Return { info, value } => format!(
                "StmtNode::Return {{ info: {info}, value: {value} }}",
                info = self.info_to_literal(info),
                value = self.option_literal(
                    value.as_ref().map(|expr| self.expr_to_rust_literal(expr))
                )
            ),
            StmtNode::Expr { info, value } => format!(
                "StmtNode::Expr {{ info: {info}, value: {value} }}",
                info = self.info_to_literal(info),
                value = self.expr_to_rust_literal(value)
            ),
            StmtNode::Assign { info, target, value } => format!(
                "StmtNode::Assign {{ info: {info}, target: {target}, value: {value} }}",
                info = self.info_to_literal(info),
                target = self.string_literal(target),
                value = self.expr_to_rust_literal(value)
            ),
            StmtNode::Delete { info, target } => format!(
                "StmtNode::Delete {{ info: {info}, target: {target} }}",
                info = self.info_to_literal(info),
                target = self.string_literal(target)
            ),
            StmtNode::Pass(info) => format!("StmtNode::Pass({})", self.info_to_literal(info)),
        }
    }
}

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
    let (rewritten, placeholders) = parse_placeholders(source)?;
    let module = parse_module(&rewritten)
        .map_err(|err| format!("failed to parse Python source: {err}"))?
        .into_syntax();
    let module = diet_python::min_ast::Module::from(module);
    let builder = LiteralBuilder::new(placeholders);
    match module.body.as_slice() {
        [stmt] => Ok(builder.stmt_to_rust_literal(stmt)),
        [] => Err("expected at least one statement".to_string()),
        _ => Err("expected exactly one statement".to_string()),
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

    #[test]
    fn builds_placeholder_bindings() {
        let literal = build_stmt_literal("{target} = {ref value}")
            .expect("should build literal with placeholders");
        assert_eq!(
            literal,
            "StmtNode::Assign { info: (), target: target, value: ref value }"
        );
    }

    #[test]
    fn builds_mut_placeholder_binding() {
        let literal = build_stmt_literal("return {mut value}").expect("should build literal");
        assert_eq!(
            literal,
            "StmtNode::Return { info: (), value: Some(ref mut value) }"
        );
    }
}

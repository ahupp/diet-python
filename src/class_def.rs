use ruff_python_ast::str::{Quote, TripleQuotes};
use ruff_python_ast::str_prefix::StringLiteralPrefix;
use ruff_python_ast::visitor::transformer::{walk_stmt, Transformer};
use ruff_python_ast::{self as ast, Expr, Stmt};
use ruff_text_size::TextRange;

pub struct ClassDefRewriter;

impl ClassDefRewriter {
    pub fn new() -> Self {
        Self
    }
}

fn string_expr(value: &str) -> Expr {
    let flags = ast::StringLiteralFlags::empty()
        .with_quote_style(Quote::Double)
        .with_triple_quotes(TripleQuotes::No)
        .with_prefix(StringLiteralPrefix::Empty);
    let literal = ast::StringLiteral {
        range: TextRange::default(),
        node_index: ast::AtomicNodeIndex::default(),
        value: value.into(),
        flags,
    };
    Expr::StringLiteral(ast::ExprStringLiteral {
        node_index: ast::AtomicNodeIndex::default(),
        range: TextRange::default(),
        value: ast::StringLiteralValue::single(literal),
    })
}

impl Transformer for ClassDefRewriter {
    fn visit_stmt(&self, stmt: &mut Stmt) {
        if let Stmt::ClassDef(ast::StmtClassDef { name, body, arguments, .. }) = stmt {
            for stmt in body.iter_mut() {
                self.visit_stmt(stmt);
            }

            let class_name = name.id.as_str().to_string();
            let ns_func_name = format!("_dp_ns_{}", class_name);
            let class_func_name = format!("_class_{}", class_name);

            // Build namespace function body
            let mut ns_body = Vec::new();
            ns_body.push(crate::py_stmt!("_dp_temp_ns = {}"));
            ns_body.push(crate::py_stmt!(
                "_dp_temp_ns[\"__module__\"] = _ns[\"__module__\"] = __name__"
            ));
            // TODO: correctly calculate the qualname of the class when nested
            ns_body.push(crate::py_stmt!(
                "_dp_temp_ns[\"__qualname__\"] = _ns[\"__qualname__\"] = {q:expr}",
                q = string_expr(&class_name)
            ));

            let mut original_body = std::mem::take(body);
            if let Some(Stmt::Expr(ast::StmtExpr { value, .. })) = original_body.first() {
                if matches!(value.as_ref(), Expr::StringLiteral(_)) {
                    if let Expr::StringLiteral(s) = value.as_ref() {
                        ns_body.push(crate::py_stmt!(
                            "_dp_temp_ns[\"__doc__\"] = _ns[\"__doc__\"] = {doc:expr}",
                            doc = Expr::StringLiteral(s.clone())
                        ));
                    }
                    original_body.remove(0);
                }
            }

            for stmt in original_body {
                match stmt {
                    Stmt::Assign(ast::StmtAssign { targets, value, .. }) => {
                        if let [Expr::Name(ast::ExprName { id, .. })] = targets.as_slice() {
                            ns_body.push(crate::py_stmt!(
                                "_dp_temp_ns[{k1:expr}] = _ns[{k2:expr}] = {v:expr}",
                                k1 = string_expr(id.as_str()),
                                k2 = string_expr(id.as_str()),
                                v = value
                            ));
                        }
                    }
                    Stmt::AnnAssign(ast::StmtAnnAssign { target, value: Some(v), .. }) => {
                        if let Expr::Name(ast::ExprName { id, .. }) = target.as_ref() {
                            ns_body.push(crate::py_stmt!(
                                "_dp_temp_ns[{k1:expr}] = _ns[{k2:expr}] = {v:expr}",
                                k1 = string_expr(id.as_str()),
                                k2 = string_expr(id.as_str()),
                                v = *v
                            ));
                        }
                    }
                    Stmt::FunctionDef(func_def) => {
                        let fn_name = func_def.name.id.to_string();
                        let mk_name = format!("_mk_{}", fn_name);
                        let mk_body = vec![
                            Stmt::FunctionDef(func_def),
                            crate::py_stmt!(
                                "{fn_name:id}.__qualname__ = _ns[\"__qualname__\"] + {suffix:expr}",
                                fn_name = fn_name.as_str(),
                                suffix = string_expr(&format!(".{}", fn_name))
                            ),
                            crate::py_stmt!("return {fn_name:id}", fn_name = fn_name.as_str()),
                        ];
                        ns_body.push(crate::py_stmt!(
                            "def {mk_name:id}():\n    {body:stmt}",
                            mk_name = mk_name.as_str(),
                            body = mk_body
                        ));
                        ns_body.push(crate::py_stmt!(
                            "_dp_temp_ns[{k1:expr}] = _ns[{k2:expr}] = {mk_name:id}()",
                            k1 = string_expr(&fn_name),
                            k2 = string_expr(&fn_name),
                            mk_name = mk_name.as_str()
                        ));
                    }
                    other => ns_body.push(other),
                }
            }

            let ns_func = crate::py_stmt!(
                "def {name:id}(_ns):\n    {body:stmt}",
                name = ns_func_name.as_str(),
                body = ns_body
            );

            // Build class helper function
            let mut bases = Vec::new();
            let mut kw_keys = Vec::new();
            let mut kw_vals = Vec::new();
            if let Some(args) = arguments.take() {
                bases.extend(args.args.into_vec());
                for kw in args.keywords.into_vec() {
                    if let Some(arg) = kw.arg {
                        kw_keys.push(string_expr(arg.as_str()));
                        kw_vals.push(kw.value);
                    }
                }
            }
            let has_kw = !kw_keys.is_empty();

            let bases_tuple = Expr::Tuple(ast::ExprTuple {
                node_index: ast::AtomicNodeIndex::default(),
                range: TextRange::default(),
                elts: bases,
                ctx: ast::ExprContext::Load,
                parenthesized: true,
            });

            let bases_stmt = crate::py_stmt!(
                "bases = dp_intrinsics.resolve_bases({b:expr})",
                b = bases_tuple
            );

            let prepare_stmt = if has_kw {
                let items: Vec<ast::DictItem> = kw_keys
                    .into_iter()
                    .zip(kw_vals.into_iter())
                    .map(|(k, v)| ast::DictItem { key: Some(k), value: v })
                    .collect();
                let dict_expr = Expr::Dict(ast::ExprDict {
                    node_index: ast::AtomicNodeIndex::default(),
                    range: TextRange::default(),
                    items,
                });
                crate::py_stmt!(
                    "meta, ns, kwds = dp_intrinsics.prepare_class({n:expr}, bases, {d:expr})",
                    n = string_expr(&class_name),
                    d = dict_expr
                )
            } else {
                crate::py_stmt!(
                    "meta, ns, kwds = dp_intrinsics.prepare_class({n:expr}, bases)",
                    n = string_expr(&class_name)
                )
            };

            let exec_stmt = crate::py_stmt!(
                "{ns_func:id}(ns)",
                ns_func = ns_func_name.as_str()
            );

            let cls_stmt = if has_kw {
                crate::py_stmt!(
                    "cls = meta({n:expr}, bases, ns, **kwds)",
                    n = string_expr(&class_name)
                )
            } else {
                crate::py_stmt!(
                    "cls = meta({n:expr}, bases, ns)",
                    n = string_expr(&class_name)
                )
            };

            let ret_stmt = crate::py_stmt!("return cls");

            let class_func = crate::py_stmt!(
                "def {name:id}():\n    {body:stmt}",
                name = class_func_name.as_str(),
                body = vec![bases_stmt, prepare_stmt, exec_stmt, cls_stmt, ret_stmt]
            );

            let call_stmt = crate::py_stmt!(
                "{cls_name:id} = {builder:id}()",
                cls_name = class_name.as_str(),
                builder = class_func_name.as_str()
            );

            *stmt = crate::py_stmt!(
                "{body:stmt}",
                body = vec![ns_func, class_func, call_stmt]
            );
        } else {
            walk_stmt(self, stmt);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assert_flatten_eq;
    use ruff_python_ast::visitor::transformer::walk_body;
    use ruff_python_parser::parse_module;

    fn rewrite(source: &str) -> Vec<Stmt> {
        let parsed = parse_module(source).expect("parse error");
        let mut module = parsed.into_syntax();
        let rewriter = ClassDefRewriter::new();
        walk_body(&rewriter, &mut module.body);
        module.body
    }

    #[test]
    fn lowers_simple_class() {
        let input = r#"
class C:
    x = 1
"#;
        let expected = r#"
def _dp_ns_C(_ns):
    _dp_temp_ns = {}
    _dp_temp_ns["__module__"] = _ns["__module__"] = __name__
    _dp_temp_ns["__qualname__"] = _ns["__qualname__"] = "C"
    _dp_temp_ns["x"] = _ns["x"] = 1
def _class_C():
    bases = dp_intrinsics.resolve_bases(())
    meta, ns, kwds = dp_intrinsics.prepare_class("C", bases)
    _dp_ns_C(ns)
    cls = meta("C", bases, ns)
    return cls
C = _class_C()
"#;
        let output = rewrite(input);
        assert_flatten_eq!(output, expected);
    }

    #[test]
    fn lowers_inherits() {
        let input = r#"
class C(B):
    pass
"#;
        let expected = r#"
def _dp_ns_C(_ns):
    _dp_temp_ns = {}
    _dp_temp_ns["__module__"] = _ns["__module__"] = __name__
    _dp_temp_ns["__qualname__"] = _ns["__qualname__"] = "C"
    pass
def _class_C():
    bases = dp_intrinsics.resolve_bases((B,))
    meta, ns, kwds = dp_intrinsics.prepare_class("C", bases)
    _dp_ns_C(ns)
    cls = meta("C", bases, ns)
    return cls
C = _class_C()
"#;
        let output = rewrite(input);
        assert_flatten_eq!(output, expected);
    }

    #[test]
    fn lowers_with_docstring_and_keywords() {
        let input = r#"
class C(B, metaclass=Meta, kw=1):
    'doc'
    x = 2
"#;
        let expected = r#"
def _dp_ns_C(_ns):
    _dp_temp_ns = {}
    _dp_temp_ns["__module__"] = _ns["__module__"] = __name__
    _dp_temp_ns["__qualname__"] = _ns["__qualname__"] = "C"
    _dp_temp_ns["__doc__"] = _ns["__doc__"] = 'doc'
    _dp_temp_ns["x"] = _ns["x"] = 2
def _class_C():
    bases = dp_intrinsics.resolve_bases((B,))
    meta, ns, kwds = dp_intrinsics.prepare_class("C", bases, {"metaclass": Meta, "kw": 1})
    _dp_ns_C(ns)
    cls = meta("C", bases, ns, **kwds)
    return cls
C = _class_C()
"#;
        let output = rewrite(input);
        assert_flatten_eq!(output, expected);
    }

    #[test]
    fn lowers_method() {
        let input = r#"
class C:
    def m(self):
        return 1
"#;
        let expected = r#"
def _dp_ns_C(_ns):
    _dp_temp_ns = {}
    _dp_temp_ns["__module__"] = _ns["__module__"] = __name__
    _dp_temp_ns["__qualname__"] = _ns["__qualname__"] = "C"
    def _mk_m():
        def m(self):
            return 1
        m.__qualname__ = _ns["__qualname__"] + ".m"
        return m
    _dp_temp_ns["m"] = _ns["m"] = _mk_m()
def _class_C():
    bases = dp_intrinsics.resolve_bases(())
    meta, ns, kwds = dp_intrinsics.prepare_class("C", bases)
    _dp_ns_C(ns)
    cls = meta("C", bases, ns)
    return cls
C = _class_C()
"#;
        let output = rewrite(input);
        assert_flatten_eq!(output, expected);
    }
}


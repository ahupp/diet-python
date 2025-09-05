#[macro_export]
macro_rules! py_expr {
    ($template:literal $(, $name:ident = $value:expr)* $(,)?) => {{
        use ruff_python_parser::parse_expression;
        use ruff_python_ast::{self as ast, Expr, Identifier, name::Name};
        use ruff_python_ast::visitor::transformer::{Transformer, walk_expr};
        use ruff_text_size::TextRange;
        use std::collections::HashMap;

        #[allow(dead_code)]
        struct PlaceholderRewriter<'a> {
            placeholder: &'a str,
            replacement: &'a Expr,
        }

        impl<'a> Transformer for PlaceholderRewriter<'a> {
            fn visit_expr(&self, expr: &mut Expr) {
                if let Expr::Name(ast::ExprName { id, .. }) = expr {
                    if id.as_str() == self.placeholder {
                        *expr = self.replacement.clone();
                        return;
                    }
                }
                walk_expr(self, expr);
            }
        }

        #[allow(dead_code)]
        struct IdentRewriter<'a> {
            placeholder: &'a str,
            replacement: &'a str,
        }

        impl<'a> Transformer for IdentRewriter<'a> {
            fn visit_expr(&self, expr: &mut Expr) {
                match expr {
                    Expr::Attribute(ast::ExprAttribute { attr, .. }) => {
                        if attr.id.as_str() == self.placeholder {
                            *attr = Identifier::new(
                                Name::new(self.replacement.to_string()),
                                TextRange::default(),
                            );
                        }
                    }
                    Expr::Name(ast::ExprName { id, .. }) => {
                        if id.as_str() == self.placeholder {
                            *id = Name::new(self.replacement.to_string());
                        }
                    }
                    _ => {}
                }
                walk_expr(self, expr);
            }
        }

        enum PlaceholderKind {
            Expr,
            Id,
        }

        enum PlaceholderValue {
            Expr(Box<Expr>),
            Id(String),
        }

        impl From<Expr> for PlaceholderValue {
            fn from(value: Expr) -> Self {
                Self::Expr(Box::new(value))
            }
        }

        impl From<&str> for PlaceholderValue {
            fn from(value: &str) -> Self {
                Self::Id(value.to_string())
            }
        }

        impl From<String> for PlaceholderValue {
            fn from(value: String) -> Self {
                Self::Id(value)
            }
        }

        let mut src = String::new();
        let mut placeholders: HashMap<String, PlaceholderKind> = HashMap::new();
        let mut rest = $template;
        while let Some(start) = rest.find('{') {
            src.push_str(&rest[..start]);
            rest = &rest[start + 1..];
            if let Some(end) = rest.find('}') {
                let inner = &rest[..end];
                rest = &rest[end + 1..];
                if let Some((name, kind)) = inner.split_once(':') {
                    let kind = match kind {
                        "expr" => PlaceholderKind::Expr,
                        "id" => PlaceholderKind::Id,
                        _ => panic!("unknown placeholder type: {kind}"),
                    };
                    let marker = match kind {
                        PlaceholderKind::Expr => format!("__dp_placeholder_{}__", name),
                        PlaceholderKind::Id => format!("__dp_placeholder_ident_{}__", name),
                    };
                    placeholders.insert(name.to_string(), kind);
                    src.push_str(&marker);
                } else {
                    src.push('{');
                    src.push_str(inner);
                    src.push('}');
                }
            } else {
                src.push('{');
                src.push_str(rest);
                break;
            }
        }
        src.push_str(rest);

        let mut expr = *parse_expression(&src)
            .expect("template parse error")
            .into_syntax()
            .body;

        let mut values: HashMap<&str, PlaceholderValue> = HashMap::new();
        $(values.insert(stringify!($name), PlaceholderValue::from($value));)*

        for (name, kind) in placeholders {
            match (kind, values.remove(name.as_str())) {
                (PlaceholderKind::Expr, Some(PlaceholderValue::Expr(value))) => {
                    let placeholder = format!("__dp_placeholder_{}__", name);
                    let rewriter = PlaceholderRewriter {
                        placeholder: &placeholder,
                        replacement: &value,
                    };
                    rewriter.visit_expr(&mut expr);
                }
                (PlaceholderKind::Id, Some(PlaceholderValue::Id(value))) => {
                    let placeholder = format!("__dp_placeholder_ident_{}__", name);
                    let rewriter = IdentRewriter {
                        placeholder: &placeholder,
                        replacement: value.as_str(),
                    };
                    rewriter.visit_expr(&mut expr);
                }
                (PlaceholderKind::Expr, _) => {
                    panic!("expected expr for placeholder {name}");
                }
                (PlaceholderKind::Id, _) => {
                    panic!("expected id for placeholder {name}");
                }
            }
        }

        expr
    }};
}

#[cfg(test)]
mod tests {
    use ruff_python_ast::comparable::ComparableExpr;
    use ruff_python_parser::parse_expression;

    #[test]
    fn inserts_placeholder() {
        let fragment = *parse_expression("2").unwrap().into_syntax().body;
        let expr = py_expr!("1 + {two:expr}", two = fragment);
        let expected = *parse_expression("1 + 2").unwrap().into_syntax().body;
        assert_eq!(ComparableExpr::from(&expr), ComparableExpr::from(&expected));
    }

    #[test]
    fn inserts_identifier() {
        let expr = py_expr!("operator.{func:id}(1)", func = "add");
        let expected = *parse_expression("operator.add(1)")
            .unwrap()
            .into_syntax()
            .body;
        assert_eq!(ComparableExpr::from(&expr), ComparableExpr::from(&expected));
    }
}

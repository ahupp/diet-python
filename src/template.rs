#[macro_export]
macro_rules! py_expr {
    ($template:literal $(, $name:ident = $value:expr)* $(; id $id_name:ident = $id_value:expr)* $(,)?) => {{
        use ruff_python_parser::parse_expression;
        use ruff_python_ast::{self as ast, Expr, Identifier, name::Name};
        use ruff_python_ast::visitor::transformer::{Transformer, walk_expr};
        use ruff_text_size::TextRange;

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

        let mut src = $template.to_string();
        $(
            let placeholder = concat!("__dp_placeholder_", stringify!($name), "__");
            let marker = format!("{{{}}}", stringify!($name));
            src = src.replace(&marker, placeholder);
        )*
        $(
            let placeholder = concat!("__dp_placeholder_ident_", stringify!($id_name), "__");
            let marker = format!("{{{}}}", stringify!($id_name));
            src = src.replace(&marker, placeholder);
        )*

        let mut expr = *parse_expression(&src)
            .expect("template parse error")
            .into_syntax()
            .body;

        $(
            let rewriter = PlaceholderRewriter {
                placeholder: concat!("__dp_placeholder_", stringify!($name), "__"),
                replacement: &$value,
            };
            rewriter.visit_expr(&mut expr);
        )*
        $(
            let rewriter = IdentRewriter {
                placeholder: concat!("__dp_placeholder_ident_", stringify!($id_name), "__"),
                replacement: $id_value,
            };
            rewriter.visit_expr(&mut expr);
        )*

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
        let expr = py_expr!("1 + {two}", two = fragment);
        let expected = *parse_expression("1 + 2").unwrap().into_syntax().body;
        assert_eq!(ComparableExpr::from(&expr), ComparableExpr::from(&expected));
    }

    #[test]
    fn inserts_identifier() {
        let expr = py_expr!("operator.{func}(1)"; id func = "add");
        let expected = *parse_expression("operator.add(1)").unwrap().into_syntax().body;
        assert_eq!(ComparableExpr::from(&expr), ComparableExpr::from(&expected));
    }
}

use crate::{py_expr, template::make_tuple};
use ruff_python_ast::{self as ast, Expr};

fn join_parts(parts: Vec<Expr>) -> Expr {
    if parts.len() == 1 {
        parts.into_iter().next().unwrap()
    } else {
        let tuple = make_tuple(parts);
        py_expr!(r#"".join({tuple:expr})"#, tuple = tuple)
    }
}

fn rewrite_elements(elements: &ast::InterpolatedStringElements, parts: &mut Vec<Expr>) {
    for element in elements.iter() {
        match element {
            ast::InterpolatedStringElement::Literal(lit) => {
                parts.push(py_expr!("{literal:literal}", literal = lit.value.as_ref()));
            }
            ast::InterpolatedStringElement::Interpolation(interp) => {
                let value = (*interp.expression).clone();
                parts.push(py_expr!("str({value:expr})", value = value,));
            }
        }
    }
}

pub fn rewrite_fstring(expr: ast::ExprFString) -> Expr {
    let mut parts = Vec::new();
    for part in expr.value.iter() {
        match part {
            ast::FStringPart::Literal(lit) => {
                parts.push(py_expr!("{literal:literal}", literal = lit.value.as_ref()));
            }
            ast::FStringPart::FString(f) => {
                rewrite_elements(&f.elements, &mut parts);
            }
        }
    }
    join_parts(parts)
}

pub fn rewrite_tstring(expr: ast::ExprTString) -> Expr {
    let mut parts = Vec::new();
    for t in expr.value.iter() {
        rewrite_elements(&t.elements, &mut parts);
    }
    join_parts(parts)
}

#[cfg(test)]
mod tests {
    use crate::test_util::assert_transform_eq;

    #[test]
    fn desugars_fstring() {
        let input = r#"
f"x={x}"
"#;
        let expected = r#"
"".join(("x=", str(x)))
"#;
        assert_transform_eq(input, expected);
    }

    #[test]
    fn desugars_tstring() {
        let input = r#"
t"x={x}"
"#;
        let expected = r#"
"".join(("x=", str(x)))
"#;
        assert_transform_eq(input, expected);
    }
}

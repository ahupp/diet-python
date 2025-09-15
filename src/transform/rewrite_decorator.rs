use std::cell::Cell;

use ruff_python_ast::{self as ast, Stmt};

/// Rewrite decorated functions and classes into explicit decorator applications.
pub fn rewrite(
    decorators: Vec<ast::Decorator>,
    name: &str,
    item: Stmt,
    base: Option<&str>,
    count: &Cell<usize>,
) -> Stmt {
    let mut assigns = Vec::new();
    let mut names = Vec::new();

    for decorator in decorators {
        if let ast::Expr::Name(ast::ExprName { id, .. }) = &decorator.expression {
            names.push(id.to_string());
        } else {
            let id = count.get() + 1;
            count.set(id);
            let tmp = format!("_dp_dec_{}", id);
            let assign = crate::py_stmt!(
                "{name:id} = {decorator:expr}",
                name = tmp.as_str(),
                decorator = decorator.expression,
            );
            assigns.push(assign);
            names.push(tmp);
        }
    }

    let mut call_expr = if let Some(base_name) = base {
        crate::py_expr!("{name:id}", name = base_name)
    } else {
        crate::py_expr!("{name:id}", name = name)
    };
    for decorator in names.iter().rev() {
        call_expr = crate::py_expr!(
            "{decorator:id}({expr:expr})",
            decorator = decorator.as_str(),
            expr = call_expr,
        );
    }
    let call_stmt = crate::py_stmt!("{name:id} = {expr:expr}", name = name, expr = call_expr);

    let mut body = assigns;
    body.push(item);
    body.push(call_stmt);
    crate::py_stmt!("{body:stmt}", body = body)
}

#[cfg(test)]
mod tests {
    use crate::test_util::assert_transform_eq;

    #[test]
    fn rewrites_function_decorators() {
        let input = r#"
@dec2(5)
@dec1
def foo():
    pass
"#;
        let expected = r#"
_dp_dec_1 = dec2(5)
def foo():
    pass
foo = _dp_dec_1(dec1(foo))
"#;
        assert_transform_eq(input, expected);
    }

    #[test]
    fn rewrites_class_decorators() {
        let input = r#"
@dec
class C:
    pass
"#;
        let expected = r#"
def _dp_ns_C(_ns):
    _dp_temp_ns = dict(())
    _dp_tmp_1 = __name__
    __dp__.setitem(_dp_temp_ns, "__module__", _dp_tmp_1)
    __dp__.setitem(_ns, "__module__", _dp_tmp_1)
    _dp_tmp_2 = "C"
    __dp__.setitem(_dp_temp_ns, "__qualname__", _dp_tmp_2)
    __dp__.setitem(_ns, "__qualname__", _dp_tmp_2)
    pass
def _class_C():
    bases = __dp__.resolve_bases(())
    _dp_tmp_3 = __dp__.prepare_class("C", bases)
    meta = __dp__.getitem(_dp_tmp_3, 0)
    ns = __dp__.getitem(_dp_tmp_3, 1)
    kwds = __dp__.getitem(_dp_tmp_3, 2)
    _dp_ns_C(ns)
    cls = meta("C", bases, ns)
    return cls
_dp_class_C = _class_C()
C = dec(_dp_class_C)
"#;
        assert_transform_eq(input, expected);
    }

    #[test]
    fn rewrites_multiple_class_decorators() {
        let input = r#"
@dec2(5)
@dec1
class C:
    pass
"#;
        let expected = r#"
_dp_dec_1 = dec2(5)
def _dp_ns_C(_ns):
    _dp_temp_ns = dict(())
    _dp_tmp_1 = __name__
    __dp__.setitem(_dp_temp_ns, "__module__", _dp_tmp_1)
    __dp__.setitem(_ns, "__module__", _dp_tmp_1)
    _dp_tmp_2 = "C"
    __dp__.setitem(_dp_temp_ns, "__qualname__", _dp_tmp_2)
    __dp__.setitem(_ns, "__qualname__", _dp_tmp_2)
    pass
def _class_C():
    bases = __dp__.resolve_bases(())
    _dp_tmp_3 = __dp__.prepare_class("C", bases)
    meta = __dp__.getitem(_dp_tmp_3, 0)
    ns = __dp__.getitem(_dp_tmp_3, 1)
    kwds = __dp__.getitem(_dp_tmp_3, 2)
    _dp_ns_C(ns)
    cls = meta("C", bases, ns)
    return cls
_dp_class_C = _class_C()
C = _dp_dec_1(dec1(_dp_class_C))
"#;
        assert_transform_eq(input, expected);
    }
}

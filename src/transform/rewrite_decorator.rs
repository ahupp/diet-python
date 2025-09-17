use super::context::Context;
use ruff_python_ast::{self as ast, Stmt};

use crate::{py_expr, py_stmt};

/// Rewrite decorated functions and classes into explicit decorator applications.
pub fn rewrite(
    decorators: Vec<ast::Decorator>,
    name: &str,
    item: Stmt,
    base: Option<&str>,
    ctx: &Context,
) -> Stmt {
    let decorator_expr =
        decorators
            .into_iter()
            .rev()
            .fold(py_expr!("_dp_the_func"), |acc, decorator| {
                py_expr!(
                    "{decorator:expr}({acc:expr})",
                    decorator = decorator.expression,
                    acc = acc
                )
            });

    let base_or_name = base.unwrap_or(name);

    let dec_apply_fn = ctx.fresh("dec_apply");

    py_stmt!(
        r#"
def {dec_apply_fn:id}(_dp_the_func):
    return {decorator_expr:expr}
{item:stmt}
{base_or_name:id} = {dec_apply_fn:id}({base_or_name:id})"#,
        dec_apply_fn = dec_apply_fn.as_str(),
        decorator_expr = decorator_expr,
        item = item,
        base_or_name = base_or_name,
    )
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
def _dp_dec_apply_1(_dp_the_func):
    return dec2(5)(dec1(_dp_the_func))
def foo():
    pass
foo = _dp_dec_apply_1(foo)
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
def _dp_dec_apply_5(_dp_the_func):
    return dec(_dp_the_func)
def _dp_ns_C(_ns):
    _dp_temp_ns = dict(())
    _dp_tmp_1 = __name__
    __dp__.setitem(_dp_temp_ns, "__module__", _dp_tmp_1)
    __dp__.setitem(_ns, "__module__", _dp_tmp_1)
    _dp_tmp_2 = "C"
    __dp__.setitem(_dp_temp_ns, "__qualname__", _dp_tmp_2)
    __dp__.setitem(_ns, "__qualname__", _dp_tmp_2)
    pass
def _dp_make_class_C():
    bases = __dp__.resolve_bases(())
    _dp_tmp_3 = __dp__.prepare_class("C", bases, None)
    meta = __dp__.getitem(_dp_tmp_3, 0)
    ns = __dp__.getitem(_dp_tmp_3, 1)
    kwds = __dp__.getitem(_dp_tmp_3, 2)
    _dp_ns_C(ns)
    return meta("C", bases, ns, **kwds)
_dp_tmp_4 = _dp_make_class_C()
C = _dp_tmp_4
_dp_class_C = _dp_tmp_4
_dp_class_C = _dp_dec_apply_5(_dp_class_C)
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
def _dp_dec_apply_5(_dp_the_func):
    return dec2(5)(dec1(_dp_the_func))
def _dp_ns_C(_ns):
    _dp_temp_ns = dict(())
    _dp_tmp_1 = __name__
    __dp__.setitem(_dp_temp_ns, "__module__", _dp_tmp_1)
    __dp__.setitem(_ns, "__module__", _dp_tmp_1)
    _dp_tmp_2 = "C"
    __dp__.setitem(_dp_temp_ns, "__qualname__", _dp_tmp_2)
    __dp__.setitem(_ns, "__qualname__", _dp_tmp_2)
    pass
def _dp_make_class_C():
    bases = __dp__.resolve_bases(())
    _dp_tmp_3 = __dp__.prepare_class("C", bases, None)
    meta = __dp__.getitem(_dp_tmp_3, 0)
    ns = __dp__.getitem(_dp_tmp_3, 1)
    kwds = __dp__.getitem(_dp_tmp_3, 2)
    _dp_ns_C(ns)
    return meta("C", bases, ns, **kwds)
_dp_tmp_4 = _dp_make_class_C()
C = _dp_tmp_4
_dp_class_C = _dp_tmp_4
_dp_class_C = _dp_dec_apply_5(_dp_class_C)
"#;
        assert_transform_eq(input, expected);
    }
}

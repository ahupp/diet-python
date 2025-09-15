use std::cell::RefCell;

use super::{
    context::Context, rewrite_assert, rewrite_class_def, rewrite_decorator, rewrite_for_loop,
    rewrite_import, rewrite_match_case, rewrite_string, rewrite_try_except, rewrite_with, Options,
};
use crate::template::{
    is_simple, make_binop, make_generator, make_tuple, make_unaryop, single_stmt,
};
use crate::{py_expr, py_stmt};
use ruff_python_ast::name::Name;
use ruff_python_ast::visitor::transformer::{walk_expr, walk_stmt, Transformer};
use ruff_python_ast::{self as ast, CmpOp, Expr, Operator, Stmt, UnaryOp};
use ruff_text_size::TextRange;

pub struct ExprRewriter<'a> {
    ctx: &'a Context,
    options: Options,
    scopes: RefCell<Vec<Vec<Stmt>>>,
}

impl<'a> ExprRewriter<'a> {
    pub fn new(ctx: &'a Context) -> Self {
        Self {
            options: ctx.options,
            ctx,
            scopes: RefCell::new(Vec::new()),
        }
    }

    fn tempify(&self, expr: &mut Expr, stmts: &mut Vec<Stmt>) -> Expr {
        if !is_simple(expr) {
            let tmp = self.ctx.fresh("tmp");
            let value = expr.clone();
            let assign = py_stmt!(
                "\n{tmp:id} = {expr:expr}\n",
                tmp = tmp.as_str(),
                expr = value,
            );
            stmts.push(assign);
            py_expr!("{tmp:id}\n", tmp = tmp.as_str())
        } else {
            expr.clone()
        }
    }

    pub fn rewrite_body(&self, body: &mut Vec<Stmt>) {
        self.push_scope();
        for stmt in body.iter_mut() {
            self.visit_stmt(stmt);
        }
        let functions = self.pop_scope();
        if !functions.is_empty() {
            body.splice(0..0, functions);
        }
    }

    fn push_scope(&self) {
        self.scopes.borrow_mut().push(Vec::new());
    }

    fn pop_scope(&self) -> Vec<Stmt> {
        self.scopes.borrow_mut().pop().unwrap()
    }

    fn add_function(&self, func: Stmt) {
        self.scopes.borrow_mut().last_mut().unwrap().push(func);
    }

    fn rewrite_target(&self, target: Expr, value: Expr, out: &mut Vec<Stmt>) {
        match target {
            Expr::Tuple(tuple) => {
                let tmp_name = self.ctx.fresh("tmp");
                let tmp_expr = py_expr!(
                    "
{name:id}
",
                    name = tmp_name.as_str(),
                );
                let mut tmp_stmt = py_stmt!(
                    "
{name:id} = {value:expr}
",
                    name = tmp_name.as_str(),
                    value = value,
                );
                walk_stmt(self, &mut tmp_stmt);
                out.push(tmp_stmt);
                for (i, elt) in tuple.elts.into_iter().enumerate() {
                    let mut elt_stmt = py_stmt!(
                        "
{target:expr} = {tmp:expr}[{idx:literal}]
",
                        target = elt,
                        tmp = tmp_expr.clone(),
                        idx = i,
                    );
                    walk_stmt(self, &mut elt_stmt);
                    out.push(elt_stmt);
                }
            }
            Expr::List(list) => {
                let tmp_name = self.ctx.fresh("tmp");
                let tmp_expr = py_expr!(
                    "
{name:id}
",
                    name = tmp_name.as_str(),
                );
                let mut tmp_stmt = py_stmt!(
                    "
{name:id} = {value:expr}
",
                    name = tmp_name.as_str(),
                    value = value,
                );
                walk_stmt(self, &mut tmp_stmt);
                out.push(tmp_stmt);
                for (i, elt) in list.elts.into_iter().enumerate() {
                    let mut elt_stmt = py_stmt!(
                        "
{target:expr} = {tmp:expr}[{idx:literal}]
",
                        target = elt,
                        tmp = tmp_expr.clone(),
                        idx = i,
                    );
                    walk_stmt(self, &mut elt_stmt);
                    out.push(elt_stmt);
                }
            }
            Expr::Attribute(attr) => {
                let obj = (*attr.value).clone();
                let mut stmt = py_stmt!(
                    "
__dp__.setattr({obj:expr}, {name:literal}, {value:expr})
",
                    obj = obj,
                    name = attr.attr.as_str(),
                    value = value,
                );
                walk_stmt(self, &mut stmt);
                out.push(stmt);
            }
            Expr::Subscript(sub) => {
                let obj = (*sub.value).clone();
                let key = (*sub.slice).clone();
                let mut stmt = py_stmt!(
                    "
__dp__.setitem({obj:expr}, {key:expr}, {value:expr})
",
                    obj = obj,
                    key = key,
                    value = value,
                );
                walk_stmt(self, &mut stmt);
                out.push(stmt);
            }
            Expr::Name(_) => {
                let mut stmt = py_stmt!(
                    "
{target:expr} = {value:expr}
",
                    target = target,
                    value = value,
                );
                walk_stmt(self, &mut stmt);
                out.push(stmt);
            }
            _ => {
                panic!("unsupported assignment target");
            }
        }
    }
}

impl<'a> Transformer for ExprRewriter<'a> {
    fn visit_expr(&self, expr: &mut Expr) {
        if let Expr::Lambda(lambda) = expr {
            let func_name = self.ctx.fresh("lambda");

            let parameters = lambda
                .parameters
                .as_ref()
                .map(|params| (**params).clone())
                .unwrap_or_else(|| ast::Parameters {
                    range: TextRange::default(),
                    node_index: ast::AtomicNodeIndex::default(),
                    posonlyargs: vec![],
                    args: vec![],
                    vararg: None,
                    kwonlyargs: vec![],
                    kwarg: None,
                });

            let mut func_def = py_stmt!(
                r#"
def {func:id}():
    return {body:expr}"#,
                func = func_name.as_str(),
                body = (*lambda.body).clone(),
            );

            if let Stmt::FunctionDef(ast::StmtFunctionDef {
                parameters: params, ..
            }) = &mut func_def
            {
                *params = Box::new(parameters);
            }

            walk_stmt(self, &mut func_def);
            self.add_function(func_def);

            *expr = py_expr!("{func:id}", func = func_name.as_str(),);
        } else if let Expr::Generator(gen) = expr {
            let first_iter_expr = gen.generators.first().unwrap().iter.clone();

            // Avoid using a double underscore prefix for generated function names.
            // Python name mangling affects identifiers starting with two underscores
            // inside class bodies; use a single underscore to keep helpers hidden
            // without triggering mangling.
            let func_name = self.ctx.fresh("gen");

            let param_name = if let Expr::Name(ast::ExprName { id, .. }) = &first_iter_expr {
                id.clone()
            } else {
                Name::new(self.ctx.fresh("iter"))
            };

            let mut body = vec![py_stmt!("\nyield {value:expr}", value = (*gen.elt).clone(),)];

            for comp in gen.generators.iter().rev() {
                let mut inner = body;
                for if_expr in comp.ifs.iter().rev() {
                    inner = vec![py_stmt!(
                        "if {test:expr}:\n    {body:stmt}",
                        test = if_expr.clone(),
                        body = inner,
                    )];
                }
                body = vec![if comp.is_async {
                    py_stmt!(
                        "async for {target:expr} in {iter:expr}:\n    {body:stmt}",
                        target = comp.target.clone(),
                        iter = comp.iter.clone(),
                        body = inner,
                    )
                } else {
                    py_stmt!(
                        "for {target:expr} in {iter:expr}:\n    {body:stmt}",
                        target = comp.target.clone(),
                        iter = comp.iter.clone(),
                        body = inner,
                    )
                }];
            }

            if let Stmt::For(ast::StmtFor { iter, .. }) = body.first_mut().unwrap() {
                *iter = Box::new(py_expr!("{name:id}", name = param_name.as_str(),));
            }

            let mut func_def = py_stmt!(
                "\ndef {func:id}({param:id}):\n    {body:stmt}",
                func = func_name.as_str(),
                param = param_name.as_str(),
                body = body,
            );

            walk_stmt(self, &mut func_def);
            self.add_function(func_def);

            *expr = py_expr!(
                "{func:id}(__dp__.iter({iter:expr}))",
                iter = first_iter_expr,
                func = func_name.as_str(),
            );
        } else {
            let original = expr.clone();
            *expr = match original {
                Expr::FString(f_string) => rewrite_string::rewrite_fstring(f_string),
                Expr::TString(t_string) => rewrite_string::rewrite_tstring(t_string),
                Expr::Slice(ast::ExprSlice {
                    lower, upper, step, ..
                }) => {
                    fn none_name() -> Expr {
                        py_expr!("None")
                    }
                    let lower_expr = lower.map(|expr| *expr).unwrap_or_else(none_name);
                    let upper_expr = upper.map(|expr| *expr).unwrap_or_else(none_name);
                    let step_expr = step.map(|expr| *expr).unwrap_or_else(none_name);
                    py_expr!(
                        "slice({lower:expr}, {upper:expr}, {step:expr})",
                        lower = lower_expr,
                        upper = upper_expr,
                        step = step_expr,
                    )
                }
                Expr::EllipsisLiteral(_) => {
                    py_expr!("Ellipsis")
                }
                Expr::NumberLiteral(ast::ExprNumberLiteral {
                    value: ast::Number::Complex { real, imag },
                    ..
                }) => {
                    let real_expr = Expr::NumberLiteral(ast::ExprNumberLiteral {
                        node_index: ast::AtomicNodeIndex::default(),
                        range: TextRange::default(),
                        value: ast::Number::Float(real),
                    });
                    let imag_expr = Expr::NumberLiteral(ast::ExprNumberLiteral {
                        node_index: ast::AtomicNodeIndex::default(),
                        range: TextRange::default(),
                        value: ast::Number::Float(imag),
                    });
                    py_expr!(
                        "complex({real:expr}, {imag:expr})",
                        real = real_expr,
                        imag = imag_expr,
                    )
                }
                Expr::Attribute(ast::ExprAttribute {
                    value, attr, ctx, ..
                }) if matches!(ctx, ast::ExprContext::Load) && self.options.lower_attributes => {
                    let value_expr = *value;
                    py_expr!(
                        "getattr({value:expr}, {attr:literal})",
                        value = value_expr,
                        attr = attr.id.as_str(),
                    )
                }
                Expr::NoneLiteral(_) => {
                    py_expr!("None")
                }
                Expr::ListComp(ast::ExprListComp {
                    elt, generators, ..
                }) => py_expr!("list({expr:expr})", expr = make_generator(*elt, generators)),
                Expr::SetComp(ast::ExprSetComp {
                    elt, generators, ..
                }) => py_expr!("set({expr:expr})", expr = make_generator(*elt, generators)),
                Expr::DictComp(ast::ExprDictComp {
                    key,
                    value,
                    generators,
                    ..
                }) => {
                    let tuple = py_expr!("({key:expr}, {value:expr})", key = *key, value = *value,);
                    py_expr!(
                        "dict({expr:expr})",
                        expr = make_generator(tuple, generators)
                    )
                }
                Expr::List(ast::ExprList { elts, ctx, .. })
                    if matches!(ctx, ast::ExprContext::Load) =>
                {
                    let tuple = make_tuple(elts);
                    py_expr!("list({tuple:expr})", tuple = tuple,)
                }
                Expr::Set(ast::ExprSet { elts, .. }) => {
                    let tuple = make_tuple(elts);
                    py_expr!("set({tuple:expr})", tuple = tuple,)
                }
                Expr::Dict(ast::ExprDict { items, .. })
                    if items.iter().all(|item| item.key.is_some()) =>
                {
                    let pairs: Vec<Expr> = items
                        .into_iter()
                        .map(|item| {
                            let key = item.key.unwrap();
                            let value = item.value;
                            py_expr!("({key:expr}, {value:expr})", key = key, value = value,)
                        })
                        .collect();
                    let tuple = make_tuple(pairs);
                    py_expr!("dict({tuple:expr})", tuple = tuple,)
                }
                Expr::If(ast::ExprIf {
                    test, body, orelse, ..
                }) => {
                    let test_expr = *test;
                    let body_expr = *body;
                    let orelse_expr = *orelse;
                    py_expr!(
                        "__dp__.if_expr({cond:expr}, lambda: {body:expr}, lambda: {orelse:expr})",
                        cond = test_expr,
                        body = body_expr,
                        orelse = orelse_expr,
                    )
                }
                Expr::BoolOp(ast::ExprBoolOp { op, mut values, .. }) => {
                    let mut result = values.pop().expect("boolop with no values");
                    while let Some(value) = values.pop() {
                        result = match op {
                            ast::BoolOp::Or => py_expr!(
                                "__dp__.or_expr({left:expr}, lambda: {right:expr})",
                                left = value,
                                right = result,
                            ),
                            ast::BoolOp::And => py_expr!(
                                "__dp__.and_expr({left:expr}, lambda: {right:expr})",
                                left = value,
                                right = result,
                            ),
                        };
                    }
                    result
                }
                Expr::BinOp(ast::ExprBinOp {
                    left, right, op, ..
                }) => {
                    let func_name = match op {
                        Operator::Add => "add",
                        Operator::Sub => "sub",
                        Operator::Mult => "mul",
                        Operator::MatMult => "matmul",
                        Operator::Div => "truediv",
                        Operator::Mod => "mod",
                        Operator::Pow => "pow",
                        Operator::LShift => "lshift",
                        Operator::RShift => "rshift",
                        Operator::BitOr => "or_",
                        Operator::BitXor => "xor",
                        Operator::BitAnd => "and_",
                        Operator::FloorDiv => "floordiv",
                    };
                    make_binop(func_name, *left, *right)
                }
                Expr::UnaryOp(ast::ExprUnaryOp { operand, op, .. }) => {
                    let func_name = match op {
                        UnaryOp::Not => "not_",
                        UnaryOp::Invert => "invert",
                        UnaryOp::USub => "neg",
                        UnaryOp::UAdd => "pos",
                    };
                    make_unaryop(func_name, *operand)
                }
                Expr::Compare(ast::ExprCompare {
                    left,
                    ops,
                    comparators,
                    ..
                }) if ops.len() == 1 && comparators.len() == 1 => {
                    let mut ops_vec = ops.into_vec();
                    let mut comps_vec = comparators.into_vec();
                    let left = *left;
                    let right = comps_vec.pop().unwrap();
                    let op = ops_vec.pop().unwrap();
                    let call = match op {
                        CmpOp::Eq => make_binop("eq", left, right),
                        CmpOp::NotEq => make_binop("ne", left, right),
                        CmpOp::Lt => make_binop("lt", left, right),
                        CmpOp::LtE => make_binop("le", left, right),
                        CmpOp::Gt => make_binop("gt", left, right),
                        CmpOp::GtE => make_binop("ge", left, right),
                        CmpOp::Is => make_binop("is_", left, right),
                        CmpOp::IsNot => make_binop("is_not", left, right),
                        CmpOp::In => make_binop("contains", right, left),
                        CmpOp::NotIn => {
                            let contains = make_binop("contains", right, left);
                            make_unaryop("not_", contains)
                        }
                    };
                    call
                }
                Expr::Subscript(ast::ExprSubscript {
                    value, slice, ctx, ..
                }) if matches!(ctx, ast::ExprContext::Load) => {
                    let obj = *value;
                    let key = *slice;
                    make_binop("getitem", obj, key)
                }
                _ => original,
            };
        }
        walk_expr(self, expr);
    }

    fn visit_stmt(&self, stmt: &mut Stmt) {
        if matches!(stmt, Stmt::FunctionDef(_)) {
            if let Stmt::FunctionDef(ast::StmtFunctionDef {
                decorator_list,
                name,
                ..
            }) = stmt
            {
                if !decorator_list.is_empty() {
                    let decorators = std::mem::take(decorator_list);
                    let func_name = name.id.clone();
                    let func_def = stmt.clone();
                    *stmt = rewrite_decorator::rewrite(
                        decorators,
                        func_name.as_str(),
                        func_def,
                        None,
                        self.ctx,
                    );
                    self.visit_stmt(stmt);
                    return;
                }
            }
            self.push_scope();
            walk_stmt(self, stmt);
            let functions = self.pop_scope();
            if !functions.is_empty() {
                if let Stmt::FunctionDef(ast::StmtFunctionDef { body, .. }) = stmt {
                    body.splice(0..0, functions);
                }
            }
            return;
        }

        *stmt = match stmt {
            Stmt::With(with) => rewrite_with::rewrite(with.clone(), self.ctx),
            Stmt::Assert(assert) => rewrite_assert::rewrite(assert.clone()),
            Stmt::ClassDef(class_def) => {
                let mut base_stmt = rewrite_class_def::rewrite(class_def.clone());
                self.visit_stmt(&mut base_stmt);
                let class_name = class_def.name.id.clone();
                if !class_def.decorator_list.is_empty() {
                    let decorators = class_def.decorator_list.clone();
                    let base_name = format!("_dp_class_{}", class_name);
                    rewrite_decorator::rewrite(
                        decorators,
                        class_name.as_str(),
                        base_stmt,
                        Some(base_name.as_str()),
                        self.ctx,
                    )
                } else {
                    base_stmt
                }
            }
            Stmt::For(for_stmt) => rewrite_for_loop::rewrite(for_stmt.clone(), self.ctx),
            Stmt::Try(try_stmt) => rewrite_try_except::rewrite(try_stmt.clone(), self.ctx),
            Stmt::Match(match_stmt) => rewrite_match_case::rewrite(match_stmt.clone(), self.ctx),
            Stmt::Import(import) => rewrite_import::rewrite(import.clone()),
            Stmt::ImportFrom(import_from) => {
                match rewrite_import::rewrite_from(import_from.clone(), &self.options) {
                    Some(stmt) => stmt,
                    None => Stmt::ImportFrom(import_from.clone()),
                }
            }
            Stmt::AnnAssign(ann_assign) => {
                let target = (*ann_assign.target).clone();
                let value = ann_assign
                    .value
                    .clone()
                    .map(|v| *v)
                    .unwrap_or_else(|| py_expr!("None"));
                let mut stmts = Vec::new();
                self.rewrite_target(target, value, &mut stmts);
                single_stmt(stmts)
            }
            Stmt::Assign(assign) => {
                let value = (*assign.value).clone();
                let mut stmts = Vec::new();
                if assign.targets.len() > 1 {
                    let tmp_name = self.ctx.fresh("tmp");
                    let tmp_expr = py_expr!(
                        "
{name:id}
",
                        name = tmp_name.as_str(),
                    );
                    let tmp_stmt = py_stmt!(
                        "
{name:id} = {value:expr}
",
                        name = tmp_name.as_str(),
                        value = value,
                    );

                    stmts.push(tmp_stmt);
                    for target in &assign.targets {
                        self.rewrite_target(target.clone(), tmp_expr.clone(), &mut stmts);
                    }
                } else {
                    self.rewrite_target(assign.targets[0].clone(), value, &mut stmts);
                }

                single_stmt(stmts)
            }
            Stmt::AugAssign(aug) => {
                let target = (*aug.target).clone();
                let value = (*aug.value).clone();

                let func_name = match aug.op {
                    Operator::Add => "iadd",
                    Operator::Sub => "isub",
                    Operator::Mult => "imul",
                    Operator::MatMult => "imatmul",
                    Operator::Div => "itruediv",
                    Operator::Mod => "imod",
                    Operator::Pow => "ipow",
                    Operator::LShift => "ilshift",
                    Operator::RShift => "irshift",
                    Operator::BitOr => "ior",
                    Operator::BitXor => "ixor",
                    Operator::BitAnd => "iand",
                    Operator::FloorDiv => "ifloordiv",
                };

                let mut target_expr = target.clone();
                match &mut target_expr {
                    Expr::Name(name) => name.ctx = ast::ExprContext::Load,
                    Expr::Attribute(attr) => attr.ctx = ast::ExprContext::Load,
                    Expr::Subscript(sub) => sub.ctx = ast::ExprContext::Load,
                    _ => {}
                }
                let call = make_binop(func_name, target_expr, value);
                let mut stmts = Vec::new();
                self.rewrite_target(target, call, &mut stmts);
                single_stmt(stmts)
            }
            Stmt::Delete(del) => {
                let mut stmts = Vec::with_capacity(del.targets.len());
                for target in &del.targets {
                    let new_stmt = if let Expr::Subscript(sub) = target {
                        py_stmt!(
                            "__dp__.delitem({obj:expr}, {key:expr})",
                            obj = (*sub.value).clone(),
                            key = (*sub.slice).clone(),
                        )
                    } else if let Expr::Attribute(attr) = target {
                        py_stmt!(
                            "__dp__.delattr({obj:expr}, {name:literal})",
                            obj = (*attr.value).clone(),
                            name = attr.attr.as_str(),
                        )
                    } else {
                        py_stmt!("del {target:expr}", target = target.clone())
                    };

                    stmts.push(new_stmt);
                }
                single_stmt(stmts)
            }
            Stmt::Raise(ast::StmtRaise {
                exc: Some(exc),
                cause: Some(cause),
                ..
            }) => {
                py_stmt!(
                    "raise __dp__.raise_from({exc:expr}, {cause:expr})",
                    exc = *exc.clone(),
                    cause = *cause.clone(),
                )
            }
            _ => stmt.clone(),
        };

        walk_stmt(self, stmt);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transform::Options;
    use ruff_python_codegen::{Generator, Stylist};
    use ruff_python_parser::parse_module;

    fn rewrite_source(source: &str) -> String {
        let parsed = parse_module(source).expect("parse error");
        let tokens = parsed.tokens().clone();
        let mut module = parsed.into_syntax();

        let ctx = Context::new(Options::default());
        let expr_transformer = ExprRewriter::new(&ctx);
        expr_transformer.rewrite_body(&mut module.body);

        crate::template::flatten(&mut module.body);

        let stylist = Stylist::from_tokens(&tokens, source);
        let mut output = String::new();
        for stmt in &module.body {
            let snippet = Generator::from(&stylist).stmt(stmt);
            output.push_str(&snippet);
            output.push_str(stylist.line_ending().as_str());
        }
        output
    }

    #[test]
    fn rewrites_binary_ops() {
        let cases = [
            (r#"a + b"#, r#"getattr(__dp__, "add")(a, b)"#),
            (r#"a - b"#, r#"getattr(__dp__, "sub")(a, b)"#),
        ];

        for (input, expected) in cases {
            let output = rewrite_source(input);
            assert_eq!(output.trim_end(), expected);
        }
    }

    #[test]
    fn rewrites_aug_assign() {
        let input = "
x = 1
x += 2
";
        let expected = r#"
x = 1
x = getattr(__dp__, "iadd")(x, 2)
"#;
        let output = rewrite_source(input);
        assert_eq!(output.trim(), expected.trim());
    }

    #[test]
    fn rewrites_attribute_aug_assign() {
        let input = "
a.b += c
";
        let expected = r#"
getattr(__dp__, "setattr")(a, "b", getattr(__dp__, "iadd")(getattr(a, "b"), c))
"#;
        let output = rewrite_source(input);
        assert_eq!(output.trim(), expected.trim());
    }

    #[test]
    fn rewrites_ann_assign() {
        let input = "
x: int = 1
";
        let expected = "
x = 1
";
        let output = rewrite_source(input);
        assert_eq!(output.trim(), expected.trim());
    }

    #[test]
    fn rewrites_unary_ops() {
        let cases = [
            (
                r#"
-a
"#,
                r#"
getattr(__dp__, "neg")(a)
"#,
            ),
            (
                r#"
~b
"#,
                r#"
getattr(__dp__, "invert")(b)
"#,
            ),
            (
                r#"
not c
"#,
                r#"
getattr(__dp__, "not_")(c)
"#,
            ),
            (
                r#"
+a
"#,
                r#"
getattr(__dp__, "pos")(a)
"#,
            ),
        ];

        for (input, expected) in cases {
            let output = rewrite_source(input);
            assert_eq!(output.trim(), expected.trim());
        }
    }

    #[test]
    fn rewrites_bool_ops() {
        let cases = [
            (
                r#"
a or b
"#,
                r#"
def _dp_lambda_1():
    return b
getattr(__dp__, "or_expr")(a, _dp_lambda_1)
"#,
            ),
            (
                r#"
a and b
"#,
                r#"
def _dp_lambda_1():
    return b
getattr(__dp__, "and_expr")(a, _dp_lambda_1)
"#,
            ),
            (
                r#"
f() or a
"#,
                r#"
def _dp_lambda_1():
    return a
getattr(__dp__, "or_expr")(f(), _dp_lambda_1)
"#,
            ),
            (
                r#"
f() and a
"#,
                r#"
def _dp_lambda_1():
    return a
getattr(__dp__, "and_expr")(f(), _dp_lambda_1)
"#,
            ),
        ];

        for (input, expected) in cases {
            let output = rewrite_source(input);
            assert_eq!(output.trim(), expected.trim());
        }
    }

    #[test]
    fn rewrites_multi_bool_ops() {
        let output = rewrite_source(
            r#"
a or b or c
"#,
        );
        assert_eq!(
            output.trim(),
            r#"
def _dp_lambda_2():
    return c
def _dp_lambda_1():
    return getattr(__dp__, "or_expr")(b, _dp_lambda_2)
getattr(__dp__, "or_expr")(a, _dp_lambda_1)
"#
            .trim(),
        );

        let output = rewrite_source(
            r#"
a and b and c
"#,
        );
        assert_eq!(
            output.trim(),
            r#"
def _dp_lambda_2():
    return c
def _dp_lambda_1():
    return getattr(__dp__, "and_expr")(b, _dp_lambda_2)
getattr(__dp__, "and_expr")(a, _dp_lambda_1)
"#
            .trim(),
        );
    }

    #[test]
    fn rewrites_comparisons() {
        let cases = [
            (r#"a == b"#, r#"getattr(__dp__, "eq")(a, b)"#),
            (r#"a != b"#, r#"getattr(__dp__, "ne")(a, b)"#),
            (r#"a < b"#, r#"getattr(__dp__, "lt")(a, b)"#),
            (r#"a <= b"#, r#"getattr(__dp__, "le")(a, b)"#),
            (r#"a > b"#, r#"getattr(__dp__, "gt")(a, b)"#),
            (r#"a >= b"#, r#"getattr(__dp__, "ge")(a, b)"#),
            (r#"a is b"#, r#"getattr(__dp__, "is_")(a, b)"#),
            (r#"a is not b"#, r#"getattr(__dp__, "is_not")(a, b)"#),
            (r#"a in b"#, r#"getattr(__dp__, "contains")(b, a)"#),
            (
                r#"a not in b"#,
                r#"getattr(__dp__, "not_")(getattr(__dp__, "contains")(b, a))"#,
            ),
        ];

        for (input, expected) in cases {
            let output = rewrite_source(input);
            assert_eq!(output.trim_end(), expected);
        }
    }

    #[test]
    fn rewrites_if_expr() {
        let cases = [
            (
                r#"
a if b else c
"#,
                r#"
def _dp_lambda_1():
    return a
def _dp_lambda_2():
    return c
getattr(__dp__, "if_expr")(b, _dp_lambda_1, _dp_lambda_2)
"#,
            ),
            (
                r#"
(a + 1) if f() else (b + 2)
"#,
                r#"
def _dp_lambda_1():
    return getattr(__dp__, "add")(a, 1)
def _dp_lambda_2():
    return getattr(__dp__, "add")(b, 2)
getattr(__dp__, "if_expr")(f(), _dp_lambda_1, _dp_lambda_2)
"#,
            ),
        ];
        for (input, expected) in cases {
            let output = rewrite_source(input);
            assert_eq!(output.trim(), expected.trim());
        }
    }

    #[test]
    fn rewrites_getitem() {
        let output = rewrite_source("a[b]");
        assert_eq!(output.trim_end(), r#"getattr(__dp__, "getitem")(a, b)"#);
    }

    #[test]
    fn rewrites_delitem() {
        let output = rewrite_source("del a[b]");
        assert_eq!(output.trim_end(), r#"getattr(__dp__, "delitem")(a, b)"#);
    }

    #[test]
    fn rewrites_delattr() {
        let output = rewrite_source("del a.b");
        assert_eq!(output.trim_end(), r#"getattr(__dp__, "delattr")(a, "b")"#);
    }

    #[test]
    fn rewrites_nested_delitem() {
        let output = rewrite_source("del a.b[1]");
        assert_eq!(
            output.trim_end(),
            r#"getattr(__dp__, "delitem")(getattr(a, "b"), 1)"#
        );
    }

    #[test]
    fn rewrites_delattr_after_getitem() {
        let output = rewrite_source("del a.b[1].c");
        assert_eq!(
            output.trim_end(),
            r#"getattr(__dp__, "delattr")(getattr(__dp__, "getitem")(getattr(a, "b"), 1), "c")"#
        );
    }

    #[test]
    fn rewrites_multi_delitem_targets() {
        let output = rewrite_source("del a[0], b[0]");
        let expected = r#"getattr(__dp__, "delitem")(a, 0)
getattr(__dp__, "delitem")(b, 0)"#;
        assert_eq!(output.trim(), expected.trim());
    }

    #[test]
    fn rewrites_chain_assignment() {
        let output = rewrite_source(
            r#"
a = b = c
"#,
        );
        let expected = r#"
_dp_tmp_1 = c
a = _dp_tmp_1
b = _dp_tmp_1
"#;
        assert_eq!(output.trim(), expected.trim());
    }

    #[test]
    fn rewrites_raise_from() {
        let output = rewrite_source("raise ValueError from exc");
        assert_eq!(
            output.trim_end(),
            r#"raise getattr(__dp__, "raise_from")(ValueError, exc)"#,
        );
    }

    #[test]
    fn does_not_rewrite_plain_raise() {
        let output = rewrite_source("raise ValueError");
        assert_eq!(output.trim_end(), "raise ValueError");
    }

    #[test]
    fn rewrites_list_literal() {
        let input = r#"
a = [1, 2, 3]
"#;
        let expected = r#"
a = list((1, 2, 3))
"#;
        let output = rewrite_source(input);
        assert_eq!(output.trim(), expected.trim());
    }

    #[test]
    fn rewrites_set_literal() {
        let input = r#"
a = {1, 2, 3}
"#;
        let expected = r#"
a = set((1, 2, 3))
"#;
        let output = rewrite_source(input);
        assert_eq!(output.trim(), expected.trim());
    }

    #[test]
    fn rewrites_dict_literal() {
        let input = r#"
a = {'a': 1, 'b': 2}
"#;
        let expected = r#"
a = dict((('a', 1), ('b', 2)))
"#;
        let output = rewrite_source(input);
        assert_eq!(output.trim(), expected.trim());
    }

    #[test]
    fn rewrites_slices() {
        let cases = [
            (
                r#"a[1:2:3]"#,
                r#"getattr(__dp__, "getitem")(a, slice(1, 2, 3))"#,
            ),
            (
                r#"a[1:2]"#,
                r#"getattr(__dp__, "getitem")(a, slice(1, 2, None))"#,
            ),
            (
                r#"a[:2]"#,
                r#"getattr(__dp__, "getitem")(a, slice(None, 2, None))"#,
            ),
            (
                r#"a[::2]"#,
                r#"getattr(__dp__, "getitem")(a, slice(None, None, 2))"#,
            ),
            (
                r#"a[:]"#,
                r#"getattr(__dp__, "getitem")(a, slice(None, None, None))"#,
            ),
        ];

        for (input, expected) in cases {
            let output = rewrite_source(input);
            assert_eq!(output.trim_end(), expected);
        }
    }

    #[test]
    fn rewrites_complex_literals() {
        let cases = [
            (r#"a = 1j"#, r#"a = complex(0.0, 1.0)"#),
            (
                r#"a = 1 + 2j"#,
                r#"a = getattr(__dp__, "add")(1, complex(0.0, 2.0))"#,
            ),
        ];

        for (input, expected) in cases {
            let output = rewrite_source(input);
            assert_eq!(output.trim_end(), expected);
        }
    }

    #[test]
    fn rewrites_ellipsis() {
        let cases = [("a = ...", "a = Ellipsis"), ("...", "Ellipsis")];

        for (input, expected) in cases {
            let output = rewrite_source(input);
            assert_eq!(output.trim_end(), expected);
        }
    }

    #[test]
    fn rewrites_attribute_access() {
        let cases = [
            ("obj.attr", "getattr(obj, \"attr\")"),
            ("foo.bar.baz", "getattr(getattr(foo, \"bar\"), \"baz\")"),
        ];

        for (input, expected) in cases {
            let output = rewrite_source(input);
            assert_eq!(output.trim_end(), expected);
        }
    }

    #[test]
    fn desugars_tuple_unpacking() {
        let output = rewrite_source(
            r#"
a, b = c
"#,
        );
        let expected = r#"
_dp_tmp_1 = c
a = getattr(__dp__, "getitem")(_dp_tmp_1, 0)
b = getattr(__dp__, "getitem")(_dp_tmp_1, 1)
"#;
        assert_eq!(output.trim(), expected.trim());
    }

    #[test]
    fn desugars_list_unpacking() {
        let output = rewrite_source(
            r#"
[a, b] = c
"#,
        );
        let expected = r#"
_dp_tmp_1 = c
a = getattr(__dp__, "getitem")(_dp_tmp_1, 0)
b = getattr(__dp__, "getitem")(_dp_tmp_1, 1)
"#;
        assert_eq!(output.trim(), expected.trim());
    }

    #[test]
    fn rewrites_attribute_assignment() {
        let output = rewrite_source(
            r#"
a.b = c
"#,
        );
        let expected = r#"
getattr(__dp__, "setattr")(a, "b", c)
"#;
        assert_eq!(output.trim(), expected.trim());
    }

    #[test]
    fn rewrites_subscript_assignment() {
        let output = rewrite_source(
            r#"
a[b] = c
"#,
        );
        let expected = r#"
getattr(__dp__, "setitem")(a, b, c)
"#;
        assert_eq!(output.trim(), expected.trim());
    }

    #[test]
    fn rewrites_chain_assignment_with_subscript() {
        let output = rewrite_source(
            r#"
a[0] = b = 1
"#,
        );
        let expected = r#"
_dp_tmp_1 = 1
getattr(__dp__, "setitem")(a, 0, _dp_tmp_1)
b = _dp_tmp_1
"#;
        assert_eq!(output.trim(), expected.trim());
    }

    #[test]
    fn rewrites_list_comp() {
        let input = "
r = [a + 1 for a in items if a % 2 == 0]
";
        let output = rewrite_source(input);
        assert!(output.contains("getattr(__dp__, \"iter\")(items)"));
        assert!(output.contains("yield getattr(__dp__, \"add\")(a, 1)"));
    }

    #[test]
    fn rewrites_set_comp() {
        let input = "
r = {a for a in items}
";
        let output = rewrite_source(input);
        assert!(output.contains("getattr(__dp__, \"iter\")(items)"));
        assert!(output.contains("yield a"));
    }

    #[test]
    fn rewrites_dict_comp() {
        let input = "
r = {k: v + 1 for k, v in items if k % 2 == 0}
";
        let output = rewrite_source(input);
        assert!(output.contains("getattr(__dp__, \"iter\")(items)"));
        assert!(output.contains("yield k, getattr(__dp__, \"add\")(v, 1)"));
    }

    #[test]
    fn rewrites_multi_generator_list_comp() {
        let input = "
r = [a * b for a in items for b in items2]
";
        let output = rewrite_source(input);
        assert!(output.contains("getattr(__dp__, \"iter\")(items)"));
        assert!(output.contains("getattr(__dp__, \"mul\")(a, b)"));
    }
}

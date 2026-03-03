use super::*;

#[derive(Default)]
struct AwaitToYieldFromPass {
    rewritten_count: usize,
    temp_counter: usize,
}

impl AwaitToYieldFromPass {
    fn fresh_tmp(&mut self) -> String {
        let name = format!("_dp_await_tmp_{}", self.temp_counter);
        self.temp_counter += 1;
        name
    }

    fn hoist_awaits_in_expr(&mut self, expr: &mut Expr, prefix: &mut Vec<Stmt>) {
        match expr {
            Expr::Await(await_expr) => {
                self.hoist_awaits_in_expr(await_expr.value.as_mut(), prefix);
                let tmp = self.fresh_tmp();
                prefix.push(py_stmt!(
                    "{tmp:id} = yield from __dp_await_iter({value:expr})",
                    tmp = tmp.as_str(),
                    value = *await_expr.value.clone(),
                ));
                *expr = py_expr!("{tmp:id}", tmp = tmp.as_str());
                self.rewritten_count += 1;
            }
            Expr::Call(call_expr) => {
                self.hoist_awaits_in_expr(call_expr.func.as_mut(), prefix);
                for arg in &mut call_expr.arguments.args {
                    self.hoist_awaits_in_expr(arg, prefix);
                }
                for keyword in &mut call_expr.arguments.keywords {
                    self.hoist_awaits_in_expr(&mut keyword.value, prefix);
                }
            }
            Expr::Attribute(attribute_expr) => {
                self.hoist_awaits_in_expr(attribute_expr.value.as_mut(), prefix);
            }
            Expr::Subscript(subscript_expr) => {
                self.hoist_awaits_in_expr(subscript_expr.value.as_mut(), prefix);
                self.hoist_awaits_in_expr(subscript_expr.slice.as_mut(), prefix);
            }
            Expr::UnaryOp(unary_expr) => {
                self.hoist_awaits_in_expr(unary_expr.operand.as_mut(), prefix);
            }
            Expr::BinOp(binop_expr) => {
                self.hoist_awaits_in_expr(binop_expr.left.as_mut(), prefix);
                self.hoist_awaits_in_expr(binop_expr.right.as_mut(), prefix);
            }
            Expr::List(list_expr) => {
                for item in &mut list_expr.elts {
                    self.hoist_awaits_in_expr(item, prefix);
                }
            }
            Expr::Tuple(tuple_expr) => {
                for item in &mut tuple_expr.elts {
                    self.hoist_awaits_in_expr(item, prefix);
                }
            }
            Expr::Set(set_expr) => {
                for item in &mut set_expr.elts {
                    self.hoist_awaits_in_expr(item, prefix);
                }
            }
            Expr::Dict(dict_expr) => {
                for item in &mut dict_expr.items {
                    if let Some(key_expr) = &mut item.key {
                        self.hoist_awaits_in_expr(key_expr, prefix);
                    }
                    self.hoist_awaits_in_expr(&mut item.value, prefix);
                }
            }
            Expr::Starred(starred_expr) => {
                self.hoist_awaits_in_expr(starred_expr.value.as_mut(), prefix);
            }
            _ => {}
        }
    }
}

impl Transformer for AwaitToYieldFromPass {
    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        if matches!(stmt, Stmt::FunctionDef(_) | Stmt::ClassDef(_)) {
            return;
        }
        let mut prefix = Vec::new();
        match stmt {
            Stmt::Expr(expr_stmt) => {
                if let Expr::Await(await_expr) = expr_stmt.value.as_ref() {
                    expr_stmt.value = Box::new(py_expr!(
                        "yield from __dp_await_iter({value:expr})",
                        value = *await_expr.value.clone(),
                    ));
                    self.rewritten_count += 1;
                } else {
                    self.hoist_awaits_in_expr(expr_stmt.value.as_mut(), &mut prefix);
                }
            }
            Stmt::Assign(assign_stmt) => {
                if let Expr::Await(await_expr) = assign_stmt.value.as_ref() {
                    assign_stmt.value = Box::new(py_expr!(
                        "yield from __dp_await_iter({value:expr})",
                        value = *await_expr.value.clone(),
                    ));
                    self.rewritten_count += 1;
                } else {
                    self.hoist_awaits_in_expr(assign_stmt.value.as_mut(), &mut prefix);
                }
            }
            Stmt::Return(return_stmt) => {
                if let Some(value) = return_stmt.value.as_mut() {
                    if let Expr::Await(await_expr) = value.as_ref() {
                        *value = Box::new(py_expr!(
                            "yield from __dp_await_iter({value:expr})",
                            value = *await_expr.value.clone(),
                        ));
                        self.rewritten_count += 1;
                    } else {
                        self.hoist_awaits_in_expr(value.as_mut(), &mut prefix);
                    }
                }
            }
            _ => {}
        }
        if !prefix.is_empty() {
            prefix.push(stmt.clone());
            *stmt = into_body(prefix);
        }
        walk_stmt(self, stmt);
    }
}

pub(super) fn lower_coroutine_awaits_to_yield_from(stmts: &mut [Box<Stmt>]) -> bool {
    let mut pass = AwaitToYieldFromPass::default();
    for stmt in stmts {
        pass.visit_stmt(stmt.as_mut());
    }
    pass.rewritten_count > 0
}

pub(super) fn coroutine_generator_marker_stmt() -> Box<Stmt> {
    Box::new(py_stmt!("if False:\n    yield __dp_NONE"))
}

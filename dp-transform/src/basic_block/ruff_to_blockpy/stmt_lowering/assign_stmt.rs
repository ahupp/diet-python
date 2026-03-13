use super::*;

impl StmtLowerer for ast::StmtAssign {
    fn simplify_ast(self) -> Stmt {
        Stmt::Assign(self)
    }

    fn to_blockpy<E>(
        &self,
        out: &mut BlockPyStmtFragmentBuilder<E>,
        _loop_ctx: Option<&LoopContext>,
        _next_label_id: &mut usize,
    ) -> Result<(), String>
    where
        E: From<Expr> + std::fmt::Debug,
    {
        if self.targets.len() != 1 {
            return Err(assign_delete_error(
                "multi-target assignment reached BlockPy conversion",
                &Stmt::Assign(self.clone()),
            ));
        }
        let Some(target) = self.targets[0].as_name_expr().cloned() else {
            return Err(assign_delete_error(
                "non-name assignment target reached BlockPy conversion",
                &Stmt::Assign(self.clone()),
            ));
        };
        out.push_stmt(BlockPyStmt::Assign(BlockPyAssign {
            target,
            value: (*self.value).clone().into(),
        }));
        Ok(())
    }
}

pub(super) fn with_target_object_expr(value: Expr) -> Expr {
    if let Expr::Name(name) = &value {
        py_expr!(
            "__dp_load_deleted_name({name:literal}, {value:expr})",
            name = name.id.as_str(),
            value = value,
        )
    } else {
        value
    }
}

pub(super) fn rewrite_assignment_target<F>(
    target: Expr,
    rhs: Expr,
    out: &mut Vec<Stmt>,
    next_temp: &mut F,
) where
    F: FnMut(&str) -> String,
{
    match target {
        Expr::Tuple(tuple) => rewrite_unpack_target(tuple.elts, rhs, out, next_temp),
        Expr::List(list) => rewrite_unpack_target(list.elts, rhs, out, next_temp),
        Expr::Subscript(ast::ExprSubscript { value, slice, .. }) => {
            out.push(py_stmt!(
                "__dp_setitem({obj:expr}, {key:expr}, {rhs:expr})",
                obj = with_target_object_expr(*value),
                key = *slice,
                rhs = rhs,
            ));
        }
        Expr::Attribute(ast::ExprAttribute { value, attr, .. }) => {
            out.push(py_stmt!(
                "__dp_setattr({obj:expr}, {name:literal}, {rhs:expr})",
                obj = with_target_object_expr(*value),
                name = attr.as_str(),
                rhs = rhs,
            ));
        }
        Expr::Name(ast::ExprName { id, .. }) => {
            out.push(py_stmt!(
                "{name:id} = {rhs:expr}",
                name = id.as_str(),
                rhs = rhs
            ));
        }
        other => {
            panic!("unsupported assignment target in Ruff AST -> BlockPy lowering: {other:?}");
        }
    }
}

fn rewrite_unpack_target<F>(elts: Vec<Expr>, value: Expr, out: &mut Vec<Stmt>, next_temp: &mut F)
where
    F: FnMut(&str) -> String,
{
    let unpacked_name = next_temp("tmp");
    let unpacked_tmp = py_expr!("{tmp:id}", tmp = unpacked_name.as_str());

    let mut spec_elts = Vec::new();
    let mut starred_seen = false;
    for elt in &elts {
        match elt {
            Expr::Starred(_) => {
                if starred_seen {
                    panic!("unsupported starred with-target assignment");
                }
                starred_seen = true;
                spec_elts.push(py_expr!("False"));
            }
            _ => spec_elts.push(py_expr!("True")),
        }
    }

    out.push(py_stmt!(
        "{tmp:id} = __dp_unpack({value:expr}, {spec:expr})",
        tmp = unpacked_name.as_str(),
        value = value,
        spec = make_tuple(spec_elts),
    ));

    let starred_index = elts.iter().position(|elt| matches!(elt, Expr::Starred(_)));
    for (idx, elt) in elts.into_iter().enumerate() {
        match elt {
            Expr::Starred(ast::ExprStarred { value, .. }) if Some(idx) == starred_index => {
                rewrite_assignment_target(
                    *value,
                    py_expr!(
                        "__dp_list(__dp_getitem({tmp:expr}, {idx:literal}))",
                        tmp = unpacked_tmp.clone(),
                        idx = idx as i64,
                    ),
                    out,
                    next_temp,
                );
            }
            other => {
                rewrite_assignment_target(
                    other,
                    py_expr!(
                        "__dp_getitem({tmp:expr}, {idx:literal})",
                        tmp = unpacked_tmp.clone(),
                        idx = idx as i64,
                    ),
                    out,
                    next_temp,
                );
            }
        }
    }

    out.push(py_stmt!("del {tmp:id}", tmp = unpacked_name.as_str()));
}

pub(crate) fn build_for_target_assign_body<F>(
    target: &Expr,
    tmp_expr: Expr,
    tmp_name: &str,
    cell_slots: &std::collections::HashSet<String>,
    next_temp: &mut F,
) -> Vec<Stmt>
where
    F: FnMut(&str) -> String,
{
    let mut out = Vec::new();
    rewrite_assignment_target(target.clone(), tmp_expr, &mut out, next_temp);
    out.extend(sync_target_cells_stmts_shared(target, cell_slots));
    out.push(py_stmt!("{tmp:id} = None", tmp = tmp_name));
    out
}

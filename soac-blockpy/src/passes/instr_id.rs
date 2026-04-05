use crate::block_py::{
    walk_expr_mut, BlockPyFunction, BlockPyModule, ChildVisitable, CodegenBlockPyExpr, HasMeta,
    InstrId, VisitMut, WithMeta,
};
use crate::passes::CodegenBlockPyPass;

struct InstrIdAssigner {
    next_instr_id: InstrId,
}

impl InstrIdAssigner {
    fn assign<I>(&mut self, expr: &mut I)
    where
        I: crate::block_py::Instr + ChildVisitable<I> + HasMeta + WithMeta + Clone,
    {
        let mut meta = expr.meta();
        meta.instr_id = Some(self.next_instr_id);
        self.next_instr_id = InstrId::new(
            self.next_instr_id
                .get()
                .checked_add(1)
                .expect("per-function instruction count should fit in u32"),
        )
        .expect("instruction ids should stay non-zero");
        *expr = expr.clone().with_meta(meta);
    }
}

impl crate::block_py::VisitMut<CodegenBlockPyExpr> for InstrIdAssigner {
    fn visit_instr_mut(&mut self, expr: &mut CodegenBlockPyExpr)
    where
        CodegenBlockPyExpr: ChildVisitable<CodegenBlockPyExpr>,
    {
        self.assign(expr);
        walk_expr_mut(self, expr);
    }
}

pub fn assign_function_instr_ids(function: &mut BlockPyFunction<CodegenBlockPyPass>) {
    let mut assigner = InstrIdAssigner {
        next_instr_id: InstrId::new(1).expect("1 should be a valid non-zero instruction id"),
    };
    assigner.visit_fn_mut(function);
}

pub fn assign_module_instr_ids(module: &mut BlockPyModule<CodegenBlockPyPass>) {
    for function in &mut module.callable_defs {
        assign_function_instr_ids(function);
    }
}

#[cfg(test)]
mod test {
    use super::assign_module_instr_ids;
    use crate::block_py::{
        walk_fn, ChildVisitable, CodegenBlockPyExpr, HasMeta, Visit,
    };
    use crate::passes::CodegenBlockPyPass;
    use crate::lower_python_to_blockpy_for_testing;

    struct InstrIdCollector {
        ids: Vec<u32>,
    }

    impl Visit<CodegenBlockPyExpr> for InstrIdCollector {
        fn visit_instr(&mut self, expr: &CodegenBlockPyExpr)
        where
            CodegenBlockPyExpr: ChildVisitable<CodegenBlockPyExpr>,
        {
            self.ids
                .push(expr.meta().instr_id.expect("instr ids should be assigned").get());
            crate::block_py::walk_expr(self, expr);
        }
    }

    #[test]
    fn assigns_sequential_instr_ids_per_function() {
        let mut lowered = lower_python_to_blockpy_for_testing(
            r#"
def f(x):
    y = g(x + 1)
    return h(y)

def g(v):
    return v
"#,
        )
        .expect("transform should succeed")
        .codegen_module;

        assign_module_instr_ids(&mut lowered);

        let f = lowered
            .callable_defs
            .iter()
            .find(|function| function.names.qualname == "f")
            .expect("missing lowered function f");
        let g = lowered
            .callable_defs
            .iter()
            .find(|function| function.names.qualname == "g")
            .expect("missing lowered function g");

        let mut f_ids = InstrIdCollector { ids: Vec::new() };
        walk_fn(&mut f_ids, f);
        assert_eq!(
            f_ids.ids,
            (1..=u32::try_from(f_ids.ids.len()).unwrap()).collect::<Vec<_>>()
        );

        let mut g_ids = InstrIdCollector { ids: Vec::new() };
        walk_fn(&mut g_ids, g);
        assert_eq!(
            g_ids.ids,
            (1..=u32::try_from(g_ids.ids.len()).unwrap()).collect::<Vec<_>>()
        );
    }
}

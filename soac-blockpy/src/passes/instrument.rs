use crate::block_py::{
    Block, BlockLabel, BlockTerm, CounterDef, CounterId, CounterScope, CounterSite, Instr,
};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CounterHandle {
    id: CounterId,
}

impl CounterHandle {
    pub const fn id(self) -> CounterId {
        self.id
    }
}

pub struct CounterBuilder<'a> {
    defs: &'a mut Vec<CounterDef>,
    next_id: usize,
}

impl<'a> CounterBuilder<'a> {
    pub fn new(defs: &'a mut Vec<CounterDef>) -> Self {
        let next_id = defs
            .iter()
            .map(|def| def.id.0)
            .max()
            .map(|id| id + 1)
            .unwrap_or(0);
        Self { defs, next_id }
    }

    pub fn define(
        &mut self,
        scope: CounterScope,
        kind: impl Into<String>,
        site: CounterSite,
    ) -> CounterHandle {
        let handle = CounterHandle {
            id: CounterId(self.next_id),
        };
        self.next_id += 1;
        self.defs.push(CounterDef {
            id: handle.id,
            scope,
            kind: kind.into(),
            site,
        });
        handle
    }

    pub fn define_if_missing(
        &mut self,
        scope: CounterScope,
        kind: impl Into<String>,
        site: CounterSite,
    ) -> CounterHandle {
        let kind = kind.into();
        if let Some(existing) = self
            .defs
            .iter()
            .find(|counter| counter.scope == scope && counter.kind == kind && counter.site == site)
        {
            return CounterHandle { id: existing.id };
        }
        self.define(scope, kind, site)
    }
}

#[derive(Debug, Clone)]
pub enum OptInstr<I: Instr> {
    Instr(I),
    Block(OptBlock<I>),
}

#[derive(Debug, Clone)]
pub struct OptBlock<I: Instr> {
    entry: Block<I, I>,
    dependencies: Vec<Block<I, I>>,
}

impl<I: Instr> OptBlock<I> {
    pub fn new(entry: Block<I, I>, dependencies: Vec<Block<I, I>>) -> Result<Self, String> {
        validate_opt_block(&entry, &dependencies)?;
        Ok(Self {
            entry,
            dependencies,
        })
    }

    pub fn entry(&self) -> &Block<I, I> {
        &self.entry
    }

    pub fn entry_mut(&mut self) -> &mut Block<I, I> {
        &mut self.entry
    }

    pub fn dependencies(&self) -> &[Block<I, I>] {
        &self.dependencies
    }

    pub fn dependencies_mut(&mut self) -> &mut [Block<I, I>] {
        &mut self.dependencies
    }

    pub fn into_parts(self) -> (Block<I, I>, Vec<Block<I, I>>) {
        (self.entry, self.dependencies)
    }

    pub fn replace_fallthrough_target(&mut self, target: BlockLabel) -> bool {
        let mut replaced = self.entry.replace_fallthrough_target(target);
        for block in &mut self.dependencies {
            replaced |= block.replace_fallthrough_target(target);
        }
        replaced
    }
}

pub trait InstrumentInstr<I: Instr> {
    type Counter;

    fn instrument_instr(&self, instr: &I) -> Option<Self::Counter>;

    fn optimize_instr(&self, counter: &Self::Counter, instr: &I) -> OptInstr<I>;
}

fn validate_opt_block<I: Instr>(
    entry: &Block<I, I>,
    dependencies: &[Block<I, I>],
) -> Result<(), String> {
    let mut blocks_by_label = HashMap::new();
    blocks_by_label.insert(entry.label, entry);
    for block in dependencies {
        if blocks_by_label.insert(block.label, block).is_some() {
            return Err(format!(
                "optimization fragment reuses block label {}",
                block.label
            ));
        }
    }

    let mut reachable = HashSet::new();
    let mut memo = HashMap::new();
    if !all_paths_end_in_fallthrough(
        entry.label,
        &blocks_by_label,
        &mut reachable,
        &mut Vec::new(),
        &mut memo,
    ) {
        return Err(format!(
            "optimization fragment rooted at {} must keep every reachable path inside the fragment until {}",
            entry.label,
            BlockLabel::fallthrough()
        ));
    }

    for block in dependencies {
        if !reachable.contains(&block.label) {
            return Err(format!(
                "optimization fragment dependency {} is unreachable from {}",
                block.label, entry.label
            ));
        }
    }

    Ok(())
}

fn all_paths_end_in_fallthrough<I: Instr>(
    label: BlockLabel,
    blocks_by_label: &HashMap<BlockLabel, &Block<I, I>>,
    reachable: &mut HashSet<BlockLabel>,
    stack: &mut Vec<BlockLabel>,
    memo: &mut HashMap<BlockLabel, bool>,
) -> bool {
    if let Some(valid) = memo.get(&label) {
        reachable.insert(label);
        return *valid;
    }

    if stack.contains(&label) {
        return false;
    }

    let Some(block) = blocks_by_label.get(&label) else {
        return false;
    };

    reachable.insert(label);
    stack.push(label);
    let valid = match &block.term {
        BlockTerm::Jump(edge) => {
            edge.target.is_fallthrough()
                || all_paths_end_in_fallthrough(
                    edge.target,
                    blocks_by_label,
                    reachable,
                    stack,
                    memo,
                )
        }
        BlockTerm::IfTerm(if_term) => {
            [if_term.then_label, if_term.else_label]
                .into_iter()
                .all(|target| {
                    target.is_fallthrough()
                        || all_paths_end_in_fallthrough(
                            target,
                            blocks_by_label,
                            reachable,
                            stack,
                            memo,
                        )
                })
        }
        BlockTerm::BranchTable(branch) => branch
            .targets
            .iter()
            .copied()
            .chain(std::iter::once(branch.default_label))
            .all(|target| {
                target.is_fallthrough()
                    || all_paths_end_in_fallthrough(target, blocks_by_label, reachable, stack, memo)
            }),
        BlockTerm::Raise(_) | BlockTerm::Return(_) => false,
    };
    stack.pop();
    memo.insert(label, valid);
    valid
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block_py::{BlockEdge, FunctionId, InstrId};
    use crate::py_expr;

    #[test]
    fn counter_builder_allocates_sequential_ids() {
        let mut defs = Vec::new();
        let mut builder = CounterBuilder::new(&mut defs);

        let first = builder.define(
            CounterScope::This,
            "call_hot_targets",
            CounterSite::Runtime {
                function_id: Some(FunctionId::new(1, 2)),
                instr_id: Some(InstrId::new(BlockLabel::from_index(3), 4)),
            },
        );
        let second = builder.define(
            CounterScope::Global,
            "global_load_hit",
            CounterSite::Runtime {
                function_id: None,
                instr_id: None,
            },
        );

        assert_eq!(first.id(), CounterId(0));
        assert_eq!(second.id(), CounterId(1));
    }

    #[test]
    fn counter_builder_reuses_existing_definition() {
        let site = CounterSite::Runtime {
            function_id: Some(FunctionId::new(1, 2)),
            instr_id: Some(InstrId::new(BlockLabel::from_index(0), 7)),
        };
        let mut defs = vec![CounterDef {
            id: CounterId(9),
            scope: CounterScope::Function,
            kind: "runtime_incref".to_string(),
            site: site.clone(),
        }];

        let handle = CounterBuilder::new(&mut defs).define_if_missing(
            CounterScope::Function,
            "runtime_incref",
            site,
        );

        assert_eq!(handle.id(), CounterId(9));
        assert_eq!(defs.len(), 1);
    }

    #[test]
    fn opt_block_accepts_fallthrough_fragment() {
        let entry = Block::new(
            BlockLabel::from_index(0),
            vec![py_expr!("x")],
            BlockTerm::Jump(BlockEdge::new(BlockLabel::fallthrough())),
            Vec::new(),
            None,
        );

        let fragment = OptBlock::new(entry, Vec::new()).expect("fragment should validate");

        assert_eq!(fragment.entry().label, BlockLabel::from_index(0));
    }

    #[test]
    fn opt_block_rejects_non_fallthrough_cycle() {
        let entry = Block::new(
            BlockLabel::from_index(0),
            Vec::<ruff_python_ast::Expr>::new(),
            BlockTerm::Jump(BlockEdge::new(BlockLabel::from_index(1))),
            Vec::new(),
            None,
        );
        let dep = Block::new(
            BlockLabel::from_index(1),
            Vec::<ruff_python_ast::Expr>::new(),
            BlockTerm::Jump(BlockEdge::new(BlockLabel::from_index(0))),
            Vec::new(),
            None,
        );

        let err = OptBlock::new(entry, vec![dep]).expect_err("cycle should be rejected");

        assert!(err.contains("fallthrough"), "{err}");
    }

    #[test]
    fn opt_block_rejects_return_exit() {
        let entry = Block::new(
            BlockLabel::from_index(0),
            Vec::<ruff_python_ast::Expr>::new(),
            BlockTerm::Return(py_expr!("None")),
            Vec::new(),
            None,
        );

        let err = OptBlock::new(entry, Vec::new()).expect_err("return should be rejected");

        assert!(err.contains("fallthrough"), "{err}");
    }
}

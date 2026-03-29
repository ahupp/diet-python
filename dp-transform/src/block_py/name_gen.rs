use crate::block_py::{BlockPyLabel, FunctionId};
use ruff_python_ast as ast;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

#[derive(Debug)]
pub struct FunctionNameGen {
    state: Arc<FunctionNameGenState>,
}

#[derive(Debug)]
struct FunctionNameGenState {
    function_id: FunctionId,
    next_block_id: AtomicUsize,
    next_tmp_id: AtomicUsize,
}

impl FunctionNameGen {
    fn new(function_id: FunctionId) -> Self {
        Self {
            state: Arc::new(FunctionNameGenState {
                function_id,
                next_block_id: AtomicUsize::new(0),
                next_tmp_id: AtomicUsize::new(0),
            }),
        }
    }

    pub(crate) fn share(&self) -> Self {
        Self {
            state: Arc::clone(&self.state),
        }
    }

    pub fn function_id(&self) -> FunctionId {
        self.state.function_id
    }

    pub fn next_block_name(&self) -> BlockPyLabel {
        let current = self.state.next_block_id.fetch_add(1, Ordering::Relaxed);
        BlockPyLabel::from_index(current)
    }

    pub fn next_tmp_name(&self, prefix: &str) -> ast::name::Name {
        let current = self.state.next_tmp_id.fetch_add(1, Ordering::Relaxed);
        ast::name::Name::new(format!(
            "_dp_{prefix}_{}_{}",
            self.state.function_id.0, current
        ))
    }
}

#[derive(Debug)]
pub struct ModuleNameGen {
    next_function_id: usize,
}

impl ModuleNameGen {
    pub fn new(next_function_id: usize) -> Self {
        Self { next_function_id }
    }

    pub fn next_function_name_gen(&mut self) -> FunctionNameGen {
        let function_id = FunctionId(self.next_function_id);
        self.next_function_id += 1;
        FunctionNameGen::new(function_id)
    }
}

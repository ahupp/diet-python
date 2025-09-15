use std::cell::Cell;

use super::Options;

pub struct Namer {
    counter: Cell<usize>,
}

impl Namer {
    pub fn new() -> Self {
        Self {
            counter: Cell::new(0),
        }
    }

    pub fn fresh(&self, name: &str) -> String {
        let id = self.counter.get() + 1;
        self.counter.set(id);
        format!("_dp_{name}_{id}")
    }
}

pub struct Context {
    pub namer: Namer,
    pub options: Options,
}

impl Context {
    pub fn new(options: Options) -> Self {
        Self {
            namer: Namer::new(),
            options,
        }
    }

    pub fn fresh(&self, name: &str) -> String {
        self.namer.fresh(name)
    }
}

use std::cell::{Cell, RefCell};

use std::collections::HashSet;

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

#[derive(Clone)]
struct ClassContext {
    name: String,
    qualname: String,
}

pub struct Context {
    pub namer: Namer,
    pub options: Options,
    function_stack: RefCell<Vec<String>>,
    class_stack: RefCell<Vec<ClassContext>>,
    function_globals: RefCell<Vec<HashSet<String>>>,
}

impl Context {
    pub fn new(options: Options) -> Self {
        Self {
            namer: Namer::new(),
            options,
            function_stack: RefCell::new(Vec::new()),
            class_stack: RefCell::new(Vec::new()),
            function_globals: RefCell::new(Vec::new()),
        }
    }

    pub fn fresh(&self, name: &str) -> String {
        self.namer.fresh(name)
    }

    pub fn push_function(&self, qualname: String) {
        self.function_stack.borrow_mut().push(qualname);
    }

    pub fn current_function_qualname(&self) -> Option<String> {
        self.function_stack.borrow().last().cloned()
    }

    pub fn pop_function(&self) {
        self.function_stack.borrow_mut().pop();
    }

    pub fn push_function_globals(&self, globals: HashSet<String>) {
        self.function_globals.borrow_mut().push(globals);
    }

    pub fn pop_function_globals(&self) {
        self.function_globals.borrow_mut().pop();
    }

    pub fn function_globals_contains(&self, name: &str) -> bool {
        self.function_globals
            .borrow()
            .last()
            .map_or(false, |globals| globals.contains(name))
    }

    pub fn push_class(&self, class_name: String, class_qualname: String) {
        self.class_stack.borrow_mut().push(ClassContext {
            name: class_name,
            qualname: class_qualname,
        });
    }

    pub fn current_class_name(&self) -> Option<String> {
        self.class_stack.borrow().last().map(|ctx| ctx.name.clone())
    }

    pub fn current_class_qualname(&self) -> Option<String> {
        self.class_stack
            .borrow()
            .last()
            .map(|ctx| ctx.qualname.clone())
    }

    pub fn pop_class(&self) {
        self.class_stack.borrow_mut().pop();
    }
}

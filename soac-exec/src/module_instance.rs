use crate::module_symbols;
use crate::scope::Scope;
use diet_python::min_ast;

pub struct ModuleInstance {
    pub globals: Scope,
    pub ast: min_ast::Module,
}

unsafe impl Send for ModuleInstance {}
unsafe impl Sync for ModuleInstance {}

impl ModuleInstance {
    pub fn new(ast: min_ast::Module) -> Self {
        let symbols = module_symbols::module_symbols(&ast);
        let globals = Scope::new(
            &symbols,
            &["__dp__", "getattr", "__name__", "__spec__", "__loader__"],
        );
        ModuleInstance { globals, ast }
    }
}

use std::collections::HashMap;

use diet_python::min_ast::{self, StmtNode};

#[derive(Debug, Default, PartialEq, Eq)]
pub struct ModuleSymbols {
    pub globals: HashMap<String, SymbolMetadata>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SymbolMetadata {
    pub index: usize,
    pub written_after_init: bool,
}

pub fn module_symbols(module: &min_ast::Module) -> ModuleSymbols {
    let mut symbols = ModuleSymbols::default();
    collect_module_symbols(&module.body, &mut symbols);
    let mut names: Vec<_> = symbols.globals.keys().cloned().collect();
    names.sort();
    for (idx, name) in names.iter().enumerate() {
        if let Some(meta) = symbols.globals.get_mut(name) {
            meta.index = idx;
        }
    }
    symbols
}

fn collect_module_symbols(stmts: &[StmtNode], symbols: &mut ModuleSymbols) {
    for stmt in stmts {
        match stmt {
            StmtNode::FunctionDef(func) => {
                symbols.globals.entry(func.name.clone()).or_default();
                for name in &func.scope_vars.globals {
                    symbols
                        .globals
                        .entry(name.clone())
                        .or_default()
                        .written_after_init = true;
                }
                collect_globals_in_function(&func.body, symbols);
            }
            StmtNode::Assign { target, .. } | StmtNode::Delete { target } => {
                symbols.globals.entry(target.clone()).or_default();
            }
            StmtNode::If { body, orelse, .. } | StmtNode::While { body, orelse, .. } => {
                collect_module_symbols(body, symbols);
                collect_module_symbols(orelse, symbols);
            }
            StmtNode::Try {
                body,
                handler,
                orelse,
                finalbody,
            } => {
                collect_module_symbols(body, symbols);
                if let Some(h) = handler {
                    collect_module_symbols(h, symbols);
                }
                collect_module_symbols(orelse, symbols);
                collect_module_symbols(finalbody, symbols);
            }
            StmtNode::Break
            | StmtNode::Continue
            | StmtNode::Return { .. }
            | StmtNode::Expr(_)
            | StmtNode::Pass => {}
        }
    }
}

fn collect_globals_in_function(stmts: &[StmtNode], symbols: &mut ModuleSymbols) {
    for stmt in stmts {
        match stmt {
            StmtNode::FunctionDef(func) => {
                for name in &func.scope_vars.globals {
                    symbols
                        .globals
                        .entry(name.clone())
                        .or_default()
                        .written_after_init = true;
                }
                collect_globals_in_function(&func.body, symbols);
            }
            StmtNode::If { body, orelse, .. } | StmtNode::While { body, orelse, .. } => {
                collect_globals_in_function(body, symbols);
                collect_globals_in_function(orelse, symbols);
            }
            StmtNode::Try {
                body,
                handler,
                orelse,
                finalbody,
            } => {
                collect_globals_in_function(body, symbols);
                if let Some(h) = handler {
                    collect_globals_in_function(h, symbols);
                }
                collect_globals_in_function(orelse, symbols);
                collect_globals_in_function(finalbody, symbols);
            }
            StmtNode::Assign { .. }
            | StmtNode::Delete { .. }
            | StmtNode::Break
            | StmtNode::Continue
            | StmtNode::Return { .. }
            | StmtNode::Expr(_)
            | StmtNode::Pass => {}
        }
    }
}

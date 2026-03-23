#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ScopeKind {
    Function,
    Class,
    Module,
}

pub fn is_internal_symbol(name: &str) -> bool {
    name.starts_with("_dp_") || name == "__dp__"
}

pub fn cell_name(name: &str) -> String {
    format!("_dp_cell_{name}")
}

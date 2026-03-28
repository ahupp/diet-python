use ruff_python_ast as ast;
use ruff_text_size::TextRange;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum BinOpKind {
    Add,
    Sub,
    Mul,
    MatMul,
    TrueDiv,
    FloorDiv,
    Mod,
    LShift,
    RShift,
    Or,
    Xor,
    And,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Contains,
    Is,
    IsNot,
}

impl BinOpKind {
    pub fn helper_name(self) -> &'static str {
        match self {
            Self::Add => "__dp_add",
            Self::Sub => "__dp_sub",
            Self::Mul => "__dp_mul",
            Self::MatMul => "__dp_matmul",
            Self::TrueDiv => "__dp_truediv",
            Self::FloorDiv => "__dp_floordiv",
            Self::Mod => "__dp_mod",
            Self::LShift => "__dp_lshift",
            Self::RShift => "__dp_rshift",
            Self::Or => "__dp_or_",
            Self::Xor => "__dp_xor",
            Self::And => "__dp_and_",
            Self::Eq => "__dp_eq",
            Self::Ne => "__dp_ne",
            Self::Lt => "__dp_lt",
            Self::Le => "__dp_le",
            Self::Gt => "__dp_gt",
            Self::Ge => "__dp_ge",
            Self::Contains => "__dp_contains",
            Self::Is => "__dp_is_",
            Self::IsNot => "__dp_is_not",
        }
    }

    fn from_helper_name(name: &str) -> Option<Self> {
        Some(match name {
            "__dp_add" => Self::Add,
            "__dp_sub" => Self::Sub,
            "__dp_mul" => Self::Mul,
            "__dp_matmul" => Self::MatMul,
            "__dp_truediv" => Self::TrueDiv,
            "__dp_floordiv" => Self::FloorDiv,
            "__dp_mod" => Self::Mod,
            "__dp_lshift" => Self::LShift,
            "__dp_rshift" => Self::RShift,
            "__dp_or_" => Self::Or,
            "__dp_xor" => Self::Xor,
            "__dp_and_" => Self::And,
            "__dp_eq" => Self::Eq,
            "__dp_ne" => Self::Ne,
            "__dp_lt" => Self::Lt,
            "__dp_le" => Self::Le,
            "__dp_gt" => Self::Gt,
            "__dp_ge" => Self::Ge,
            "__dp_contains" => Self::Contains,
            "__dp_is_" => Self::Is,
            "__dp_is_not" => Self::IsNot,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum UnaryOpKind {
    Pos,
    Neg,
    Invert,
    Not,
    Truth,
}

impl UnaryOpKind {
    pub fn helper_name(self) -> &'static str {
        match self {
            Self::Pos => "__dp_pos",
            Self::Neg => "__dp_neg",
            Self::Invert => "__dp_invert",
            Self::Not => "__dp_not_",
            Self::Truth => "__dp_truth",
        }
    }

    fn from_helper_name(name: &str) -> Option<Self> {
        Some(match name {
            "__dp_pos" => Self::Pos,
            "__dp_neg" => Self::Neg,
            "__dp_invert" => Self::Invert,
            "__dp_not_" => Self::Not,
            "__dp_truth" => Self::Truth,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum InplaceBinOpKind {
    Add,
    Sub,
    Mul,
    MatMul,
    TrueDiv,
    FloorDiv,
    Mod,
    LShift,
    RShift,
    Or,
    Xor,
    And,
}

impl InplaceBinOpKind {
    pub fn helper_name(self) -> &'static str {
        match self {
            Self::Add => "__dp_iadd",
            Self::Sub => "__dp_isub",
            Self::Mul => "__dp_imul",
            Self::MatMul => "__dp_imatmul",
            Self::TrueDiv => "__dp_itruediv",
            Self::FloorDiv => "__dp_ifloordiv",
            Self::Mod => "__dp_imod",
            Self::LShift => "__dp_ilshift",
            Self::RShift => "__dp_irshift",
            Self::Or => "__dp_ior",
            Self::Xor => "__dp_ixor",
            Self::And => "__dp_iand",
        }
    }

    fn from_helper_name(name: &str) -> Option<Self> {
        Some(match name {
            "__dp_iadd" => Self::Add,
            "__dp_isub" => Self::Sub,
            "__dp_imul" => Self::Mul,
            "__dp_imatmul" => Self::MatMul,
            "__dp_itruediv" => Self::TrueDiv,
            "__dp_ifloordiv" => Self::FloorDiv,
            "__dp_imod" => Self::Mod,
            "__dp_ilshift" => Self::LShift,
            "__dp_irshift" => Self::RShift,
            "__dp_ior" => Self::Or,
            "__dp_ixor" => Self::Xor,
            "__dp_iand" => Self::And,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum TernaryOpKind {
    Pow,
}

impl TernaryOpKind {
    pub fn helper_name(self) -> &'static str {
        match self {
            Self::Pow => "__dp_pow",
        }
    }

    fn from_helper_name(name: &str) -> Option<Self> {
        Some(match name {
            "__dp_pow" => Self::Pow,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone)]
pub struct BinOp<E> {
    pub node_index: ast::AtomicNodeIndex,
    pub range: TextRange,
    pub kind: BinOpKind,
    pub arg0: E,
    pub arg1: E,
}

#[derive(Debug, Clone)]
pub struct UnaryOp<E> {
    pub node_index: ast::AtomicNodeIndex,
    pub range: TextRange,
    pub kind: UnaryOpKind,
    pub arg0: E,
}

#[derive(Debug, Clone)]
pub struct InplaceBinOp<E> {
    pub node_index: ast::AtomicNodeIndex,
    pub range: TextRange,
    pub kind: InplaceBinOpKind,
    pub arg0: E,
    pub arg1: E,
}

#[derive(Debug, Clone)]
pub struct TernaryOp<E> {
    pub node_index: ast::AtomicNodeIndex,
    pub range: TextRange,
    pub kind: TernaryOpKind,
    pub arg0: E,
    pub arg1: E,
    pub arg2: E,
}

#[derive(Debug, Clone)]
pub struct GetAttr<E> {
    pub node_index: ast::AtomicNodeIndex,
    pub range: TextRange,
    pub arg0: E,
    pub arg1: E,
}

#[derive(Debug, Clone)]
pub struct SetAttr<E> {
    pub node_index: ast::AtomicNodeIndex,
    pub range: TextRange,
    pub arg0: E,
    pub arg1: E,
    pub arg2: E,
}

#[derive(Debug, Clone)]
pub struct GetItem<E> {
    pub node_index: ast::AtomicNodeIndex,
    pub range: TextRange,
    pub arg0: E,
    pub arg1: E,
}

#[derive(Debug, Clone)]
pub struct SetItem<E> {
    pub node_index: ast::AtomicNodeIndex,
    pub range: TextRange,
    pub arg0: E,
    pub arg1: E,
    pub arg2: E,
}

#[derive(Debug, Clone)]
pub struct DelItem<E> {
    pub node_index: ast::AtomicNodeIndex,
    pub range: TextRange,
    pub arg0: E,
    pub arg1: E,
}

#[derive(Debug, Clone)]
pub struct LoadGlobal<E> {
    pub node_index: ast::AtomicNodeIndex,
    pub range: TextRange,
    pub arg0: E,
    pub arg1: E,
}

#[derive(Debug, Clone)]
pub struct StoreGlobal<E> {
    pub node_index: ast::AtomicNodeIndex,
    pub range: TextRange,
    pub arg0: E,
    pub arg1: E,
    pub arg2: E,
}

#[derive(Debug, Clone)]
pub struct LoadCell<E> {
    pub node_index: ast::AtomicNodeIndex,
    pub range: TextRange,
    pub arg0: E,
}

#[derive(Debug, Clone)]
pub struct MakeCell<E> {
    pub node_index: ast::AtomicNodeIndex,
    pub range: TextRange,
    pub arg0: E,
}

#[derive(Debug, Clone)]
pub struct CellRef<E> {
    pub node_index: ast::AtomicNodeIndex,
    pub range: TextRange,
    pub arg0: E,
}

#[derive(Debug, Clone)]
pub struct StoreCell<E> {
    pub node_index: ast::AtomicNodeIndex,
    pub range: TextRange,
    pub arg0: E,
    pub arg1: E,
}

#[derive(Debug, Clone)]
pub struct DelQuietly<E> {
    pub node_index: ast::AtomicNodeIndex,
    pub range: TextRange,
    pub arg0: E,
    pub arg1: E,
}

#[derive(Debug, Clone)]
pub struct DelDerefQuietly<E> {
    pub node_index: ast::AtomicNodeIndex,
    pub range: TextRange,
    pub arg0: E,
}

#[derive(Debug, Clone)]
pub struct DelDeref<E> {
    pub node_index: ast::AtomicNodeIndex,
    pub range: TextRange,
    pub arg0: E,
}

#[derive(Debug, Clone)]
pub enum Operation<E> {
    BinOp(BinOp<E>),
    UnaryOp(UnaryOp<E>),
    InplaceBinOp(InplaceBinOp<E>),
    TernaryOp(TernaryOp<E>),
    GetAttr(GetAttr<E>),
    SetAttr(SetAttr<E>),
    GetItem(GetItem<E>),
    SetItem(SetItem<E>),
    DelItem(DelItem<E>),
    LoadGlobal(LoadGlobal<E>),
    StoreGlobal(StoreGlobal<E>),
    LoadCell(LoadCell<E>),
    MakeCell(MakeCell<E>),
    CellRef(CellRef<E>),
    StoreCell(StoreCell<E>),
    DelQuietly(DelQuietly<E>),
    DelDerefQuietly(DelDerefQuietly<E>),
    DelDeref(DelDeref<E>),
}

impl<E> Operation<E> {
    pub fn helper_name(&self) -> &'static str {
        match self {
            Self::BinOp(op) => op.kind.helper_name(),
            Self::UnaryOp(op) => op.kind.helper_name(),
            Self::InplaceBinOp(op) => op.kind.helper_name(),
            Self::TernaryOp(op) => op.kind.helper_name(),
            Self::GetAttr(_) => "__dp_getattr",
            Self::SetAttr(_) => "__dp_setattr",
            Self::GetItem(_) => "__dp_getitem",
            Self::SetItem(_) => "__dp_setitem",
            Self::DelItem(_) => "__dp_delitem",
            Self::LoadGlobal(_) => "__dp_load_global",
            Self::StoreGlobal(_) => "__dp_store_global",
            Self::LoadCell(_) => "__dp_load_cell",
            Self::MakeCell(_) => "__dp_make_cell",
            Self::CellRef(_) => "__dp_cell_ref",
            Self::StoreCell(_) => "__dp_store_cell",
            Self::DelQuietly(_) => "__dp_del_quietly",
            Self::DelDerefQuietly(_) => "__dp_del_deref_quietly",
            Self::DelDeref(_) => "__dp_del_deref",
        }
    }

    pub fn node_index(&self) -> &ast::AtomicNodeIndex {
        match self {
            Self::BinOp(op) => &op.node_index,
            Self::UnaryOp(op) => &op.node_index,
            Self::InplaceBinOp(op) => &op.node_index,
            Self::TernaryOp(op) => &op.node_index,
            Self::GetAttr(op) => &op.node_index,
            Self::SetAttr(op) => &op.node_index,
            Self::GetItem(op) => &op.node_index,
            Self::SetItem(op) => &op.node_index,
            Self::DelItem(op) => &op.node_index,
            Self::LoadGlobal(op) => &op.node_index,
            Self::StoreGlobal(op) => &op.node_index,
            Self::LoadCell(op) => &op.node_index,
            Self::MakeCell(op) => &op.node_index,
            Self::CellRef(op) => &op.node_index,
            Self::StoreCell(op) => &op.node_index,
            Self::DelQuietly(op) => &op.node_index,
            Self::DelDerefQuietly(op) => &op.node_index,
            Self::DelDeref(op) => &op.node_index,
        }
    }

    pub fn range(&self) -> TextRange {
        match self {
            Self::BinOp(op) => op.range,
            Self::UnaryOp(op) => op.range,
            Self::InplaceBinOp(op) => op.range,
            Self::TernaryOp(op) => op.range,
            Self::GetAttr(op) => op.range,
            Self::SetAttr(op) => op.range,
            Self::GetItem(op) => op.range,
            Self::SetItem(op) => op.range,
            Self::DelItem(op) => op.range,
            Self::LoadGlobal(op) => op.range,
            Self::StoreGlobal(op) => op.range,
            Self::LoadCell(op) => op.range,
            Self::MakeCell(op) => op.range,
            Self::CellRef(op) => op.range,
            Self::StoreCell(op) => op.range,
            Self::DelQuietly(op) => op.range,
            Self::DelDerefQuietly(op) => op.range,
            Self::DelDeref(op) => op.range,
        }
    }

    pub fn map_expr<T>(self, f: &mut impl FnMut(E) -> T) -> Operation<T> {
        match self {
            Self::BinOp(op) => Operation::BinOp(BinOp {
                node_index: op.node_index,
                range: op.range,
                kind: op.kind,
                arg0: f(op.arg0),
                arg1: f(op.arg1),
            }),
            Self::UnaryOp(op) => Operation::UnaryOp(UnaryOp {
                node_index: op.node_index,
                range: op.range,
                kind: op.kind,
                arg0: f(op.arg0),
            }),
            Self::InplaceBinOp(op) => Operation::InplaceBinOp(InplaceBinOp {
                node_index: op.node_index,
                range: op.range,
                kind: op.kind,
                arg0: f(op.arg0),
                arg1: f(op.arg1),
            }),
            Self::TernaryOp(op) => Operation::TernaryOp(TernaryOp {
                node_index: op.node_index,
                range: op.range,
                kind: op.kind,
                arg0: f(op.arg0),
                arg1: f(op.arg1),
                arg2: f(op.arg2),
            }),
            Self::GetAttr(op) => Operation::GetAttr(GetAttr {
                node_index: op.node_index,
                range: op.range,
                arg0: f(op.arg0),
                arg1: f(op.arg1),
            }),
            Self::SetAttr(op) => Operation::SetAttr(SetAttr {
                node_index: op.node_index,
                range: op.range,
                arg0: f(op.arg0),
                arg1: f(op.arg1),
                arg2: f(op.arg2),
            }),
            Self::GetItem(op) => Operation::GetItem(GetItem {
                node_index: op.node_index,
                range: op.range,
                arg0: f(op.arg0),
                arg1: f(op.arg1),
            }),
            Self::SetItem(op) => Operation::SetItem(SetItem {
                node_index: op.node_index,
                range: op.range,
                arg0: f(op.arg0),
                arg1: f(op.arg1),
                arg2: f(op.arg2),
            }),
            Self::DelItem(op) => Operation::DelItem(DelItem {
                node_index: op.node_index,
                range: op.range,
                arg0: f(op.arg0),
                arg1: f(op.arg1),
            }),
            Self::LoadGlobal(op) => Operation::LoadGlobal(LoadGlobal {
                node_index: op.node_index,
                range: op.range,
                arg0: f(op.arg0),
                arg1: f(op.arg1),
            }),
            Self::StoreGlobal(op) => Operation::StoreGlobal(StoreGlobal {
                node_index: op.node_index,
                range: op.range,
                arg0: f(op.arg0),
                arg1: f(op.arg1),
                arg2: f(op.arg2),
            }),
            Self::LoadCell(op) => Operation::LoadCell(LoadCell {
                node_index: op.node_index,
                range: op.range,
                arg0: f(op.arg0),
            }),
            Self::MakeCell(op) => Operation::MakeCell(MakeCell {
                node_index: op.node_index,
                range: op.range,
                arg0: f(op.arg0),
            }),
            Self::CellRef(op) => Operation::CellRef(CellRef {
                node_index: op.node_index,
                range: op.range,
                arg0: f(op.arg0),
            }),
            Self::StoreCell(op) => Operation::StoreCell(StoreCell {
                node_index: op.node_index,
                range: op.range,
                arg0: f(op.arg0),
                arg1: f(op.arg1),
            }),
            Self::DelQuietly(op) => Operation::DelQuietly(DelQuietly {
                node_index: op.node_index,
                range: op.range,
                arg0: f(op.arg0),
                arg1: f(op.arg1),
            }),
            Self::DelDerefQuietly(op) => Operation::DelDerefQuietly(DelDerefQuietly {
                node_index: op.node_index,
                range: op.range,
                arg0: f(op.arg0),
            }),
            Self::DelDeref(op) => Operation::DelDeref(DelDeref {
                node_index: op.node_index,
                range: op.range,
                arg0: f(op.arg0),
            }),
        }
    }

    pub fn try_map_expr<T, Error>(
        self,
        f: &mut impl FnMut(E) -> Result<T, Error>,
    ) -> Result<Operation<T>, Error> {
        Ok(match self {
            Self::BinOp(op) => Operation::BinOp(BinOp {
                node_index: op.node_index,
                range: op.range,
                kind: op.kind,
                arg0: f(op.arg0)?,
                arg1: f(op.arg1)?,
            }),
            Self::UnaryOp(op) => Operation::UnaryOp(UnaryOp {
                node_index: op.node_index,
                range: op.range,
                kind: op.kind,
                arg0: f(op.arg0)?,
            }),
            Self::InplaceBinOp(op) => Operation::InplaceBinOp(InplaceBinOp {
                node_index: op.node_index,
                range: op.range,
                kind: op.kind,
                arg0: f(op.arg0)?,
                arg1: f(op.arg1)?,
            }),
            Self::TernaryOp(op) => Operation::TernaryOp(TernaryOp {
                node_index: op.node_index,
                range: op.range,
                kind: op.kind,
                arg0: f(op.arg0)?,
                arg1: f(op.arg1)?,
                arg2: f(op.arg2)?,
            }),
            Self::GetAttr(op) => Operation::GetAttr(GetAttr {
                node_index: op.node_index,
                range: op.range,
                arg0: f(op.arg0)?,
                arg1: f(op.arg1)?,
            }),
            Self::SetAttr(op) => Operation::SetAttr(SetAttr {
                node_index: op.node_index,
                range: op.range,
                arg0: f(op.arg0)?,
                arg1: f(op.arg1)?,
                arg2: f(op.arg2)?,
            }),
            Self::GetItem(op) => Operation::GetItem(GetItem {
                node_index: op.node_index,
                range: op.range,
                arg0: f(op.arg0)?,
                arg1: f(op.arg1)?,
            }),
            Self::SetItem(op) => Operation::SetItem(SetItem {
                node_index: op.node_index,
                range: op.range,
                arg0: f(op.arg0)?,
                arg1: f(op.arg1)?,
                arg2: f(op.arg2)?,
            }),
            Self::DelItem(op) => Operation::DelItem(DelItem {
                node_index: op.node_index,
                range: op.range,
                arg0: f(op.arg0)?,
                arg1: f(op.arg1)?,
            }),
            Self::LoadGlobal(op) => Operation::LoadGlobal(LoadGlobal {
                node_index: op.node_index,
                range: op.range,
                arg0: f(op.arg0)?,
                arg1: f(op.arg1)?,
            }),
            Self::StoreGlobal(op) => Operation::StoreGlobal(StoreGlobal {
                node_index: op.node_index,
                range: op.range,
                arg0: f(op.arg0)?,
                arg1: f(op.arg1)?,
                arg2: f(op.arg2)?,
            }),
            Self::LoadCell(op) => Operation::LoadCell(LoadCell {
                node_index: op.node_index,
                range: op.range,
                arg0: f(op.arg0)?,
            }),
            Self::MakeCell(op) => Operation::MakeCell(MakeCell {
                node_index: op.node_index,
                range: op.range,
                arg0: f(op.arg0)?,
            }),
            Self::CellRef(op) => Operation::CellRef(CellRef {
                node_index: op.node_index,
                range: op.range,
                arg0: f(op.arg0)?,
            }),
            Self::StoreCell(op) => Operation::StoreCell(StoreCell {
                node_index: op.node_index,
                range: op.range,
                arg0: f(op.arg0)?,
                arg1: f(op.arg1)?,
            }),
            Self::DelQuietly(op) => Operation::DelQuietly(DelQuietly {
                node_index: op.node_index,
                range: op.range,
                arg0: f(op.arg0)?,
                arg1: f(op.arg1)?,
            }),
            Self::DelDerefQuietly(op) => Operation::DelDerefQuietly(DelDerefQuietly {
                node_index: op.node_index,
                range: op.range,
                arg0: f(op.arg0)?,
            }),
            Self::DelDeref(op) => Operation::DelDeref(DelDeref {
                node_index: op.node_index,
                range: op.range,
                arg0: f(op.arg0)?,
            }),
        })
    }

    pub fn walk_args(&self, f: &mut impl FnMut(&E)) {
        match self {
            Self::BinOp(op) => {
                f(&op.arg0);
                f(&op.arg1);
            }
            Self::UnaryOp(op) => f(&op.arg0),
            Self::InplaceBinOp(op) => {
                f(&op.arg0);
                f(&op.arg1);
            }
            Self::TernaryOp(op) => {
                f(&op.arg0);
                f(&op.arg1);
                f(&op.arg2);
            }
            Self::GetAttr(op) => {
                f(&op.arg0);
                f(&op.arg1);
            }
            Self::SetAttr(op) => {
                f(&op.arg0);
                f(&op.arg1);
                f(&op.arg2);
            }
            Self::GetItem(op) => {
                f(&op.arg0);
                f(&op.arg1);
            }
            Self::SetItem(op) => {
                f(&op.arg0);
                f(&op.arg1);
                f(&op.arg2);
            }
            Self::DelItem(op) => {
                f(&op.arg0);
                f(&op.arg1);
            }
            Self::LoadGlobal(op) => {
                f(&op.arg0);
                f(&op.arg1);
            }
            Self::StoreGlobal(op) => {
                f(&op.arg0);
                f(&op.arg1);
                f(&op.arg2);
            }
            Self::LoadCell(op) => f(&op.arg0),
            Self::MakeCell(op) => f(&op.arg0),
            Self::CellRef(op) => f(&op.arg0),
            Self::StoreCell(op) => {
                f(&op.arg0);
                f(&op.arg1);
            }
            Self::DelQuietly(op) => {
                f(&op.arg0);
                f(&op.arg1);
            }
            Self::DelDerefQuietly(op) => f(&op.arg0),
            Self::DelDeref(op) => f(&op.arg0),
        }
    }

    pub fn walk_args_mut(&mut self, f: &mut impl FnMut(&mut E)) {
        match self {
            Self::BinOp(op) => {
                f(&mut op.arg0);
                f(&mut op.arg1);
            }
            Self::UnaryOp(op) => f(&mut op.arg0),
            Self::InplaceBinOp(op) => {
                f(&mut op.arg0);
                f(&mut op.arg1);
            }
            Self::TernaryOp(op) => {
                f(&mut op.arg0);
                f(&mut op.arg1);
                f(&mut op.arg2);
            }
            Self::GetAttr(op) => {
                f(&mut op.arg0);
                f(&mut op.arg1);
            }
            Self::SetAttr(op) => {
                f(&mut op.arg0);
                f(&mut op.arg1);
                f(&mut op.arg2);
            }
            Self::GetItem(op) => {
                f(&mut op.arg0);
                f(&mut op.arg1);
            }
            Self::SetItem(op) => {
                f(&mut op.arg0);
                f(&mut op.arg1);
                f(&mut op.arg2);
            }
            Self::DelItem(op) => {
                f(&mut op.arg0);
                f(&mut op.arg1);
            }
            Self::LoadGlobal(op) => {
                f(&mut op.arg0);
                f(&mut op.arg1);
            }
            Self::StoreGlobal(op) => {
                f(&mut op.arg0);
                f(&mut op.arg1);
                f(&mut op.arg2);
            }
            Self::LoadCell(op) => f(&mut op.arg0),
            Self::MakeCell(op) => f(&mut op.arg0),
            Self::CellRef(op) => f(&mut op.arg0),
            Self::StoreCell(op) => {
                f(&mut op.arg0);
                f(&mut op.arg1);
            }
            Self::DelQuietly(op) => {
                f(&mut op.arg0);
                f(&mut op.arg1);
            }
            Self::DelDerefQuietly(op) => f(&mut op.arg0),
            Self::DelDeref(op) => f(&mut op.arg0),
        }
    }

    pub fn into_call_args(self) -> Vec<E> {
        let mut out = Vec::new();
        match self {
            Self::BinOp(op) => {
                out.push(op.arg0);
                out.push(op.arg1);
            }
            Self::UnaryOp(op) => {
                out.push(op.arg0);
            }
            Self::InplaceBinOp(op) => {
                out.push(op.arg0);
                out.push(op.arg1);
            }
            Self::TernaryOp(op) => {
                out.push(op.arg0);
                out.push(op.arg1);
                out.push(op.arg2);
            }
            Self::GetAttr(op) => {
                out.push(op.arg0);
                out.push(op.arg1);
            }
            Self::SetAttr(op) => {
                out.push(op.arg0);
                out.push(op.arg1);
                out.push(op.arg2);
            }
            Self::GetItem(op) => {
                out.push(op.arg0);
                out.push(op.arg1);
            }
            Self::SetItem(op) => {
                out.push(op.arg0);
                out.push(op.arg1);
                out.push(op.arg2);
            }
            Self::DelItem(op) => {
                out.push(op.arg0);
                out.push(op.arg1);
            }
            Self::LoadGlobal(op) => {
                out.push(op.arg0);
                out.push(op.arg1);
            }
            Self::StoreGlobal(op) => {
                out.push(op.arg0);
                out.push(op.arg1);
                out.push(op.arg2);
            }
            Self::LoadCell(op) => out.push(op.arg0),
            Self::MakeCell(op) => out.push(op.arg0),
            Self::CellRef(op) => out.push(op.arg0),
            Self::StoreCell(op) => {
                out.push(op.arg0);
                out.push(op.arg1);
            }
            Self::DelQuietly(op) => {
                out.push(op.arg0);
                out.push(op.arg1);
            }
            Self::DelDerefQuietly(op) => out.push(op.arg0),
            Self::DelDeref(op) => out.push(op.arg0),
        }
        out
    }

    pub fn call_args(&self) -> Vec<&E> {
        let mut out = Vec::new();
        match self {
            Self::BinOp(op) => {
                out.push(&op.arg0);
                out.push(&op.arg1);
            }
            Self::UnaryOp(op) => {
                out.push(&op.arg0);
            }
            Self::InplaceBinOp(op) => {
                out.push(&op.arg0);
                out.push(&op.arg1);
            }
            Self::TernaryOp(op) => {
                out.push(&op.arg0);
                out.push(&op.arg1);
                out.push(&op.arg2);
            }
            Self::GetAttr(op) => {
                out.push(&op.arg0);
                out.push(&op.arg1);
            }
            Self::SetAttr(op) => {
                out.push(&op.arg0);
                out.push(&op.arg1);
                out.push(&op.arg2);
            }
            Self::GetItem(op) => {
                out.push(&op.arg0);
                out.push(&op.arg1);
            }
            Self::SetItem(op) => {
                out.push(&op.arg0);
                out.push(&op.arg1);
                out.push(&op.arg2);
            }
            Self::DelItem(op) => {
                out.push(&op.arg0);
                out.push(&op.arg1);
            }
            Self::LoadGlobal(op) => {
                out.push(&op.arg0);
                out.push(&op.arg1);
            }
            Self::StoreGlobal(op) => {
                out.push(&op.arg0);
                out.push(&op.arg1);
                out.push(&op.arg2);
            }
            Self::LoadCell(op) => out.push(&op.arg0),
            Self::MakeCell(op) => out.push(&op.arg0),
            Self::CellRef(op) => out.push(&op.arg0),
            Self::StoreCell(op) => {
                out.push(&op.arg0);
                out.push(&op.arg1);
            }
            Self::DelQuietly(op) => {
                out.push(&op.arg0);
                out.push(&op.arg1);
            }
            Self::DelDerefQuietly(op) => out.push(&op.arg0),
            Self::DelDeref(op) => out.push(&op.arg0),
        }
        out
    }
}

pub fn operation_by_name_and_args<E>(
    name: &str,
    node_index: ast::AtomicNodeIndex,
    range: TextRange,
    args: Vec<E>,
) -> Option<Operation<E>> {
    let mut args = args.into_iter();
    let operation = if let Some(kind) = BinOpKind::from_helper_name(name) {
        let arg0 = args.next()?;
        let arg1 = args.next()?;
        if args.next().is_some() {
            return None;
        }
        Operation::BinOp(BinOp {
            node_index,
            range,
            kind,
            arg0,
            arg1,
        })
    } else if let Some(kind) = UnaryOpKind::from_helper_name(name) {
        let arg0 = args.next()?;
        if args.next().is_some() {
            return None;
        }
        Operation::UnaryOp(UnaryOp {
            node_index,
            range,
            kind,
            arg0,
        })
    } else if let Some(kind) = InplaceBinOpKind::from_helper_name(name) {
        let arg0 = args.next()?;
        let arg1 = args.next()?;
        if args.next().is_some() {
            return None;
        }
        Operation::InplaceBinOp(InplaceBinOp {
            node_index,
            range,
            kind,
            arg0,
            arg1,
        })
    } else if let Some(kind) = TernaryOpKind::from_helper_name(name) {
        let arg0 = args.next()?;
        let arg1 = args.next()?;
        let arg2 = args.next()?;
        if args.next().is_some() {
            return None;
        }
        Operation::TernaryOp(TernaryOp {
            node_index,
            range,
            kind,
            arg0,
            arg1,
            arg2,
        })
    } else {
        match name {
            "__dp_getattr" => Operation::GetAttr(GetAttr {
                node_index,
                range,
                arg0: args.next()?,
                arg1: args.next()?,
            }),
            "__dp_setattr" => Operation::SetAttr(SetAttr {
                node_index,
                range,
                arg0: args.next()?,
                arg1: args.next()?,
                arg2: args.next()?,
            }),
            "__dp_getitem" => Operation::GetItem(GetItem {
                node_index,
                range,
                arg0: args.next()?,
                arg1: args.next()?,
            }),
            "__dp_setitem" => Operation::SetItem(SetItem {
                node_index,
                range,
                arg0: args.next()?,
                arg1: args.next()?,
                arg2: args.next()?,
            }),
            "__dp_delitem" => Operation::DelItem(DelItem {
                node_index,
                range,
                arg0: args.next()?,
                arg1: args.next()?,
            }),
            "__dp_load_global" => Operation::LoadGlobal(LoadGlobal {
                node_index,
                range,
                arg0: args.next()?,
                arg1: args.next()?,
            }),
            "__dp_store_global" => Operation::StoreGlobal(StoreGlobal {
                node_index,
                range,
                arg0: args.next()?,
                arg1: args.next()?,
                arg2: args.next()?,
            }),
            "__dp_load_cell" => Operation::LoadCell(LoadCell {
                node_index,
                range,
                arg0: args.next()?,
            }),
            "__dp_make_cell" => Operation::MakeCell(MakeCell {
                node_index,
                range,
                arg0: args.next()?,
            }),
            "__dp_cell_ref" => Operation::CellRef(CellRef {
                node_index,
                range,
                arg0: args.next()?,
            }),
            "__dp_store_cell" => Operation::StoreCell(StoreCell {
                node_index,
                range,
                arg0: args.next()?,
                arg1: args.next()?,
            }),
            "__dp_del_quietly" => Operation::DelQuietly(DelQuietly {
                node_index,
                range,
                arg0: args.next()?,
                arg1: args.next()?,
            }),
            "__dp_del_deref_quietly" => Operation::DelDerefQuietly(DelDerefQuietly {
                node_index,
                range,
                arg0: args.next()?,
            }),
            "__dp_del_deref" => Operation::DelDeref(DelDeref {
                node_index,
                range,
                arg0: args.next()?,
            }),
            _ => return None,
        }
    };
    if args.next().is_some() {
        return None;
    }
    Some(operation)
}

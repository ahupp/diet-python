use ruff_python_ast as ast;
use ruff_text_size::TextRange;

trait OperationArg<E>: Sized {
    type Mapped<T>;

    fn map_operation_arg<T>(self, f: &mut impl FnMut(E) -> T) -> Self::Mapped<T>;

    fn try_map_operation_arg<T, Error>(
        self,
        f: &mut impl FnMut(E) -> Result<T, Error>,
    ) -> Result<Self::Mapped<T>, Error>;

    fn walk_operation_arg(&self, f: &mut impl FnMut(&E));

    fn walk_operation_arg_mut(&mut self, f: &mut impl FnMut(&mut E));

    fn push_operation_args(self, out: &mut Vec<E>);

    fn push_operation_arg_refs<'a>(&'a self, out: &mut Vec<&'a E>);

    fn take_operation_arg(args: &mut std::vec::IntoIter<E>) -> Option<Self>;
}

impl<E> OperationArg<E> for E {
    type Mapped<T> = T;

    fn map_operation_arg<T>(self, f: &mut impl FnMut(E) -> T) -> Self::Mapped<T> {
        f(self)
    }

    fn try_map_operation_arg<T, Error>(
        self,
        f: &mut impl FnMut(E) -> Result<T, Error>,
    ) -> Result<Self::Mapped<T>, Error> {
        f(self)
    }

    fn walk_operation_arg(&self, f: &mut impl FnMut(&E)) {
        f(self);
    }

    fn walk_operation_arg_mut(&mut self, f: &mut impl FnMut(&mut E)) {
        f(self);
    }

    fn push_operation_args(self, out: &mut Vec<E>) {
        out.push(self);
    }

    fn push_operation_arg_refs<'a>(&'a self, out: &mut Vec<&'a E>) {
        out.push(self);
    }

    fn take_operation_arg(args: &mut std::vec::IntoIter<E>) -> Option<Self> {
        args.next()
    }
}

impl<E> OperationArg<E> for Option<E> {
    type Mapped<T> = Option<T>;

    fn map_operation_arg<T>(self, f: &mut impl FnMut(E) -> T) -> Self::Mapped<T> {
        self.map(f)
    }

    fn try_map_operation_arg<T, Error>(
        self,
        f: &mut impl FnMut(E) -> Result<T, Error>,
    ) -> Result<Self::Mapped<T>, Error> {
        self.map(f).transpose()
    }

    fn walk_operation_arg(&self, f: &mut impl FnMut(&E)) {
        if let Some(value) = self.as_ref() {
            f(value);
        }
    }

    fn walk_operation_arg_mut(&mut self, f: &mut impl FnMut(&mut E)) {
        if let Some(value) = self.as_mut() {
            f(value);
        }
    }

    fn push_operation_args(self, out: &mut Vec<E>) {
        if let Some(value) = self {
            out.push(value);
        }
    }

    fn push_operation_arg_refs<'a>(&'a self, out: &mut Vec<&'a E>) {
        if let Some(value) = self.as_ref() {
            out.push(value);
        }
    }

    fn take_operation_arg(args: &mut std::vec::IntoIter<E>) -> Option<Self> {
        Some(args.next())
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum BinOpKind {
    Add,
    Sub,
    Mul,
    MatMul,
    TrueDiv,
    FloorDiv,
    Mod,
    Pow,
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
            Self::Pow => "__dp_pow",
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
        }
    }

    fn accepts_arity(self, arity: usize) -> bool {
        match self {
            Self::Pow => matches!(arity, 2 | 3),
            _ => arity == 2,
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
            "__dp_pow" => Self::Pow,
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
pub enum IUnaryOpKind {
    Add,
    Sub,
    Mul,
    MatMul,
    TrueDiv,
    FloorDiv,
    Mod,
    Pow,
    LShift,
    RShift,
    Or,
    Xor,
    And,
}

impl IUnaryOpKind {
    pub fn helper_name(self) -> &'static str {
        match self {
            Self::Add => "__dp_iadd",
            Self::Sub => "__dp_isub",
            Self::Mul => "__dp_imul",
            Self::MatMul => "__dp_imatmul",
            Self::TrueDiv => "__dp_itruediv",
            Self::FloorDiv => "__dp_ifloordiv",
            Self::Mod => "__dp_imod",
            Self::Pow => "__dp_ipow",
            Self::LShift => "__dp_ilshift",
            Self::RShift => "__dp_irshift",
            Self::Or => "__dp_ior",
            Self::Xor => "__dp_ixor",
            Self::And => "__dp_iand",
        }
    }

    fn accepts_arity(self, arity: usize) -> bool {
        match self {
            Self::Pow => matches!(arity, 2 | 3),
            _ => arity == 2,
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
            "__dp_ipow" => Self::Pow,
            "__dp_ilshift" => Self::LShift,
            "__dp_irshift" => Self::RShift,
            "__dp_ior" => Self::Or,
            "__dp_ixor" => Self::Xor,
            "__dp_iand" => Self::And,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone)]
pub enum Operation<E> {
    BinOp {
        node_index: ast::AtomicNodeIndex,
        range: TextRange,
        kind: BinOpKind,
        arg0: E,
        arg1: E,
        arg2: Option<E>,
    },
    UnaryOp {
        node_index: ast::AtomicNodeIndex,
        range: TextRange,
        kind: UnaryOpKind,
        arg0: E,
    },
    IUnaryOp {
        node_index: ast::AtomicNodeIndex,
        range: TextRange,
        kind: IUnaryOpKind,
        arg0: E,
        arg1: E,
        arg2: Option<E>,
    },
    GetAttr {
        node_index: ast::AtomicNodeIndex,
        range: TextRange,
        arg0: E,
        arg1: E,
    },
    SetAttr {
        node_index: ast::AtomicNodeIndex,
        range: TextRange,
        arg0: E,
        arg1: E,
        arg2: E,
    },
    GetItem {
        node_index: ast::AtomicNodeIndex,
        range: TextRange,
        arg0: E,
        arg1: E,
    },
    SetItem {
        node_index: ast::AtomicNodeIndex,
        range: TextRange,
        arg0: E,
        arg1: E,
        arg2: E,
    },
    DelItem {
        node_index: ast::AtomicNodeIndex,
        range: TextRange,
        arg0: E,
        arg1: E,
    },
    LoadGlobal {
        node_index: ast::AtomicNodeIndex,
        range: TextRange,
        arg0: E,
        arg1: E,
    },
    StoreGlobal {
        node_index: ast::AtomicNodeIndex,
        range: TextRange,
        arg0: E,
        arg1: E,
        arg2: E,
    },
    LoadCell {
        node_index: ast::AtomicNodeIndex,
        range: TextRange,
        arg0: E,
    },
    MakeCell {
        node_index: ast::AtomicNodeIndex,
        range: TextRange,
        arg0: E,
    },
    CellRef {
        node_index: ast::AtomicNodeIndex,
        range: TextRange,
        arg0: E,
    },
    StoreCell {
        node_index: ast::AtomicNodeIndex,
        range: TextRange,
        arg0: E,
        arg1: E,
    },
    DelQuietly {
        node_index: ast::AtomicNodeIndex,
        range: TextRange,
        arg0: E,
        arg1: E,
    },
    DelDerefQuietly {
        node_index: ast::AtomicNodeIndex,
        range: TextRange,
        arg0: E,
    },
    DelDeref {
        node_index: ast::AtomicNodeIndex,
        range: TextRange,
        arg0: E,
    },
    Is {
        node_index: ast::AtomicNodeIndex,
        range: TextRange,
        arg0: E,
        arg1: E,
    },
    IsNot {
        node_index: ast::AtomicNodeIndex,
        range: TextRange,
        arg0: E,
        arg1: E,
    },
}

impl<E> Operation<E> {
    pub fn helper_name(&self) -> &'static str {
        match self {
            Self::BinOp { kind, .. } => kind.helper_name(),
            Self::UnaryOp { kind, .. } => kind.helper_name(),
            Self::IUnaryOp { kind, .. } => kind.helper_name(),
            Self::GetAttr { .. } => "__dp_getattr",
            Self::SetAttr { .. } => "__dp_setattr",
            Self::GetItem { .. } => "__dp_getitem",
            Self::SetItem { .. } => "__dp_setitem",
            Self::DelItem { .. } => "__dp_delitem",
            Self::LoadGlobal { .. } => "__dp_load_global",
            Self::StoreGlobal { .. } => "__dp_store_global",
            Self::LoadCell { .. } => "__dp_load_cell",
            Self::MakeCell { .. } => "__dp_make_cell",
            Self::CellRef { .. } => "__dp_cell_ref",
            Self::StoreCell { .. } => "__dp_store_cell",
            Self::DelQuietly { .. } => "__dp_del_quietly",
            Self::DelDerefQuietly { .. } => "__dp_del_deref_quietly",
            Self::DelDeref { .. } => "__dp_del_deref",
            Self::Is { .. } => "__dp_is_",
            Self::IsNot { .. } => "__dp_is_not",
        }
    }

    pub fn node_index(&self) -> &ast::AtomicNodeIndex {
        match self {
            Self::BinOp { node_index, .. }
            | Self::UnaryOp { node_index, .. }
            | Self::IUnaryOp { node_index, .. }
            | Self::GetAttr { node_index, .. }
            | Self::SetAttr { node_index, .. }
            | Self::GetItem { node_index, .. }
            | Self::SetItem { node_index, .. }
            | Self::DelItem { node_index, .. }
            | Self::LoadGlobal { node_index, .. }
            | Self::StoreGlobal { node_index, .. }
            | Self::LoadCell { node_index, .. }
            | Self::MakeCell { node_index, .. }
            | Self::CellRef { node_index, .. }
            | Self::StoreCell { node_index, .. }
            | Self::DelQuietly { node_index, .. }
            | Self::DelDerefQuietly { node_index, .. }
            | Self::DelDeref { node_index, .. }
            | Self::Is { node_index, .. }
            | Self::IsNot { node_index, .. } => node_index,
        }
    }

    pub fn range(&self) -> TextRange {
        match self {
            Self::BinOp { range, .. }
            | Self::UnaryOp { range, .. }
            | Self::IUnaryOp { range, .. }
            | Self::GetAttr { range, .. }
            | Self::SetAttr { range, .. }
            | Self::GetItem { range, .. }
            | Self::SetItem { range, .. }
            | Self::DelItem { range, .. }
            | Self::LoadGlobal { range, .. }
            | Self::StoreGlobal { range, .. }
            | Self::LoadCell { range, .. }
            | Self::MakeCell { range, .. }
            | Self::CellRef { range, .. }
            | Self::StoreCell { range, .. }
            | Self::DelQuietly { range, .. }
            | Self::DelDerefQuietly { range, .. }
            | Self::DelDeref { range, .. }
            | Self::Is { range, .. }
            | Self::IsNot { range, .. } => *range,
        }
    }

    pub fn map_expr<T>(self, f: &mut impl FnMut(E) -> T) -> Operation<T> {
        match self {
            Self::BinOp {
                node_index,
                range,
                kind,
                arg0,
                arg1,
                arg2,
            } => Operation::BinOp {
                node_index,
                range,
                kind,
                arg0: f(arg0),
                arg1: f(arg1),
                arg2: <Option<E> as OperationArg<E>>::map_operation_arg(arg2, f),
            },
            Self::UnaryOp {
                node_index,
                range,
                kind,
                arg0,
            } => Operation::UnaryOp {
                node_index,
                range,
                kind,
                arg0: f(arg0),
            },
            Self::IUnaryOp {
                node_index,
                range,
                kind,
                arg0,
                arg1,
                arg2,
            } => Operation::IUnaryOp {
                node_index,
                range,
                kind,
                arg0: f(arg0),
                arg1: f(arg1),
                arg2: <Option<E> as OperationArg<E>>::map_operation_arg(arg2, f),
            },
            Self::GetAttr {
                node_index,
                range,
                arg0,
                arg1,
            } => Operation::GetAttr {
                node_index,
                range,
                arg0: f(arg0),
                arg1: f(arg1),
            },
            Self::SetAttr {
                node_index,
                range,
                arg0,
                arg1,
                arg2,
            } => Operation::SetAttr {
                node_index,
                range,
                arg0: f(arg0),
                arg1: f(arg1),
                arg2: f(arg2),
            },
            Self::GetItem {
                node_index,
                range,
                arg0,
                arg1,
            } => Operation::GetItem {
                node_index,
                range,
                arg0: f(arg0),
                arg1: f(arg1),
            },
            Self::SetItem {
                node_index,
                range,
                arg0,
                arg1,
                arg2,
            } => Operation::SetItem {
                node_index,
                range,
                arg0: f(arg0),
                arg1: f(arg1),
                arg2: f(arg2),
            },
            Self::DelItem {
                node_index,
                range,
                arg0,
                arg1,
            } => Operation::DelItem {
                node_index,
                range,
                arg0: f(arg0),
                arg1: f(arg1),
            },
            Self::LoadGlobal {
                node_index,
                range,
                arg0,
                arg1,
            } => Operation::LoadGlobal {
                node_index,
                range,
                arg0: f(arg0),
                arg1: f(arg1),
            },
            Self::StoreGlobal {
                node_index,
                range,
                arg0,
                arg1,
                arg2,
            } => Operation::StoreGlobal {
                node_index,
                range,
                arg0: f(arg0),
                arg1: f(arg1),
                arg2: f(arg2),
            },
            Self::LoadCell {
                node_index,
                range,
                arg0,
            } => Operation::LoadCell {
                node_index,
                range,
                arg0: f(arg0),
            },
            Self::MakeCell {
                node_index,
                range,
                arg0,
            } => Operation::MakeCell {
                node_index,
                range,
                arg0: f(arg0),
            },
            Self::CellRef {
                node_index,
                range,
                arg0,
            } => Operation::CellRef {
                node_index,
                range,
                arg0: f(arg0),
            },
            Self::StoreCell {
                node_index,
                range,
                arg0,
                arg1,
            } => Operation::StoreCell {
                node_index,
                range,
                arg0: f(arg0),
                arg1: f(arg1),
            },
            Self::DelQuietly {
                node_index,
                range,
                arg0,
                arg1,
            } => Operation::DelQuietly {
                node_index,
                range,
                arg0: f(arg0),
                arg1: f(arg1),
            },
            Self::DelDerefQuietly {
                node_index,
                range,
                arg0,
            } => Operation::DelDerefQuietly {
                node_index,
                range,
                arg0: f(arg0),
            },
            Self::DelDeref {
                node_index,
                range,
                arg0,
            } => Operation::DelDeref {
                node_index,
                range,
                arg0: f(arg0),
            },
            Self::Is {
                node_index,
                range,
                arg0,
                arg1,
            } => Operation::Is {
                node_index,
                range,
                arg0: f(arg0),
                arg1: f(arg1),
            },
            Self::IsNot {
                node_index,
                range,
                arg0,
                arg1,
            } => Operation::IsNot {
                node_index,
                range,
                arg0: f(arg0),
                arg1: f(arg1),
            },
        }
    }

    pub fn try_map_expr<T, Error>(
        self,
        f: &mut impl FnMut(E) -> Result<T, Error>,
    ) -> Result<Operation<T>, Error> {
        Ok(match self {
            Self::BinOp {
                node_index,
                range,
                kind,
                arg0,
                arg1,
                arg2,
            } => Operation::BinOp {
                node_index,
                range,
                kind,
                arg0: f(arg0)?,
                arg1: f(arg1)?,
                arg2: <Option<E> as OperationArg<E>>::try_map_operation_arg(arg2, f)?,
            },
            Self::UnaryOp {
                node_index,
                range,
                kind,
                arg0,
            } => Operation::UnaryOp {
                node_index,
                range,
                kind,
                arg0: f(arg0)?,
            },
            Self::IUnaryOp {
                node_index,
                range,
                kind,
                arg0,
                arg1,
                arg2,
            } => Operation::IUnaryOp {
                node_index,
                range,
                kind,
                arg0: f(arg0)?,
                arg1: f(arg1)?,
                arg2: <Option<E> as OperationArg<E>>::try_map_operation_arg(arg2, f)?,
            },
            Self::GetAttr {
                node_index,
                range,
                arg0,
                arg1,
            } => Operation::GetAttr {
                node_index,
                range,
                arg0: f(arg0)?,
                arg1: f(arg1)?,
            },
            Self::SetAttr {
                node_index,
                range,
                arg0,
                arg1,
                arg2,
            } => Operation::SetAttr {
                node_index,
                range,
                arg0: f(arg0)?,
                arg1: f(arg1)?,
                arg2: f(arg2)?,
            },
            Self::GetItem {
                node_index,
                range,
                arg0,
                arg1,
            } => Operation::GetItem {
                node_index,
                range,
                arg0: f(arg0)?,
                arg1: f(arg1)?,
            },
            Self::SetItem {
                node_index,
                range,
                arg0,
                arg1,
                arg2,
            } => Operation::SetItem {
                node_index,
                range,
                arg0: f(arg0)?,
                arg1: f(arg1)?,
                arg2: f(arg2)?,
            },
            Self::DelItem {
                node_index,
                range,
                arg0,
                arg1,
            } => Operation::DelItem {
                node_index,
                range,
                arg0: f(arg0)?,
                arg1: f(arg1)?,
            },
            Self::LoadGlobal {
                node_index,
                range,
                arg0,
                arg1,
            } => Operation::LoadGlobal {
                node_index,
                range,
                arg0: f(arg0)?,
                arg1: f(arg1)?,
            },
            Self::StoreGlobal {
                node_index,
                range,
                arg0,
                arg1,
                arg2,
            } => Operation::StoreGlobal {
                node_index,
                range,
                arg0: f(arg0)?,
                arg1: f(arg1)?,
                arg2: f(arg2)?,
            },
            Self::LoadCell {
                node_index,
                range,
                arg0,
            } => Operation::LoadCell {
                node_index,
                range,
                arg0: f(arg0)?,
            },
            Self::MakeCell {
                node_index,
                range,
                arg0,
            } => Operation::MakeCell {
                node_index,
                range,
                arg0: f(arg0)?,
            },
            Self::CellRef {
                node_index,
                range,
                arg0,
            } => Operation::CellRef {
                node_index,
                range,
                arg0: f(arg0)?,
            },
            Self::StoreCell {
                node_index,
                range,
                arg0,
                arg1,
            } => Operation::StoreCell {
                node_index,
                range,
                arg0: f(arg0)?,
                arg1: f(arg1)?,
            },
            Self::DelQuietly {
                node_index,
                range,
                arg0,
                arg1,
            } => Operation::DelQuietly {
                node_index,
                range,
                arg0: f(arg0)?,
                arg1: f(arg1)?,
            },
            Self::DelDerefQuietly {
                node_index,
                range,
                arg0,
            } => Operation::DelDerefQuietly {
                node_index,
                range,
                arg0: f(arg0)?,
            },
            Self::DelDeref {
                node_index,
                range,
                arg0,
            } => Operation::DelDeref {
                node_index,
                range,
                arg0: f(arg0)?,
            },
            Self::Is {
                node_index,
                range,
                arg0,
                arg1,
            } => Operation::Is {
                node_index,
                range,
                arg0: f(arg0)?,
                arg1: f(arg1)?,
            },
            Self::IsNot {
                node_index,
                range,
                arg0,
                arg1,
            } => Operation::IsNot {
                node_index,
                range,
                arg0: f(arg0)?,
                arg1: f(arg1)?,
            },
        })
    }

    pub fn walk_args(&self, f: &mut impl FnMut(&E)) {
        match self {
            Self::BinOp {
                arg0, arg1, arg2, ..
            }
            | Self::IUnaryOp {
                arg0, arg1, arg2, ..
            } => {
                f(arg0);
                f(arg1);
                <Option<E> as OperationArg<E>>::walk_operation_arg(arg2, f);
            }
            Self::UnaryOp { arg0, .. }
            | Self::LoadCell { arg0, .. }
            | Self::MakeCell { arg0, .. }
            | Self::CellRef { arg0, .. }
            | Self::DelDerefQuietly { arg0, .. }
            | Self::DelDeref { arg0, .. } => f(arg0),
            Self::GetAttr { arg0, arg1, .. }
            | Self::GetItem { arg0, arg1, .. }
            | Self::DelItem { arg0, arg1, .. }
            | Self::LoadGlobal { arg0, arg1, .. }
            | Self::StoreCell { arg0, arg1, .. }
            | Self::DelQuietly { arg0, arg1, .. }
            | Self::Is { arg0, arg1, .. }
            | Self::IsNot { arg0, arg1, .. } => {
                f(arg0);
                f(arg1);
            }
            Self::SetAttr {
                arg0, arg1, arg2, ..
            }
            | Self::SetItem {
                arg0, arg1, arg2, ..
            }
            | Self::StoreGlobal {
                arg0, arg1, arg2, ..
            } => {
                f(arg0);
                f(arg1);
                f(arg2);
            }
        }
    }

    pub fn walk_args_mut(&mut self, f: &mut impl FnMut(&mut E)) {
        match self {
            Self::BinOp {
                arg0, arg1, arg2, ..
            }
            | Self::IUnaryOp {
                arg0, arg1, arg2, ..
            } => {
                f(arg0);
                f(arg1);
                <Option<E> as OperationArg<E>>::walk_operation_arg_mut(arg2, f);
            }
            Self::UnaryOp { arg0, .. }
            | Self::LoadCell { arg0, .. }
            | Self::MakeCell { arg0, .. }
            | Self::CellRef { arg0, .. }
            | Self::DelDerefQuietly { arg0, .. }
            | Self::DelDeref { arg0, .. } => f(arg0),
            Self::GetAttr { arg0, arg1, .. }
            | Self::GetItem { arg0, arg1, .. }
            | Self::DelItem { arg0, arg1, .. }
            | Self::LoadGlobal { arg0, arg1, .. }
            | Self::StoreCell { arg0, arg1, .. }
            | Self::DelQuietly { arg0, arg1, .. }
            | Self::Is { arg0, arg1, .. }
            | Self::IsNot { arg0, arg1, .. } => {
                f(arg0);
                f(arg1);
            }
            Self::SetAttr {
                arg0, arg1, arg2, ..
            }
            | Self::SetItem {
                arg0, arg1, arg2, ..
            }
            | Self::StoreGlobal {
                arg0, arg1, arg2, ..
            } => {
                f(arg0);
                f(arg1);
                f(arg2);
            }
        }
    }

    pub fn into_call_args(self) -> Vec<E> {
        let mut out = Vec::new();
        match self {
            Self::BinOp {
                arg0, arg1, arg2, ..
            }
            | Self::IUnaryOp {
                arg0, arg1, arg2, ..
            } => {
                out.push(arg0);
                out.push(arg1);
                <Option<E> as OperationArg<E>>::push_operation_args(arg2, &mut out);
            }
            Self::UnaryOp { arg0, .. }
            | Self::LoadCell { arg0, .. }
            | Self::MakeCell { arg0, .. }
            | Self::CellRef { arg0, .. }
            | Self::DelDerefQuietly { arg0, .. }
            | Self::DelDeref { arg0, .. } => {
                out.push(arg0);
            }
            Self::GetAttr { arg0, arg1, .. }
            | Self::GetItem { arg0, arg1, .. }
            | Self::DelItem { arg0, arg1, .. }
            | Self::LoadGlobal { arg0, arg1, .. }
            | Self::StoreCell { arg0, arg1, .. }
            | Self::DelQuietly { arg0, arg1, .. }
            | Self::Is { arg0, arg1, .. }
            | Self::IsNot { arg0, arg1, .. } => {
                out.push(arg0);
                out.push(arg1);
            }
            Self::SetAttr {
                arg0, arg1, arg2, ..
            }
            | Self::SetItem {
                arg0, arg1, arg2, ..
            }
            | Self::StoreGlobal {
                arg0, arg1, arg2, ..
            } => {
                out.push(arg0);
                out.push(arg1);
                out.push(arg2);
            }
        }
        out
    }

    pub fn call_args(&self) -> Vec<&E> {
        let mut out = Vec::new();
        match self {
            Self::BinOp {
                arg0, arg1, arg2, ..
            }
            | Self::IUnaryOp {
                arg0, arg1, arg2, ..
            } => {
                out.push(arg0);
                out.push(arg1);
                <Option<E> as OperationArg<E>>::push_operation_arg_refs(arg2, &mut out);
            }
            Self::UnaryOp { arg0, .. }
            | Self::LoadCell { arg0, .. }
            | Self::MakeCell { arg0, .. }
            | Self::CellRef { arg0, .. }
            | Self::DelDerefQuietly { arg0, .. }
            | Self::DelDeref { arg0, .. } => {
                out.push(arg0);
            }
            Self::GetAttr { arg0, arg1, .. }
            | Self::GetItem { arg0, arg1, .. }
            | Self::DelItem { arg0, arg1, .. }
            | Self::LoadGlobal { arg0, arg1, .. }
            | Self::StoreCell { arg0, arg1, .. }
            | Self::DelQuietly { arg0, arg1, .. }
            | Self::Is { arg0, arg1, .. }
            | Self::IsNot { arg0, arg1, .. } => {
                out.push(arg0);
                out.push(arg1);
            }
            Self::SetAttr {
                arg0, arg1, arg2, ..
            }
            | Self::SetItem {
                arg0, arg1, arg2, ..
            }
            | Self::StoreGlobal {
                arg0, arg1, arg2, ..
            } => {
                out.push(arg0);
                out.push(arg1);
                out.push(arg2);
            }
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
        let arg2 = args.next();
        if !kind.accepts_arity(2 + usize::from(arg2.is_some())) {
            return None;
        }
        Operation::BinOp {
            node_index,
            range,
            kind,
            arg0,
            arg1,
            arg2,
        }
    } else if let Some(kind) = UnaryOpKind::from_helper_name(name) {
        let arg0 = args.next()?;
        if args.next().is_some() {
            return None;
        }
        Operation::UnaryOp {
            node_index,
            range,
            kind,
            arg0,
        }
    } else if let Some(kind) = IUnaryOpKind::from_helper_name(name) {
        let arg0 = args.next()?;
        let arg1 = args.next()?;
        let arg2 = args.next();
        if !kind.accepts_arity(2 + usize::from(arg2.is_some())) {
            return None;
        }
        Operation::IUnaryOp {
            node_index,
            range,
            kind,
            arg0,
            arg1,
            arg2,
        }
    } else {
        match name {
            "__dp_getattr" => Operation::GetAttr {
                node_index,
                range,
                arg0: args.next()?,
                arg1: args.next()?,
            },
            "__dp_setattr" => Operation::SetAttr {
                node_index,
                range,
                arg0: args.next()?,
                arg1: args.next()?,
                arg2: args.next()?,
            },
            "__dp_getitem" => Operation::GetItem {
                node_index,
                range,
                arg0: args.next()?,
                arg1: args.next()?,
            },
            "__dp_setitem" => Operation::SetItem {
                node_index,
                range,
                arg0: args.next()?,
                arg1: args.next()?,
                arg2: args.next()?,
            },
            "__dp_delitem" => Operation::DelItem {
                node_index,
                range,
                arg0: args.next()?,
                arg1: args.next()?,
            },
            "__dp_load_global" => Operation::LoadGlobal {
                node_index,
                range,
                arg0: args.next()?,
                arg1: args.next()?,
            },
            "__dp_store_global" => Operation::StoreGlobal {
                node_index,
                range,
                arg0: args.next()?,
                arg1: args.next()?,
                arg2: args.next()?,
            },
            "__dp_load_cell" => Operation::LoadCell {
                node_index,
                range,
                arg0: args.next()?,
            },
            "__dp_make_cell" => Operation::MakeCell {
                node_index,
                range,
                arg0: args.next()?,
            },
            "__dp_cell_ref" => Operation::CellRef {
                node_index,
                range,
                arg0: args.next()?,
            },
            "__dp_store_cell" => Operation::StoreCell {
                node_index,
                range,
                arg0: args.next()?,
                arg1: args.next()?,
            },
            "__dp_del_quietly" => Operation::DelQuietly {
                node_index,
                range,
                arg0: args.next()?,
                arg1: args.next()?,
            },
            "__dp_del_deref_quietly" => Operation::DelDerefQuietly {
                node_index,
                range,
                arg0: args.next()?,
            },
            "__dp_del_deref" => Operation::DelDeref {
                node_index,
                range,
                arg0: args.next()?,
            },
            "__dp_is_" => Operation::Is {
                node_index,
                range,
                arg0: args.next()?,
                arg1: args.next()?,
            },
            "__dp_is_not" => Operation::IsNot {
                node_index,
                range,
                arg0: args.next()?,
                arg1: args.next()?,
            },
            _ => return None,
        }
    };
    if args.next().is_some() {
        return None;
    }
    Some(operation)
}

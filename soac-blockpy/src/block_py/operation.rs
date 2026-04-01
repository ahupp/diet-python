use super::{
    BlockPyFunctionKind, CellLocation, FunctionId, HasMeta, LocalLocation, Meta, WithMeta,
};
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
}

impl BinOpKind {
    pub(crate) fn from_helper_name(name: &str) -> Option<Self> {
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
    pub(crate) fn from_helper_name(name: &str) -> Option<Self> {
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
    pub(crate) fn from_helper_name(name: &str) -> Option<Self> {
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
    pub(crate) fn from_helper_name(name: &str) -> Option<Self> {
        Some(match name {
            "__dp_pow" => Self::Pow,
            _ => return None,
        })
    }
}

pub trait ExprOperationNode<E>: Sized {
    type Mapped<T>;

    fn visit_exprs(&self, f: &mut impl FnMut(&E));
    fn visit_exprs_mut(&mut self, f: &mut impl FnMut(&mut E));
    fn map_op<T>(self, f: &mut impl FnMut(E) -> T) -> Self::Mapped<T>;
    fn try_map_op<T, Error>(
        self,
        f: &mut impl FnMut(E) -> Result<T, Error>,
    ) -> Result<Self::Mapped<T>, Error>;
}

macro_rules! define_operation {
    (
        $vis:vis struct $name:ident<$expr_ty:ident> {
            $($fields:tt)*
        }
    ) => {
        define_operation!(
            @collect_fields
            [$vis]
            [$name]
            [$expr_ty]
            [$($fields)*]
            []
            []
            []
            $($fields)*
        );
    };
    (
        @collect_fields
        [$vis:vis]
        [$name:ident]
        [$expr_ty:ident]
        [$($raw_fields:tt)*]
        [$($struct_fields:tt)*]
        [$($ctor_args:tt)*]
        [$($ctor_init:tt)*]
    ) => {
        #[derive(Debug, Clone)]
        $vis struct $name<$expr_ty> {
            _meta: Meta,
            $($struct_fields)*
        }

        impl<$expr_ty> $name<$expr_ty> {
            pub fn new($($ctor_args)*) -> Self {
                Self {
                    _meta: Meta::default(),
                    $($ctor_init)*
                }
            }
        }

        impl<$expr_ty> HasMeta for $name<$expr_ty> {
            fn meta(&self) -> Meta {
                self._meta.clone()
            }
        }

        impl<$expr_ty> WithMeta for $name<$expr_ty> {
            fn with_meta(mut self, meta: Meta) -> Self {
                self._meta = meta;
                self
            }
        }

        impl<$expr_ty> ExprOperationNode<$expr_ty> for $name<$expr_ty> {
            type Mapped<T> = $name<T>;

            fn visit_exprs(&self, f: &mut impl FnMut(&$expr_ty)) {
                #[allow(unused_variables)]
                let _ = &f;
                define_operation!(@visit_expr_fields self, f, $($raw_fields)*);
            }

            fn visit_exprs_mut(&mut self, f: &mut impl FnMut(&mut $expr_ty)) {
                #[allow(unused_variables)]
                let _ = &f;
                define_operation!(@visit_expr_fields_mut self, f, $($raw_fields)*);
            }

            fn map_op<T>(self, f: &mut impl FnMut($expr_ty) -> T) -> Self::Mapped<T> {
                #[allow(unused_variables)]
                let _ = &f;
                define_operation!(@build_mapped [$name::<T>] [] self, f, $($raw_fields)*)
            }

            fn try_map_op<T, Error>(
                self,
                f: &mut impl FnMut($expr_ty) -> Result<T, Error>,
            ) -> Result<Self::Mapped<T>, Error> {
                #[allow(unused_variables)]
                let _ = &f;
                define_operation!(@build_try_mapped [$name::<T>] [] self, f, $($raw_fields)*)
            }
        }
    };
    (
        $vis:vis struct $name:ident {
            $($fields:tt)*
        }
    ) => {
        define_operation!(
            @collect_value_fields
            [$vis]
            [$name]
            []
            []
            []
            $($fields)*
        );
    };
    (
        @collect_value_fields
        [$vis:vis]
        [$name:ident]
        [$($struct_fields:tt)*]
        [$($ctor_args:tt)*]
        [$($ctor_init:tt)*]
    ) => {
        #[derive(Debug, Clone)]
        $vis struct $name {
            _meta: Meta,
            $($struct_fields)*
        }

        impl $name {
            pub fn new($($ctor_args)*) -> Self {
                Self {
                    _meta: Meta::default(),
                    $($ctor_init)*
                }
            }
        }

        impl HasMeta for $name {
            fn meta(&self) -> Meta {
                self._meta.clone()
            }
        }

        impl WithMeta for $name {
            fn with_meta(mut self, meta: Meta) -> Self {
                self._meta = meta;
                self
            }
        }

        impl<E> ExprOperationNode<E> for $name {
            type Mapped<T> = $name;

            fn visit_exprs(&self, f: &mut impl FnMut(&E)) {
                let _ = &f;
            }

            fn visit_exprs_mut(&mut self, f: &mut impl FnMut(&mut E)) {
                let _ = &f;
            }

            fn map_op<T>(self, f: &mut impl FnMut(E) -> T) -> Self::Mapped<T> {
                let _ = &f;
                self
            }

            fn try_map_op<T, Error>(
                self,
                f: &mut impl FnMut(E) -> Result<T, Error>,
            ) -> Result<Self::Mapped<T>, Error> {
                let _ = &f;
                Ok(self)
            }
        }
    };
    (
        @collect_value_fields
        [$vis:vis]
        [$name:ident]
        [$($struct_fields:tt)*]
        [$($ctor_args:tt)*]
        [$($ctor_init:tt)*]
        $field:ident : $ty:ty,
        $($rest:tt)*
    ) => {
        define_operation!(
            @collect_value_fields
            [$vis]
            [$name]
            [$($struct_fields)* pub $field: $ty,]
            [$($ctor_args)* $field: impl Into<$ty>,]
            [$($ctor_init)* $field: $field.into(),]
            $($rest)*
        );
    };
    (
        @collect_value_fields
        [$vis:vis]
        [$name:ident]
        [$($struct_fields:tt)*]
        [$($ctor_args:tt)*]
        [$($ctor_init:tt)*]
        $field:ident : $ty:ty
    ) => {
        define_operation!(
            @collect_value_fields
            [$vis]
            [$name]
            [$($struct_fields)* pub $field: $ty,]
            [$($ctor_args)* $field: impl Into<$ty>,]
            [$($ctor_init)* $field: $field.into(),]
        );
    };
    (
        @collect_fields
        [$vis:vis]
        [$name:ident]
        [$expr_ty:ident]
        [$($raw_fields:tt)*]
        [$($struct_fields:tt)*]
        [$($ctor_args:tt)*]
        [$($ctor_init:tt)*]
        $field:ident : Box<$inner_expr_ty:ident>,
        $($rest:tt)*
    ) => {
        define_operation!(
            @collect_fields
            [$vis]
            [$name]
            [$expr_ty]
            [$($raw_fields)*]
            [$($struct_fields)* pub $field: Box<$inner_expr_ty>,]
            [$($ctor_args)* $field: impl Into<Box<$inner_expr_ty>>,]
            [$($ctor_init)* $field: $field.into(),]
            $($rest)*
        );
    };
    (
        @collect_fields
        [$vis:vis]
        [$name:ident]
        [$expr_ty:ident]
        [$($raw_fields:tt)*]
        [$($struct_fields:tt)*]
        [$($ctor_args:tt)*]
        [$($ctor_init:tt)*]
        $field:ident : Box<$inner_expr_ty:ident>
    ) => {
        define_operation!(
            @collect_fields
            [$vis]
            [$name]
            [$expr_ty]
            [$($raw_fields)*]
            [$($struct_fields)* pub $field: Box<$inner_expr_ty>,]
            [$($ctor_args)* $field: impl Into<Box<$inner_expr_ty>>,]
            [$($ctor_init)* $field: $field.into(),]
        );
    };
    (
        @collect_fields
        [$vis:vis]
        [$name:ident]
        [$expr_ty:ident]
        [$($raw_fields:tt)*]
        [$($struct_fields:tt)*]
        [$($ctor_args:tt)*]
        [$($ctor_init:tt)*]
        $field:ident : $ty:ty,
        $($rest:tt)*
    ) => {
        define_operation!(
            @collect_fields
            [$vis]
            [$name]
            [$expr_ty]
            [$($raw_fields)*]
            [$($struct_fields)* pub $field: $ty,]
            [$($ctor_args)* $field: impl Into<$ty>,]
            [$($ctor_init)* $field: $field.into(),]
            $($rest)*
        );
    };
    (
        @collect_fields
        [$vis:vis]
        [$name:ident]
        [$expr_ty:ident]
        [$($raw_fields:tt)*]
        [$($struct_fields:tt)*]
        [$($ctor_args:tt)*]
        [$($ctor_init:tt)*]
        $field:ident : $ty:ty
    ) => {
        define_operation!(
            @collect_fields
            [$vis]
            [$name]
            [$expr_ty]
            [$($raw_fields)*]
            [$($struct_fields)* pub $field: $ty,]
            [$($ctor_args)* $field: impl Into<$ty>,]
            [$($ctor_init)* $field: $field.into(),]
        );
    };
    (@visit_expr_fields $self:ident, $f:ident,) => {};
    (@visit_expr_fields $self:ident, $f:ident, $field:ident : Box<$expr_ty:ident>, $($rest:tt)*) => {
        $f(&$self.$field);
        define_operation!(@visit_expr_fields $self, $f, $($rest)*);
    };
    (@visit_expr_fields $self:ident, $f:ident, $field:ident : Box<$expr_ty:ident>) => {
        $f(&$self.$field);
    };
    (@visit_expr_fields $self:ident, $f:ident, $field:ident : $ty:ty, $($rest:tt)*) => {
        define_operation!(@visit_expr_fields $self, $f, $($rest)*);
    };
    (@visit_expr_fields $self:ident, $f:ident, $field:ident : $ty:ty) => {};
    (@visit_expr_fields_mut $self:ident, $f:ident,) => {};
    (@visit_expr_fields_mut $self:ident, $f:ident, $field:ident : Box<$expr_ty:ident>, $($rest:tt)*) => {
        $f(&mut $self.$field);
        define_operation!(@visit_expr_fields_mut $self, $f, $($rest)*);
    };
    (@visit_expr_fields_mut $self:ident, $f:ident, $field:ident : Box<$expr_ty:ident>) => {
        $f(&mut $self.$field);
    };
    (@visit_expr_fields_mut $self:ident, $f:ident, $field:ident : $ty:ty, $($rest:tt)*) => {
        define_operation!(@visit_expr_fields_mut $self, $f, $($rest)*);
    };
    (@visit_expr_fields_mut $self:ident, $f:ident, $field:ident : $ty:ty) => {};
    (@build_mapped [$($mapped_ctor:tt)+] [$($out:tt)*] $self:ident, $f:ident,) => {
        $($mapped_ctor)+ { _meta: $self._meta, $($out)* }
    };
    (@build_mapped [$($mapped_ctor:tt)+] [$($out:tt)*] $self:ident, $f:ident, $field:ident : Box<$expr_ty:ident>, $($rest:tt)*) => {
        define_operation!(
            @build_mapped
            [$($mapped_ctor)+]
            [$($out)* $field: Box::new($f(*$self.$field)),]
            $self,
            $f,
            $($rest)*
        )
    };
    (@build_mapped [$($mapped_ctor:tt)+] [$($out:tt)*] $self:ident, $f:ident, $field:ident : Box<$expr_ty:ident>) => {
        $($mapped_ctor)+ { _meta: $self._meta, $($out)* $field: Box::new($f(*$self.$field)), }
    };
    (@build_mapped [$($mapped_ctor:tt)+] [$($out:tt)*] $self:ident, $f:ident, $field:ident : $ty:ty, $($rest:tt)*) => {
        define_operation!(
            @build_mapped
            [$($mapped_ctor)+]
            [$($out)* $field: $self.$field,]
            $self,
            $f,
            $($rest)*
        )
    };
    (@build_mapped [$($mapped_ctor:tt)+] [$($out:tt)*] $self:ident, $f:ident, $field:ident : $ty:ty) => {
        $($mapped_ctor)+ { _meta: $self._meta, $($out)* $field: $self.$field, }
    };
    (@build_try_mapped [$($mapped_ctor:tt)+] [$($out:tt)*] $self:ident, $f:ident,) => {
        Ok($($mapped_ctor)+ { _meta: $self._meta, $($out)* })
    };
    (@build_try_mapped [$($mapped_ctor:tt)+] [$($out:tt)*] $self:ident, $f:ident, $field:ident : Box<$expr_ty:ident>, $($rest:tt)*) => {
        define_operation!(
            @build_try_mapped
            [$($mapped_ctor)+]
            [$($out)* $field: Box::new($f(*$self.$field)?),]
            $self,
            $f,
            $($rest)*
        )
    };
    (@build_try_mapped [$($mapped_ctor:tt)+] [$($out:tt)*] $self:ident, $f:ident, $field:ident : Box<$expr_ty:ident>) => {
        Ok($($mapped_ctor)+ { _meta: $self._meta, $($out)* $field: Box::new($f(*$self.$field)?), })
    };
    (@build_try_mapped [$($mapped_ctor:tt)+] [$($out:tt)*] $self:ident, $f:ident, $field:ident : $ty:ty, $($rest:tt)*) => {
        define_operation!(
            @build_try_mapped
            [$($mapped_ctor)+]
            [$($out)* $field: $self.$field,]
            $self,
            $f,
            $($rest)*
        )
    };
    (@build_try_mapped [$($mapped_ctor:tt)+] [$($out:tt)*] $self:ident, $f:ident, $field:ident : $ty:ty) => {
        Ok($($mapped_ctor)+ { _meta: $self._meta, $($out)* $field: $self.$field, })
    };
}

define_operation! {
    pub struct BinOp<E> {
        kind: BinOpKind,
        left: Box<E>,
        right: Box<E>,
    }
}

define_operation! {
    pub struct UnaryOp<E> {
        kind: UnaryOpKind,
        operand: Box<E>,
    }
}

define_operation! {
    pub struct InplaceBinOp<E> {
        kind: InplaceBinOpKind,
        left: Box<E>,
        right: Box<E>,
    }
}

define_operation! {
    pub struct TernaryOp<E> {
        kind: TernaryOpKind,
        base: Box<E>,
        exponent: Box<E>,
        modulus: Box<E>,
    }
}

define_operation! {
    pub struct GetAttr<E> {
        value: Box<E>,
        attr: String,
    }
}

define_operation! {
    pub struct SetAttr<E> {
        value: Box<E>,
        attr: String,
        replacement: Box<E>,
    }
}

define_operation! {
    pub struct GetItem<E> {
        value: Box<E>,
        index: Box<E>,
    }
}

define_operation! {
    pub struct SetItem<E> {
        value: Box<E>,
        index: Box<E>,
        replacement: Box<E>,
    }
}

define_operation! {
    pub struct DelItem<E> {
        value: Box<E>,
        index: Box<E>,
    }
}

define_operation! {
    pub struct LoadGlobal<E> {
        globals: Box<E>,
        name: String,
    }
}

define_operation! {
    pub struct StoreGlobal<E> {
        globals: Box<E>,
        name: String,
        value: Box<E>,
    }
}

define_operation! {
    pub struct LoadRuntime {
        name: String,
    }
}

define_operation! {
    pub struct LoadName {
        name: String,
    }
}

define_operation! {
    pub struct LoadLocal {
        location: LocalLocation,
    }
}

define_operation! {
    pub struct LoadCell {
        location: CellLocation,
    }
}

define_operation! {
    pub struct MakeCell<E> {
        initial_value: Box<E>,
    }
}

define_operation! {
    pub struct MakeString {
        bytes: Vec<u8>,
    }
}

define_operation! {
    pub struct CellRefForName {
        logical_name: String,
    }
}

define_operation! {
    pub struct CellRef {
        location: CellLocation,
    }
}

define_operation! {
    pub struct MakeFunction<E> {
        function_id: FunctionId,
        kind: BlockPyFunctionKind,
        param_defaults: Box<E>,
        annotate_fn: Box<E>,
    }
}

define_operation! {
    pub struct StoreCell<E> {
        location: CellLocation,
        value: Box<E>,
    }
}

define_operation! {
    pub struct DelQuietly<E> {
        value: Box<E>,
        name: String,
    }
}

define_operation! {
    pub struct DelDerefQuietly {
        location: CellLocation,
    }
}

define_operation! {
    pub struct DelDeref {
        location: CellLocation,
    }
}

#[derive(Debug, Clone, derive_more::From)]
pub enum OperationDetail<E> {
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
    LoadRuntime(LoadRuntime),
    LoadName(LoadName),
    LoadLocal(LoadLocal),
    LoadCell(LoadCell),
    MakeCell(MakeCell<E>),
    MakeString(MakeString),
    CellRefForName(CellRefForName),
    CellRef(CellRef),
    MakeFunction(MakeFunction<E>),
    StoreCell(StoreCell<E>),
    DelQuietly(DelQuietly<E>),
    DelDerefQuietly(DelDerefQuietly),
    DelDeref(DelDeref),
}

impl<E> OperationDetail<E> {
    pub fn map_expr<T>(self, f: &mut impl FnMut(E) -> T) -> OperationDetail<T> {
        match self {
            Self::BinOp(op) => OperationDetail::BinOp(op.map_op(f)),
            Self::UnaryOp(op) => OperationDetail::UnaryOp(op.map_op(f)),
            Self::InplaceBinOp(op) => OperationDetail::InplaceBinOp(op.map_op(f)),
            Self::TernaryOp(op) => OperationDetail::TernaryOp(op.map_op(f)),
            Self::GetAttr(op) => OperationDetail::GetAttr(op.map_op(f)),
            Self::SetAttr(op) => OperationDetail::SetAttr(op.map_op(f)),
            Self::GetItem(op) => OperationDetail::GetItem(op.map_op(f)),
            Self::SetItem(op) => OperationDetail::SetItem(op.map_op(f)),
            Self::DelItem(op) => OperationDetail::DelItem(op.map_op(f)),
            Self::LoadGlobal(op) => OperationDetail::LoadGlobal(op.map_op(f)),
            Self::StoreGlobal(op) => OperationDetail::StoreGlobal(op.map_op(f)),
            Self::LoadRuntime(op) => OperationDetail::LoadRuntime(op.map_op(f)),
            Self::LoadName(op) => OperationDetail::LoadName(op.map_op(f)),
            Self::LoadLocal(op) => OperationDetail::LoadLocal(op.map_op(f)),
            Self::LoadCell(op) => OperationDetail::LoadCell(op.map_op(f)),
            Self::MakeCell(op) => OperationDetail::MakeCell(op.map_op(f)),
            Self::MakeString(op) => OperationDetail::MakeString(op.map_op(f)),
            Self::CellRefForName(op) => OperationDetail::CellRefForName(op.map_op(f)),
            Self::CellRef(op) => OperationDetail::CellRef(op.map_op(f)),
            Self::MakeFunction(op) => OperationDetail::MakeFunction(op.map_op(f)),
            Self::StoreCell(op) => OperationDetail::StoreCell(op.map_op(f)),
            Self::DelQuietly(op) => OperationDetail::DelQuietly(op.map_op(f)),
            Self::DelDerefQuietly(op) => OperationDetail::DelDerefQuietly(op.map_op(f)),
            Self::DelDeref(op) => OperationDetail::DelDeref(op.map_op(f)),
        }
    }

    pub fn try_map_expr<T, Error>(
        self,
        f: &mut impl FnMut(E) -> Result<T, Error>,
    ) -> Result<OperationDetail<T>, Error> {
        Ok(match self {
            Self::BinOp(op) => OperationDetail::BinOp(op.try_map_op(f)?),
            Self::UnaryOp(op) => OperationDetail::UnaryOp(op.try_map_op(f)?),
            Self::InplaceBinOp(op) => OperationDetail::InplaceBinOp(op.try_map_op(f)?),
            Self::TernaryOp(op) => OperationDetail::TernaryOp(op.try_map_op(f)?),
            Self::GetAttr(op) => OperationDetail::GetAttr(op.try_map_op(f)?),
            Self::SetAttr(op) => OperationDetail::SetAttr(op.try_map_op(f)?),
            Self::GetItem(op) => OperationDetail::GetItem(op.try_map_op(f)?),
            Self::SetItem(op) => OperationDetail::SetItem(op.try_map_op(f)?),
            Self::DelItem(op) => OperationDetail::DelItem(op.try_map_op(f)?),
            Self::LoadGlobal(op) => OperationDetail::LoadGlobal(op.try_map_op(f)?),
            Self::StoreGlobal(op) => OperationDetail::StoreGlobal(op.try_map_op(f)?),
            Self::LoadRuntime(op) => OperationDetail::LoadRuntime(op.try_map_op(f)?),
            Self::LoadName(op) => OperationDetail::LoadName(op.try_map_op(f)?),
            Self::LoadLocal(op) => OperationDetail::LoadLocal(op.try_map_op(f)?),
            Self::LoadCell(op) => OperationDetail::LoadCell(op.try_map_op(f)?),
            Self::MakeCell(op) => OperationDetail::MakeCell(op.try_map_op(f)?),
            Self::MakeString(op) => OperationDetail::MakeString(op.try_map_op(f)?),
            Self::CellRefForName(op) => OperationDetail::CellRefForName(op.try_map_op(f)?),
            Self::CellRef(op) => OperationDetail::CellRef(op.try_map_op(f)?),
            Self::MakeFunction(op) => OperationDetail::MakeFunction(op.try_map_op(f)?),
            Self::StoreCell(op) => OperationDetail::StoreCell(op.try_map_op(f)?),
            Self::DelQuietly(op) => OperationDetail::DelQuietly(op.try_map_op(f)?),
            Self::DelDerefQuietly(op) => OperationDetail::DelDerefQuietly(op.try_map_op(f)?),
            Self::DelDeref(op) => OperationDetail::DelDeref(op.try_map_op(f)?),
        })
    }

    pub fn walk_args(&self, f: &mut impl FnMut(&E)) {
        match self {
            Self::BinOp(op) => op.visit_exprs(f),
            Self::UnaryOp(op) => op.visit_exprs(f),
            Self::InplaceBinOp(op) => op.visit_exprs(f),
            Self::TernaryOp(op) => op.visit_exprs(f),
            Self::GetAttr(op) => op.visit_exprs(f),
            Self::SetAttr(op) => op.visit_exprs(f),
            Self::GetItem(op) => op.visit_exprs(f),
            Self::SetItem(op) => op.visit_exprs(f),
            Self::DelItem(op) => op.visit_exprs(f),
            Self::LoadGlobal(op) => op.visit_exprs(f),
            Self::StoreGlobal(op) => op.visit_exprs(f),
            Self::LoadRuntime(op) => op.visit_exprs(f),
            Self::LoadName(op) => op.visit_exprs(f),
            Self::LoadLocal(op) => op.visit_exprs(f),
            Self::LoadCell(op) => op.visit_exprs(f),
            Self::MakeCell(op) => op.visit_exprs(f),
            Self::MakeString(op) => op.visit_exprs(f),
            Self::CellRefForName(op) => op.visit_exprs(f),
            Self::CellRef(op) => op.visit_exprs(f),
            Self::MakeFunction(op) => op.visit_exprs(f),
            Self::StoreCell(op) => op.visit_exprs(f),
            Self::DelQuietly(op) => op.visit_exprs(f),
            Self::DelDerefQuietly(op) => op.visit_exprs(f),
            Self::DelDeref(op) => op.visit_exprs(f),
        }
    }

    pub fn walk_args_mut(&mut self, f: &mut impl FnMut(&mut E)) {
        match self {
            Self::BinOp(op) => op.visit_exprs_mut(f),
            Self::UnaryOp(op) => op.visit_exprs_mut(f),
            Self::InplaceBinOp(op) => op.visit_exprs_mut(f),
            Self::TernaryOp(op) => op.visit_exprs_mut(f),
            Self::GetAttr(op) => op.visit_exprs_mut(f),
            Self::SetAttr(op) => op.visit_exprs_mut(f),
            Self::GetItem(op) => op.visit_exprs_mut(f),
            Self::SetItem(op) => op.visit_exprs_mut(f),
            Self::DelItem(op) => op.visit_exprs_mut(f),
            Self::LoadGlobal(op) => op.visit_exprs_mut(f),
            Self::StoreGlobal(op) => op.visit_exprs_mut(f),
            Self::LoadRuntime(op) => op.visit_exprs_mut(f),
            Self::LoadName(op) => op.visit_exprs_mut(f),
            Self::LoadLocal(op) => op.visit_exprs_mut(f),
            Self::LoadCell(op) => op.visit_exprs_mut(f),
            Self::MakeCell(op) => op.visit_exprs_mut(f),
            Self::MakeString(op) => op.visit_exprs_mut(f),
            Self::CellRefForName(op) => op.visit_exprs_mut(f),
            Self::CellRef(op) => op.visit_exprs_mut(f),
            Self::MakeFunction(op) => op.visit_exprs_mut(f),
            Self::StoreCell(op) => op.visit_exprs_mut(f),
            Self::DelQuietly(op) => op.visit_exprs_mut(f),
            Self::DelDerefQuietly(op) => op.visit_exprs_mut(f),
            Self::DelDeref(op) => op.visit_exprs_mut(f),
        }
    }
}

impl<E> HasMeta for OperationDetail<E> {
    fn meta(&self) -> Meta {
        match self {
            Self::BinOp(op) => op.meta(),
            Self::UnaryOp(op) => op.meta(),
            Self::InplaceBinOp(op) => op.meta(),
            Self::TernaryOp(op) => op.meta(),
            Self::GetAttr(op) => op.meta(),
            Self::SetAttr(op) => op.meta(),
            Self::GetItem(op) => op.meta(),
            Self::SetItem(op) => op.meta(),
            Self::DelItem(op) => op.meta(),
            Self::LoadGlobal(op) => op.meta(),
            Self::StoreGlobal(op) => op.meta(),
            Self::LoadRuntime(op) => op.meta(),
            Self::LoadName(op) => op.meta(),
            Self::LoadLocal(op) => op.meta(),
            Self::LoadCell(op) => op.meta(),
            Self::MakeCell(op) => op.meta(),
            Self::MakeString(op) => op.meta(),
            Self::CellRefForName(op) => op.meta(),
            Self::CellRef(op) => op.meta(),
            Self::MakeFunction(op) => op.meta(),
            Self::StoreCell(op) => op.meta(),
            Self::DelQuietly(op) => op.meta(),
            Self::DelDerefQuietly(op) => op.meta(),
            Self::DelDeref(op) => op.meta(),
        }
    }
}

impl<E> WithMeta for OperationDetail<E> {
    fn with_meta(mut self, meta: Meta) -> Self {
        match &mut self {
            Self::BinOp(op) => op._meta = meta,
            Self::UnaryOp(op) => op._meta = meta,
            Self::InplaceBinOp(op) => op._meta = meta,
            Self::TernaryOp(op) => op._meta = meta,
            Self::GetAttr(op) => op._meta = meta,
            Self::SetAttr(op) => op._meta = meta,
            Self::GetItem(op) => op._meta = meta,
            Self::SetItem(op) => op._meta = meta,
            Self::DelItem(op) => op._meta = meta,
            Self::LoadGlobal(op) => op._meta = meta,
            Self::StoreGlobal(op) => op._meta = meta,
            Self::LoadRuntime(op) => op._meta = meta,
            Self::LoadName(op) => op._meta = meta,
            Self::LoadLocal(op) => op._meta = meta,
            Self::LoadCell(op) => op._meta = meta,
            Self::MakeCell(op) => op._meta = meta,
            Self::MakeString(op) => op._meta = meta,
            Self::CellRefForName(op) => op._meta = meta,
            Self::CellRef(op) => op._meta = meta,
            Self::MakeFunction(op) => op._meta = meta,
            Self::StoreCell(op) => op._meta = meta,
            Self::DelQuietly(op) => op._meta = meta,
            Self::DelDerefQuietly(op) => op._meta = meta,
            Self::DelDeref(op) => op._meta = meta,
        }
        self
    }
}

#[derive(Debug, Clone)]
pub struct Operation<E> {
    pub detail: OperationDetail<E>,
}

impl<E> Operation<E> {
    pub fn new(detail: impl Into<OperationDetail<E>>) -> Self {
        Self {
            detail: detail.into(),
        }
    }

    pub fn detail(&self) -> &OperationDetail<E> {
        &self.detail
    }

    pub fn detail_mut(&mut self) -> &mut OperationDetail<E> {
        &mut self.detail
    }

    pub fn into_detail(self) -> OperationDetail<E> {
        self.detail
    }

    pub fn node_index(&self) -> ast::AtomicNodeIndex {
        self.detail.meta().node_index
    }

    pub fn range(&self) -> TextRange {
        self.detail.meta().range
    }

    pub fn map_expr<T>(self, f: &mut impl FnMut(E) -> T) -> Operation<T> {
        Operation {
            detail: self.detail.map_expr(f),
        }
    }

    pub fn try_map_expr<T, Error>(
        self,
        f: &mut impl FnMut(E) -> Result<T, Error>,
    ) -> Result<Operation<T>, Error> {
        Ok(Operation {
            detail: self.detail.try_map_expr(f)?,
        })
    }

    pub fn walk_args(&self, f: &mut impl FnMut(&E)) {
        self.detail.walk_args(f)
    }

    pub fn walk_args_mut(&mut self, f: &mut impl FnMut(&mut E)) {
        self.detail.walk_args_mut(f)
    }
}

impl<E> HasMeta for Operation<E> {
    fn meta(&self) -> Meta {
        self.detail.meta()
    }
}

impl<E> WithMeta for Operation<E> {
    fn with_meta(self, meta: Meta) -> Self {
        Self {
            detail: self.detail.with_meta(meta),
        }
    }
}

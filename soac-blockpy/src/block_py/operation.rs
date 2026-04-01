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

pub trait OperationNode<E>: Sized {
    type Name;
    type Mapped<T, M>;

    fn walk_expr_args(&self, f: &mut impl FnMut(&E));
    fn walk_expr_args_mut(&mut self, f: &mut impl FnMut(&mut E));
    fn into_expr_args(self) -> Vec<E>;
    fn map_expr_and_name<T, M>(
        self,
        f_expr: &mut impl FnMut(E) -> T,
        f_name: &mut impl FnMut(Self::Name) -> M,
    ) -> Self::Mapped<T, M>;
    fn try_map_expr_and_name<T, M, Error>(
        self,
        f_expr: &mut impl FnMut(E) -> Result<T, Error>,
        f_name: &mut impl FnMut(Self::Name) -> Result<M, Error>,
    ) -> Result<Self::Mapped<T, M>, Error>;

    fn map_expr<T>(self, f_expr: &mut impl FnMut(E) -> T) -> Self::Mapped<T, Self::Name> {
        self.map_expr_and_name(f_expr, &mut |name| name)
    }

    fn try_map_expr<T, Error>(
        self,
        f_expr: &mut impl FnMut(E) -> Result<T, Error>,
    ) -> Result<Self::Mapped<T, Self::Name>, Error> {
        self.try_map_expr_and_name(f_expr, &mut Ok)
    }
}

macro_rules! define_operation_node {
    (@build_mapped [$($mapped_ctor:tt)+] [$($out:tt)*] $self:ident, $f_expr:ident, $f_name:ident,) => {
        $($mapped_ctor)+ { $($out)* }
    };
    (@build_mapped [$($mapped_ctor:tt)+] [$($out:tt)*] $self:ident, $f_expr:ident, $f_name:ident, $field:ident : $ty:ty => expr, $($rest:tt)*) => {
        define_operation_node!(
            @build_mapped
            [$($mapped_ctor)+]
            [$($out)* $field: Box::new($f_expr(*$self.$field)),]
            $self,
            $f_expr,
            $f_name,
            $($rest)*
        )
    };
    (@build_mapped [$($mapped_ctor:tt)+] [$($out:tt)*] $self:ident, $f_expr:ident, $f_name:ident, $field:ident : $ty:ty => name, $($rest:tt)*) => {
        define_operation_node!(
            @build_mapped
            [$($mapped_ctor)+]
            [$($out)* $field: $f_name($self.$field),]
            $self,
            $f_expr,
            $f_name,
            $($rest)*
        )
    };
    (@build_mapped [$($mapped_ctor:tt)+] [$($out:tt)*] $self:ident, $f_expr:ident, $f_name:ident, $field:ident : $ty:ty => value, $($rest:tt)*) => {
        define_operation_node!(
            @build_mapped
            [$($mapped_ctor)+]
            [$($out)* $field: $self.$field,]
            $self,
            $f_expr,
            $f_name,
            $($rest)*
        )
    };
    (@build_try_mapped [$($mapped_ctor:tt)+] [$($out:tt)*] $self:ident, $f_expr:ident, $f_name:ident,) => {
        Ok($($mapped_ctor)+ { $($out)* })
    };
    (@build_try_mapped [$($mapped_ctor:tt)+] [$($out:tt)*] $self:ident, $f_expr:ident, $f_name:ident, $field:ident : $ty:ty => expr, $($rest:tt)*) => {
        define_operation_node!(
            @build_try_mapped
            [$($mapped_ctor)+]
            [$($out)* $field: Box::new($f_expr(*$self.$field)?),]
            $self,
            $f_expr,
            $f_name,
            $($rest)*
        )
    };
    (@build_try_mapped [$($mapped_ctor:tt)+] [$($out:tt)*] $self:ident, $f_expr:ident, $f_name:ident, $field:ident : $ty:ty => name, $($rest:tt)*) => {
        define_operation_node!(
            @build_try_mapped
            [$($mapped_ctor)+]
            [$($out)* $field: $f_name($self.$field)?,]
            $self,
            $f_expr,
            $f_name,
            $($rest)*
        )
    };
    (@build_try_mapped [$($mapped_ctor:tt)+] [$($out:tt)*] $self:ident, $f_expr:ident, $f_name:ident, $field:ident : $ty:ty => value, $($rest:tt)*) => {
        define_operation_node!(
            @build_try_mapped
            [$($mapped_ctor)+]
            [$($out)* $field: $self.$field,]
            $self,
            $f_expr,
            $f_name,
            $($rest)*
        )
    };
    (@walk_expr_fields $self:ident, $f:ident,) => {};
    (@walk_expr_fields $self:ident, $f:ident, $field:ident : $ty:ty => expr, $($rest:tt)*) => {
        $f(&$self.$field);
        define_operation_node!(@walk_expr_fields $self, $f, $($rest)*);
    };
    (@walk_expr_fields $self:ident, $f:ident, $field:ident : $ty:ty => name, $($rest:tt)*) => {
        define_operation_node!(@walk_expr_fields $self, $f, $($rest)*);
    };
    (@walk_expr_fields $self:ident, $f:ident, $field:ident : $ty:ty => value, $($rest:tt)*) => {
        define_operation_node!(@walk_expr_fields $self, $f, $($rest)*);
    };
    (@walk_expr_fields_mut $self:ident, $f:ident,) => {};
    (@walk_expr_fields_mut $self:ident, $f:ident, $field:ident : $ty:ty => expr, $($rest:tt)*) => {
        $f(&mut $self.$field);
        define_operation_node!(@walk_expr_fields_mut $self, $f, $($rest)*);
    };
    (@walk_expr_fields_mut $self:ident, $f:ident, $field:ident : $ty:ty => name, $($rest:tt)*) => {
        define_operation_node!(@walk_expr_fields_mut $self, $f, $($rest)*);
    };
    (@walk_expr_fields_mut $self:ident, $f:ident, $field:ident : $ty:ty => value, $($rest:tt)*) => {
        define_operation_node!(@walk_expr_fields_mut $self, $f, $($rest)*);
    };
    (@into_expr_fields $out:ident, $self:ident,) => {};
    (@into_expr_fields $out:ident, $self:ident, $field:ident : $ty:ty => expr, $($rest:tt)*) => {
        $out.push(*$self.$field);
        define_operation_node!(@into_expr_fields $out, $self, $($rest)*);
    };
    (@into_expr_fields $out:ident, $self:ident, $field:ident : $ty:ty => name, $($rest:tt)*) => {
        define_operation_node!(@into_expr_fields $out, $self, $($rest)*);
    };
    (@into_expr_fields $out:ident, $self:ident, $field:ident : $ty:ty => value, $($rest:tt)*) => {
        define_operation_node!(@into_expr_fields $out, $self, $($rest)*);
    };
    (
        $vis:vis struct $name:ident $(<$($struct_gen:ident),*>)? {
            impl<$($impl_gen:ident),*>;
            name_type = [$($name_ty:tt)+];
            mapped_type<$mapped_expr:ident, $mapped_name:ident> = [$($mapped_ty:tt)+];
            mapped_ctor<$mapped_ctor_expr:ident, $mapped_ctor_name:ident> = [$($mapped_ctor:tt)+];
            $( $field:ident : $ty:ty => $kind:ident ),* $(,)?
        }
    ) => {
        #[derive(Debug, Clone)]
        $vis struct $name $(<$($struct_gen),*>)? {
            $(pub $field: $ty,)*
        }

        impl$(<$($struct_gen),*>)? $name $(<$($struct_gen),*>)? {
            pub fn new($($field: $ty),*) -> Self {
                Self { $($field,)* }
            }
        }

        impl<$($impl_gen),*> OperationNode<E> for $name $(<$($struct_gen),*>)? {
            type Name = $($name_ty)+;
            type Mapped<$mapped_expr, $mapped_name> = $($mapped_ty)+;

            fn walk_expr_args(&self, f: &mut impl FnMut(&E)) {
                #[allow(unused_variables)]
                let _ = &f;
                define_operation_node!(@walk_expr_fields self, f, $($field : $ty => $kind,)*);
            }

            fn walk_expr_args_mut(&mut self, f: &mut impl FnMut(&mut E)) {
                #[allow(unused_variables)]
                let _ = &f;
                define_operation_node!(@walk_expr_fields_mut self, f, $($field : $ty => $kind,)*);
            }

            fn into_expr_args(self) -> Vec<E> {
                #[allow(unused_mut)]
                let mut out = Vec::new();
                define_operation_node!(@into_expr_fields out, self, $($field : $ty => $kind,)*);
                out
            }

            fn map_expr_and_name<T, M>(
                self,
                f_expr: &mut impl FnMut(E) -> T,
                f_name: &mut impl FnMut(Self::Name) -> M,
            ) -> Self::Mapped<T, M> {
                #[allow(unused_variables)]
                let _ = (&f_expr, &f_name);
                define_operation_node!(
                    @build_mapped
                    [$($mapped_ctor)+]
                    []
                    self,
                    f_expr,
                    f_name,
                    $($field : $ty => $kind,)*
                )
            }

            fn try_map_expr_and_name<T, M, Error>(
                self,
                f_expr: &mut impl FnMut(E) -> Result<T, Error>,
                f_name: &mut impl FnMut(Self::Name) -> Result<M, Error>,
            ) -> Result<Self::Mapped<T, M>, Error> {
                #[allow(unused_variables)]
                let _ = (&f_expr, &f_name);
                define_operation_node!(
                    @build_try_mapped
                    [$($mapped_ctor)+]
                    []
                    self,
                    f_expr,
                    f_name,
                    $($field : $ty => $kind,)*
                )
            }
        }

    };
}

define_operation_node! {
    pub struct BinOp<E> {
        impl<E>;
        name_type = [()];
        mapped_type<T, M> = [BinOp<T>];
        mapped_ctor<T, M> = [BinOp::<T>];
        kind: BinOpKind => value,
        left: Box<E> => expr,
        right: Box<E> => expr,
    }
}

define_operation_node! {
    pub struct UnaryOp<E> {
        impl<E>;
        name_type = [()];
        mapped_type<T, M> = [UnaryOp<T>];
        mapped_ctor<T, M> = [UnaryOp::<T>];
        kind: UnaryOpKind => value,
        operand: Box<E> => expr,
    }
}

define_operation_node! {
    pub struct InplaceBinOp<E> {
        impl<E>;
        name_type = [()];
        mapped_type<T, M> = [InplaceBinOp<T>];
        mapped_ctor<T, M> = [InplaceBinOp::<T>];
        kind: InplaceBinOpKind => value,
        left: Box<E> => expr,
        right: Box<E> => expr,
    }
}

define_operation_node! {
    pub struct TernaryOp<E> {
        impl<E>;
        name_type = [()];
        mapped_type<T, M> = [TernaryOp<T>];
        mapped_ctor<T, M> = [TernaryOp::<T>];
        kind: TernaryOpKind => value,
        base: Box<E> => expr,
        exponent: Box<E> => expr,
        modulus: Box<E> => expr,
    }
}

define_operation_node! {
    pub struct GetAttr<E> {
        impl<E>;
        name_type = [()];
        mapped_type<T, M> = [GetAttr<T>];
        mapped_ctor<T, M> = [GetAttr::<T>];
        value: Box<E> => expr,
        attr: String => value,
    }
}

define_operation_node! {
    pub struct SetAttr<E> {
        impl<E>;
        name_type = [()];
        mapped_type<T, M> = [SetAttr<T>];
        mapped_ctor<T, M> = [SetAttr::<T>];
        value: Box<E> => expr,
        attr: String => value,
        replacement: Box<E> => expr,
    }
}

define_operation_node! {
    pub struct GetItem<E> {
        impl<E>;
        name_type = [()];
        mapped_type<T, M> = [GetItem<T>];
        mapped_ctor<T, M> = [GetItem::<T>];
        value: Box<E> => expr,
        index: Box<E> => expr,
    }
}

define_operation_node! {
    pub struct SetItem<E> {
        impl<E>;
        name_type = [()];
        mapped_type<T, M> = [SetItem<T>];
        mapped_ctor<T, M> = [SetItem::<T>];
        value: Box<E> => expr,
        index: Box<E> => expr,
        replacement: Box<E> => expr,
    }
}

define_operation_node! {
    pub struct DelItem<E> {
        impl<E>;
        name_type = [()];
        mapped_type<T, M> = [DelItem<T>];
        mapped_ctor<T, M> = [DelItem::<T>];
        value: Box<E> => expr,
        index: Box<E> => expr,
    }
}

define_operation_node! {
    pub struct LoadGlobal<E> {
        impl<E>;
        name_type = [()];
        mapped_type<T, M> = [LoadGlobal<T>];
        mapped_ctor<T, M> = [LoadGlobal::<T>];
        globals: Box<E> => expr,
        name: String => value,
    }
}

define_operation_node! {
    pub struct StoreGlobal<E> {
        impl<E>;
        name_type = [()];
        mapped_type<T, M> = [StoreGlobal<T>];
        mapped_ctor<T, M> = [StoreGlobal::<T>];
        globals: Box<E> => expr,
        name: String => value,
        value: Box<E> => expr,
    }
}

define_operation_node! {
    pub struct LoadRuntime {
        impl<E>;
        name_type = [()];
        mapped_type<T, M> = [LoadRuntime];
        mapped_ctor<T, M> = [LoadRuntime];
        name: String => value,
    }
}

define_operation_node! {
    pub struct LoadName {
        impl<E>;
        name_type = [()];
        mapped_type<T, M> = [LoadName];
        mapped_ctor<T, M> = [LoadName];
        name: String => value,
    }
}

define_operation_node! {
    pub struct LoadLocal {
        impl<E>;
        name_type = [()];
        mapped_type<T, M> = [LoadLocal];
        mapped_ctor<T, M> = [LoadLocal];
        location: LocalLocation => value,
    }
}

define_operation_node! {
    pub struct LoadCell {
        impl<E>;
        name_type = [()];
        mapped_type<T, M> = [LoadCell];
        mapped_ctor<T, M> = [LoadCell];
        location: CellLocation => value,
    }
}

define_operation_node! {
    pub struct MakeCell<E> {
        impl<E>;
        name_type = [()];
        mapped_type<T, M> = [MakeCell<T>];
        mapped_ctor<T, M> = [MakeCell::<T>];
        initial_value: Box<E> => expr,
    }
}

define_operation_node! {
    pub struct MakeString {
        impl<E>;
        name_type = [()];
        mapped_type<T, M> = [MakeString];
        mapped_ctor<T, M> = [MakeString];
        bytes: Vec<u8> => value,
    }
}

define_operation_node! {
    pub struct CellRefForName {
        impl<E>;
        name_type = [()];
        mapped_type<T, M> = [CellRefForName];
        mapped_ctor<T, M> = [CellRefForName];
        logical_name: String => value,
    }
}

define_operation_node! {
    pub struct CellRef {
        impl<E>;
        name_type = [()];
        mapped_type<T, M> = [CellRef];
        mapped_ctor<T, M> = [CellRef];
        location: CellLocation => value,
    }
}

define_operation_node! {
    pub struct MakeFunction<E> {
        impl<E>;
        name_type = [()];
        mapped_type<T, M> = [MakeFunction<T>];
        mapped_ctor<T, M> = [MakeFunction::<T>];
        function_id: FunctionId => value,
        kind: BlockPyFunctionKind => value,
        param_defaults: Box<E> => expr,
        annotate_fn: Box<E> => expr,
    }
}

define_operation_node! {
    pub struct StoreCell<E> {
        impl<E>;
        name_type = [()];
        mapped_type<T, M> = [StoreCell<T>];
        mapped_ctor<T, M> = [StoreCell::<T>];
        location: CellLocation => value,
        value: Box<E> => expr,
    }
}

define_operation_node! {
    pub struct DelQuietly<E> {
        impl<E>;
        name_type = [()];
        mapped_type<T, M> = [DelQuietly<T>];
        mapped_ctor<T, M> = [DelQuietly::<T>];
        value: Box<E> => expr,
        name: String => value,
    }
}

define_operation_node! {
    pub struct DelDerefQuietly {
        impl<E>;
        name_type = [()];
        mapped_type<T, M> = [DelDerefQuietly];
        mapped_ctor<T, M> = [DelDerefQuietly];
        location: CellLocation => value,
    }
}

define_operation_node! {
    pub struct DelDeref {
        impl<E>;
        name_type = [()];
        mapped_type<T, M> = [DelDeref];
        mapped_ctor<T, M> = [DelDeref];
        location: CellLocation => value,
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
            Self::BinOp(op) => OperationDetail::BinOp(op.map_expr(f)),
            Self::UnaryOp(op) => OperationDetail::UnaryOp(op.map_expr(f)),
            Self::InplaceBinOp(op) => OperationDetail::InplaceBinOp(op.map_expr(f)),
            Self::TernaryOp(op) => OperationDetail::TernaryOp(op.map_expr(f)),
            Self::GetAttr(op) => OperationDetail::GetAttr(op.map_expr(f)),
            Self::SetAttr(op) => OperationDetail::SetAttr(op.map_expr(f)),
            Self::GetItem(op) => OperationDetail::GetItem(op.map_expr(f)),
            Self::SetItem(op) => OperationDetail::SetItem(op.map_expr(f)),
            Self::DelItem(op) => OperationDetail::DelItem(op.map_expr(f)),
            Self::LoadGlobal(op) => OperationDetail::LoadGlobal(op.map_expr(f)),
            Self::StoreGlobal(op) => OperationDetail::StoreGlobal(op.map_expr(f)),
            Self::LoadRuntime(op) => OperationDetail::LoadRuntime(op.map_expr(f)),
            Self::LoadName(op) => OperationDetail::LoadName(op.map_expr(f)),
            Self::LoadLocal(op) => OperationDetail::LoadLocal(op.map_expr(f)),
            Self::LoadCell(op) => OperationDetail::LoadCell(op.map_expr(f)),
            Self::MakeCell(op) => OperationDetail::MakeCell(op.map_expr(f)),
            Self::MakeString(op) => OperationDetail::MakeString(op.map_expr(f)),
            Self::CellRefForName(op) => OperationDetail::CellRefForName(op.map_expr(f)),
            Self::CellRef(op) => OperationDetail::CellRef(op.map_expr(f)),
            Self::MakeFunction(op) => OperationDetail::MakeFunction(op.map_expr(f)),
            Self::StoreCell(op) => OperationDetail::StoreCell(op.map_expr(f)),
            Self::DelQuietly(op) => OperationDetail::DelQuietly(op.map_expr(f)),
            Self::DelDerefQuietly(op) => OperationDetail::DelDerefQuietly(op.map_expr(f)),
            Self::DelDeref(op) => OperationDetail::DelDeref(op.map_expr(f)),
        }
    }

    pub fn try_map_expr<T, Error>(
        self,
        f: &mut impl FnMut(E) -> Result<T, Error>,
    ) -> Result<OperationDetail<T>, Error> {
        Ok(match self {
            Self::BinOp(op) => OperationDetail::BinOp(op.try_map_expr(f)?),
            Self::UnaryOp(op) => OperationDetail::UnaryOp(op.try_map_expr(f)?),
            Self::InplaceBinOp(op) => OperationDetail::InplaceBinOp(op.try_map_expr(f)?),
            Self::TernaryOp(op) => OperationDetail::TernaryOp(op.try_map_expr(f)?),
            Self::GetAttr(op) => OperationDetail::GetAttr(op.try_map_expr(f)?),
            Self::SetAttr(op) => OperationDetail::SetAttr(op.try_map_expr(f)?),
            Self::GetItem(op) => OperationDetail::GetItem(op.try_map_expr(f)?),
            Self::SetItem(op) => OperationDetail::SetItem(op.try_map_expr(f)?),
            Self::DelItem(op) => OperationDetail::DelItem(op.try_map_expr(f)?),
            Self::LoadGlobal(op) => OperationDetail::LoadGlobal(op.try_map_expr(f)?),
            Self::StoreGlobal(op) => OperationDetail::StoreGlobal(op.try_map_expr(f)?),
            Self::LoadRuntime(op) => OperationDetail::LoadRuntime(op.try_map_expr(f)?),
            Self::LoadName(op) => OperationDetail::LoadName(op.try_map_expr(f)?),
            Self::LoadLocal(op) => OperationDetail::LoadLocal(op.try_map_expr(f)?),
            Self::LoadCell(op) => OperationDetail::LoadCell(op.try_map_expr(f)?),
            Self::MakeCell(op) => OperationDetail::MakeCell(op.try_map_expr(f)?),
            Self::MakeString(op) => OperationDetail::MakeString(op.try_map_expr(f)?),
            Self::CellRefForName(op) => OperationDetail::CellRefForName(op.try_map_expr(f)?),
            Self::CellRef(op) => OperationDetail::CellRef(op.try_map_expr(f)?),
            Self::MakeFunction(op) => OperationDetail::MakeFunction(op.try_map_expr(f)?),
            Self::StoreCell(op) => OperationDetail::StoreCell(op.try_map_expr(f)?),
            Self::DelQuietly(op) => OperationDetail::DelQuietly(op.try_map_expr(f)?),
            Self::DelDerefQuietly(op) => OperationDetail::DelDerefQuietly(op.try_map_expr(f)?),
            Self::DelDeref(op) => OperationDetail::DelDeref(op.try_map_expr(f)?),
        })
    }

    pub fn walk_args(&self, f: &mut impl FnMut(&E)) {
        match self {
            Self::BinOp(op) => op.walk_expr_args(f),
            Self::UnaryOp(op) => op.walk_expr_args(f),
            Self::InplaceBinOp(op) => op.walk_expr_args(f),
            Self::TernaryOp(op) => op.walk_expr_args(f),
            Self::GetAttr(op) => op.walk_expr_args(f),
            Self::SetAttr(op) => op.walk_expr_args(f),
            Self::GetItem(op) => op.walk_expr_args(f),
            Self::SetItem(op) => op.walk_expr_args(f),
            Self::DelItem(op) => op.walk_expr_args(f),
            Self::LoadGlobal(op) => op.walk_expr_args(f),
            Self::StoreGlobal(op) => op.walk_expr_args(f),
            Self::LoadRuntime(op) => op.walk_expr_args(f),
            Self::LoadName(op) => op.walk_expr_args(f),
            Self::LoadLocal(op) => op.walk_expr_args(f),
            Self::LoadCell(op) => op.walk_expr_args(f),
            Self::MakeCell(op) => op.walk_expr_args(f),
            Self::MakeString(op) => op.walk_expr_args(f),
            Self::CellRefForName(op) => op.walk_expr_args(f),
            Self::CellRef(op) => op.walk_expr_args(f),
            Self::MakeFunction(op) => op.walk_expr_args(f),
            Self::StoreCell(op) => op.walk_expr_args(f),
            Self::DelQuietly(op) => op.walk_expr_args(f),
            Self::DelDerefQuietly(op) => op.walk_expr_args(f),
            Self::DelDeref(op) => op.walk_expr_args(f),
        }
    }

    pub fn walk_args_mut(&mut self, f: &mut impl FnMut(&mut E)) {
        match self {
            Self::BinOp(op) => op.walk_expr_args_mut(f),
            Self::UnaryOp(op) => op.walk_expr_args_mut(f),
            Self::InplaceBinOp(op) => op.walk_expr_args_mut(f),
            Self::TernaryOp(op) => op.walk_expr_args_mut(f),
            Self::GetAttr(op) => op.walk_expr_args_mut(f),
            Self::SetAttr(op) => op.walk_expr_args_mut(f),
            Self::GetItem(op) => op.walk_expr_args_mut(f),
            Self::SetItem(op) => op.walk_expr_args_mut(f),
            Self::DelItem(op) => op.walk_expr_args_mut(f),
            Self::LoadGlobal(op) => op.walk_expr_args_mut(f),
            Self::StoreGlobal(op) => op.walk_expr_args_mut(f),
            Self::LoadRuntime(op) => op.walk_expr_args_mut(f),
            Self::LoadName(op) => op.walk_expr_args_mut(f),
            Self::LoadLocal(op) => op.walk_expr_args_mut(f),
            Self::LoadCell(op) => op.walk_expr_args_mut(f),
            Self::MakeCell(op) => op.walk_expr_args_mut(f),
            Self::MakeString(op) => op.walk_expr_args_mut(f),
            Self::CellRefForName(op) => op.walk_expr_args_mut(f),
            Self::CellRef(op) => op.walk_expr_args_mut(f),
            Self::MakeFunction(op) => op.walk_expr_args_mut(f),
            Self::StoreCell(op) => op.walk_expr_args_mut(f),
            Self::DelQuietly(op) => op.walk_expr_args_mut(f),
            Self::DelDerefQuietly(op) => op.walk_expr_args_mut(f),
            Self::DelDeref(op) => op.walk_expr_args_mut(f),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Operation<E> {
    pub meta: Meta,
    pub detail: OperationDetail<E>,
}

impl<E> Operation<E> {
    pub fn new(detail: impl Into<OperationDetail<E>>) -> Self {
        Self {
            meta: Meta::synthetic(),
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

    pub fn node_index(&self) -> &ast::AtomicNodeIndex {
        &self.meta.node_index
    }

    pub fn range(&self) -> TextRange {
        self.meta.range
    }

    pub fn map_expr<T>(self, f: &mut impl FnMut(E) -> T) -> Operation<T> {
        Operation {
            meta: self.meta,
            detail: self.detail.map_expr(f),
        }
    }

    pub fn try_map_expr<T, Error>(
        self,
        f: &mut impl FnMut(E) -> Result<T, Error>,
    ) -> Result<Operation<T>, Error> {
        Ok(Operation {
            meta: self.meta,
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

impl<E> WithMeta for Operation<E> {
    fn with_meta(mut self, meta: Meta) -> Self {
        self.meta = meta;
        self
    }
}

impl<E> HasMeta for Operation<E> {
    fn meta(&self) -> Meta {
        self.meta.clone()
    }
}

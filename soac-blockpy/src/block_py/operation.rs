use super::{BlockPyFunctionKind, FunctionId, HasMeta, Meta, WithMeta};
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
    (@build_mapped [$($mapped_ctor:tt)+] [$($out:tt)*] $self:ident, $f_expr:ident, $f_name:ident, $field:ident : $ty:ty => name_target, $($rest:tt)*) => {
        define_operation_node!(
            @build_mapped
            [$($mapped_ctor)+]
            [$($out)* $field: $self.$field.map_name($f_name),]
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
    (@build_try_mapped [$($mapped_ctor:tt)+] [$($out:tt)*] $self:ident, $f_expr:ident, $f_name:ident, $field:ident : $ty:ty => name_target, $($rest:tt)*) => {
        define_operation_node!(
            @build_try_mapped
            [$($mapped_ctor)+]
            [$($out)*
                $field: match $self.$field {
                    CellRefTarget::LogicalName(name) => CellRefTarget::LogicalName(name),
                    CellRefTarget::Name(name) => CellRefTarget::Name($f_name(name)?),
                },
            ]
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
    (@walk_expr_fields $self:ident, $f:ident, $field:ident : $ty:ty => name_target, $($rest:tt)*) => {
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
    (@walk_expr_fields_mut $self:ident, $f:ident, $field:ident : $ty:ty => name_target, $($rest:tt)*) => {
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
    (@into_expr_fields $out:ident, $self:ident, $field:ident : $ty:ty => name_target, $($rest:tt)*) => {
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

#[derive(Debug, Clone)]
pub enum CellRefTarget<N> {
    LogicalName(String),
    Name(N),
}

impl<N> CellRefTarget<N> {
    pub fn map_name<T>(self, f: &mut impl FnMut(N) -> T) -> CellRefTarget<T> {
        match self {
            Self::LogicalName(name) => CellRefTarget::LogicalName(name),
            Self::Name(name) => CellRefTarget::Name(f(name)),
        }
    }
}

define_operation_node! {
    pub struct BinOp<E> {
        impl<E>;
        name_type = [()];
        mapped_type<T, M> = [BinOp<T>];
        mapped_ctor<T, M> = [BinOp::<T>];
        kind: BinOpKind => value,
        arg0: Box<E> => expr,
        arg1: Box<E> => expr,
    }
}

define_operation_node! {
    pub struct UnaryOp<E> {
        impl<E>;
        name_type = [()];
        mapped_type<T, M> = [UnaryOp<T>];
        mapped_ctor<T, M> = [UnaryOp::<T>];
        kind: UnaryOpKind => value,
        arg0: Box<E> => expr,
    }
}

define_operation_node! {
    pub struct InplaceBinOp<E> {
        impl<E>;
        name_type = [()];
        mapped_type<T, M> = [InplaceBinOp<T>];
        mapped_ctor<T, M> = [InplaceBinOp::<T>];
        kind: InplaceBinOpKind => value,
        arg0: Box<E> => expr,
        arg1: Box<E> => expr,
    }
}

define_operation_node! {
    pub struct TernaryOp<E> {
        impl<E>;
        name_type = [()];
        mapped_type<T, M> = [TernaryOp<T>];
        mapped_ctor<T, M> = [TernaryOp::<T>];
        kind: TernaryOpKind => value,
        arg0: Box<E> => expr,
        arg1: Box<E> => expr,
        arg2: Box<E> => expr,
    }
}

define_operation_node! {
    pub struct GetAttr<E> {
        impl<E>;
        name_type = [()];
        mapped_type<T, M> = [GetAttr<T>];
        mapped_ctor<T, M> = [GetAttr::<T>];
        arg0: Box<E> => expr,
        arg1: String => value,
    }
}

define_operation_node! {
    pub struct SetAttr<E> {
        impl<E>;
        name_type = [()];
        mapped_type<T, M> = [SetAttr<T>];
        mapped_ctor<T, M> = [SetAttr::<T>];
        arg0: Box<E> => expr,
        arg1: String => value,
        arg2: Box<E> => expr,
    }
}

define_operation_node! {
    pub struct GetItem<E> {
        impl<E>;
        name_type = [()];
        mapped_type<T, M> = [GetItem<T>];
        mapped_ctor<T, M> = [GetItem::<T>];
        arg0: Box<E> => expr,
        arg1: Box<E> => expr,
    }
}

define_operation_node! {
    pub struct SetItem<E> {
        impl<E>;
        name_type = [()];
        mapped_type<T, M> = [SetItem<T>];
        mapped_ctor<T, M> = [SetItem::<T>];
        arg0: Box<E> => expr,
        arg1: Box<E> => expr,
        arg2: Box<E> => expr,
    }
}

define_operation_node! {
    pub struct DelItem<E> {
        impl<E>;
        name_type = [()];
        mapped_type<T, M> = [DelItem<T>];
        mapped_ctor<T, M> = [DelItem::<T>];
        arg0: Box<E> => expr,
        arg1: Box<E> => expr,
    }
}

define_operation_node! {
    pub struct LoadGlobal<E> {
        impl<E>;
        name_type = [()];
        mapped_type<T, M> = [LoadGlobal<T>];
        mapped_ctor<T, M> = [LoadGlobal::<T>];
        arg0: Box<E> => expr,
        arg1: String => value,
    }
}

define_operation_node! {
    pub struct StoreGlobal<E> {
        impl<E>;
        name_type = [()];
        mapped_type<T, M> = [StoreGlobal<T>];
        mapped_ctor<T, M> = [StoreGlobal::<T>];
        arg0: Box<E> => expr,
        arg1: String => value,
        arg2: Box<E> => expr,
    }
}

define_operation_node! {
    pub struct LoadName<N> {
        impl<E, N>;
        name_type = [N];
        mapped_type<T, M> = [LoadName<M>];
        mapped_ctor<T, M> = [LoadName::<M>];
        arg0: N => name,
    }
}

define_operation_node! {
    pub struct LoadLocal<N> {
        impl<E, N>;
        name_type = [N];
        mapped_type<T, M> = [LoadLocal<M>];
        mapped_ctor<T, M> = [LoadLocal::<M>];
        arg0: N => name,
    }
}

define_operation_node! {
    pub struct LoadCell<N> {
        impl<E, N>;
        name_type = [N];
        mapped_type<T, M> = [LoadCell<M>];
        mapped_ctor<T, M> = [LoadCell::<M>];
        arg0: N => name,
    }
}

define_operation_node! {
    pub struct MakeCell<E> {
        impl<E>;
        name_type = [()];
        mapped_type<T, M> = [MakeCell<T>];
        mapped_ctor<T, M> = [MakeCell::<T>];
        arg0: Box<E> => expr,
    }
}

define_operation_node! {
    pub struct MakeString {
        impl<E>;
        name_type = [()];
        mapped_type<T, M> = [MakeString];
        mapped_ctor<T, M> = [MakeString];
        arg0: Vec<u8> => value,
    }
}

define_operation_node! {
    pub struct CellRef<N> {
        impl<E, N>;
        name_type = [N];
        mapped_type<T, M> = [CellRef<M>];
        mapped_ctor<T, M> = [CellRef::<M>];
        arg0: CellRefTarget<N> => name_target,
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
        arg0: Box<E> => expr,
        arg1: Box<E> => expr,
        arg2: Box<E> => expr,
    }
}

define_operation_node! {
    pub struct StoreCell<N, E> {
        impl<E, N>;
        name_type = [N];
        mapped_type<T, M> = [StoreCell<M, T>];
        mapped_ctor<T, M> = [StoreCell::<M, T>];
        arg0: N => name,
        arg1: Box<E> => expr,
    }
}

define_operation_node! {
    pub struct DelQuietly<E> {
        impl<E>;
        name_type = [()];
        mapped_type<T, M> = [DelQuietly<T>];
        mapped_ctor<T, M> = [DelQuietly::<T>];
        arg0: Box<E> => expr,
        arg1: String => value,
    }
}

define_operation_node! {
    pub struct DelDerefQuietly<N> {
        impl<E, N>;
        name_type = [N];
        mapped_type<T, M> = [DelDerefQuietly<M>];
        mapped_ctor<T, M> = [DelDerefQuietly::<M>];
        arg0: N => name,
    }
}

define_operation_node! {
    pub struct DelDeref<N> {
        impl<E, N>;
        name_type = [N];
        mapped_type<T, M> = [DelDeref<M>];
        mapped_ctor<T, M> = [DelDeref::<M>];
        arg0: N => name,
    }
}

#[derive(Debug, Clone)]
pub enum OperationDetail<E, N = E> {
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
    LoadName(LoadName<N>),
    LoadLocal(LoadLocal<N>),
    LoadCell(LoadCell<N>),
    MakeCell(MakeCell<E>),
    MakeString(MakeString),
    CellRef(CellRef<N>),
    MakeFunction(MakeFunction<E>),
    StoreCell(StoreCell<N, E>),
    DelQuietly(DelQuietly<E>),
    DelDerefQuietly(DelDerefQuietly<N>),
    DelDeref(DelDeref<N>),
}

macro_rules! impl_expr_operation_detail_from {
    ($($name:ident),* $(,)?) => {
        $(
            impl<E, N> From<$name<E>> for OperationDetail<E, N> {
                fn from(value: $name<E>) -> Self {
                    Self::$name(value)
                }
            }
        )*
    };
}

impl_expr_operation_detail_from!(
    BinOp,
    UnaryOp,
    InplaceBinOp,
    TernaryOp,
    GetAttr,
    SetAttr,
    GetItem,
    SetItem,
    DelItem,
    LoadGlobal,
    StoreGlobal,
    MakeCell,
    MakeFunction,
    DelQuietly,
);

impl<E, N> From<LoadCell<N>> for OperationDetail<E, N> {
    fn from(value: LoadCell<N>) -> Self {
        Self::LoadCell(value)
    }
}

impl<E, N> From<LoadName<N>> for OperationDetail<E, N> {
    fn from(value: LoadName<N>) -> Self {
        Self::LoadName(value)
    }
}

impl<E, N> From<LoadLocal<N>> for OperationDetail<E, N> {
    fn from(value: LoadLocal<N>) -> Self {
        Self::LoadLocal(value)
    }
}

impl<E, N> From<MakeString> for OperationDetail<E, N> {
    fn from(value: MakeString) -> Self {
        Self::MakeString(value)
    }
}

impl<E, N> From<CellRef<N>> for OperationDetail<E, N> {
    fn from(value: CellRef<N>) -> Self {
        Self::CellRef(value)
    }
}

impl<E, N> From<StoreCell<N, E>> for OperationDetail<E, N> {
    fn from(value: StoreCell<N, E>) -> Self {
        Self::StoreCell(value)
    }
}

impl<E, N> From<DelDerefQuietly<N>> for OperationDetail<E, N> {
    fn from(value: DelDerefQuietly<N>) -> Self {
        Self::DelDerefQuietly(value)
    }
}

impl<E, N> From<DelDeref<N>> for OperationDetail<E, N> {
    fn from(value: DelDeref<N>) -> Self {
        Self::DelDeref(value)
    }
}

impl<E, N> OperationDetail<E, N> {
    pub fn map_expr<T>(self, f: &mut impl FnMut(E) -> T) -> OperationDetail<T, N> {
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
            Self::LoadName(op) => OperationDetail::LoadName(op.map_expr(f)),
            Self::LoadLocal(op) => OperationDetail::LoadLocal(op.map_expr(f)),
            Self::LoadCell(op) => OperationDetail::LoadCell(op.map_expr(f)),
            Self::MakeCell(op) => OperationDetail::MakeCell(op.map_expr(f)),
            Self::MakeString(op) => OperationDetail::MakeString(op.map_expr(f)),
            Self::CellRef(op) => OperationDetail::CellRef(op.map_expr(f)),
            Self::MakeFunction(op) => OperationDetail::MakeFunction(op.map_expr(f)),
            Self::StoreCell(op) => OperationDetail::StoreCell(op.map_expr(f)),
            Self::DelQuietly(op) => OperationDetail::DelQuietly(op.map_expr(f)),
            Self::DelDerefQuietly(op) => OperationDetail::DelDerefQuietly(op.map_expr(f)),
            Self::DelDeref(op) => OperationDetail::DelDeref(op.map_expr(f)),
        }
    }

    pub fn map_expr_and_name<T, M>(
        self,
        f_expr: &mut impl FnMut(E) -> T,
        f_name: &mut impl FnMut(N) -> M,
    ) -> OperationDetail<T, M> {
        match self {
            Self::BinOp(op) => OperationDetail::BinOp(op.map_expr(f_expr)),
            Self::UnaryOp(op) => OperationDetail::UnaryOp(op.map_expr(f_expr)),
            Self::InplaceBinOp(op) => OperationDetail::InplaceBinOp(op.map_expr(f_expr)),
            Self::TernaryOp(op) => OperationDetail::TernaryOp(op.map_expr(f_expr)),
            Self::GetAttr(op) => OperationDetail::GetAttr(op.map_expr(f_expr)),
            Self::SetAttr(op) => OperationDetail::SetAttr(op.map_expr(f_expr)),
            Self::GetItem(op) => OperationDetail::GetItem(op.map_expr(f_expr)),
            Self::SetItem(op) => OperationDetail::SetItem(op.map_expr(f_expr)),
            Self::DelItem(op) => OperationDetail::DelItem(op.map_expr(f_expr)),
            Self::LoadGlobal(op) => OperationDetail::LoadGlobal(op.map_expr(f_expr)),
            Self::StoreGlobal(op) => OperationDetail::StoreGlobal(op.map_expr(f_expr)),
            Self::LoadName(op) => OperationDetail::LoadName(op.map_expr_and_name(f_expr, f_name)),
            Self::LoadLocal(op) => OperationDetail::LoadLocal(op.map_expr_and_name(f_expr, f_name)),
            Self::LoadCell(op) => OperationDetail::LoadCell(op.map_expr_and_name(f_expr, f_name)),
            Self::MakeCell(op) => OperationDetail::MakeCell(op.map_expr(f_expr)),
            Self::MakeString(op) => OperationDetail::MakeString(op.map_expr(f_expr)),
            Self::CellRef(op) => OperationDetail::CellRef(op.map_expr_and_name(f_expr, f_name)),
            Self::MakeFunction(op) => OperationDetail::MakeFunction(op.map_expr(f_expr)),
            Self::StoreCell(op) => OperationDetail::StoreCell(op.map_expr_and_name(f_expr, f_name)),
            Self::DelQuietly(op) => OperationDetail::DelQuietly(op.map_expr(f_expr)),
            Self::DelDerefQuietly(op) => {
                OperationDetail::DelDerefQuietly(op.map_expr_and_name(f_expr, f_name))
            }
            Self::DelDeref(op) => OperationDetail::DelDeref(op.map_expr_and_name(f_expr, f_name)),
        }
    }

    pub fn try_map_expr<T, Error>(
        self,
        f: &mut impl FnMut(E) -> Result<T, Error>,
    ) -> Result<OperationDetail<T, N>, Error> {
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
            Self::LoadName(op) => OperationDetail::LoadName(op.try_map_expr(f)?),
            Self::LoadLocal(op) => OperationDetail::LoadLocal(op.try_map_expr(f)?),
            Self::LoadCell(op) => OperationDetail::LoadCell(op.try_map_expr(f)?),
            Self::MakeCell(op) => OperationDetail::MakeCell(op.try_map_expr(f)?),
            Self::MakeString(op) => OperationDetail::MakeString(op.try_map_expr(f)?),
            Self::CellRef(op) => OperationDetail::CellRef(op.try_map_expr(f)?),
            Self::MakeFunction(op) => OperationDetail::MakeFunction(op.try_map_expr(f)?),
            Self::StoreCell(op) => OperationDetail::StoreCell(op.try_map_expr(f)?),
            Self::DelQuietly(op) => OperationDetail::DelQuietly(op.try_map_expr(f)?),
            Self::DelDerefQuietly(op) => OperationDetail::DelDerefQuietly(op.try_map_expr(f)?),
            Self::DelDeref(op) => OperationDetail::DelDeref(op.try_map_expr(f)?),
        })
    }

    pub fn try_map_expr_and_name<T, M, Error>(
        self,
        f_expr: &mut impl FnMut(E) -> Result<T, Error>,
        f_name: &mut impl FnMut(N) -> Result<M, Error>,
    ) -> Result<OperationDetail<T, M>, Error> {
        Ok(match self {
            Self::BinOp(op) => OperationDetail::BinOp(op.try_map_expr(f_expr)?),
            Self::UnaryOp(op) => OperationDetail::UnaryOp(op.try_map_expr(f_expr)?),
            Self::InplaceBinOp(op) => OperationDetail::InplaceBinOp(op.try_map_expr(f_expr)?),
            Self::TernaryOp(op) => OperationDetail::TernaryOp(op.try_map_expr(f_expr)?),
            Self::GetAttr(op) => OperationDetail::GetAttr(op.try_map_expr(f_expr)?),
            Self::SetAttr(op) => OperationDetail::SetAttr(op.try_map_expr(f_expr)?),
            Self::GetItem(op) => OperationDetail::GetItem(op.try_map_expr(f_expr)?),
            Self::SetItem(op) => OperationDetail::SetItem(op.try_map_expr(f_expr)?),
            Self::DelItem(op) => OperationDetail::DelItem(op.try_map_expr(f_expr)?),
            Self::LoadGlobal(op) => OperationDetail::LoadGlobal(op.try_map_expr(f_expr)?),
            Self::StoreGlobal(op) => OperationDetail::StoreGlobal(op.try_map_expr(f_expr)?),
            Self::LoadName(op) => {
                OperationDetail::LoadName(op.try_map_expr_and_name(f_expr, f_name)?)
            }
            Self::LoadLocal(op) => {
                OperationDetail::LoadLocal(op.try_map_expr_and_name(f_expr, f_name)?)
            }
            Self::LoadCell(op) => {
                OperationDetail::LoadCell(op.try_map_expr_and_name(f_expr, f_name)?)
            }
            Self::MakeCell(op) => OperationDetail::MakeCell(op.try_map_expr(f_expr)?),
            Self::MakeString(op) => OperationDetail::MakeString(op.try_map_expr(f_expr)?),
            Self::CellRef(op) => {
                OperationDetail::CellRef(op.try_map_expr_and_name(f_expr, f_name)?)
            }
            Self::MakeFunction(op) => OperationDetail::MakeFunction(op.try_map_expr(f_expr)?),
            Self::StoreCell(op) => {
                OperationDetail::StoreCell(op.try_map_expr_and_name(f_expr, f_name)?)
            }
            Self::DelQuietly(op) => OperationDetail::DelQuietly(op.try_map_expr(f_expr)?),
            Self::DelDerefQuietly(op) => {
                OperationDetail::DelDerefQuietly(op.try_map_expr_and_name(f_expr, f_name)?)
            }
            Self::DelDeref(op) => {
                OperationDetail::DelDeref(op.try_map_expr_and_name(f_expr, f_name)?)
            }
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
            Self::LoadName(op) => op.walk_expr_args(f),
            Self::LoadLocal(op) => op.walk_expr_args(f),
            Self::LoadCell(op) => op.walk_expr_args(f),
            Self::MakeCell(op) => op.walk_expr_args(f),
            Self::MakeString(op) => op.walk_expr_args(f),
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
            Self::LoadName(op) => op.walk_expr_args_mut(f),
            Self::LoadLocal(op) => op.walk_expr_args_mut(f),
            Self::LoadCell(op) => op.walk_expr_args_mut(f),
            Self::MakeCell(op) => op.walk_expr_args_mut(f),
            Self::MakeString(op) => op.walk_expr_args_mut(f),
            Self::CellRef(op) => op.walk_expr_args_mut(f),
            Self::MakeFunction(op) => op.walk_expr_args_mut(f),
            Self::StoreCell(op) => op.walk_expr_args_mut(f),
            Self::DelQuietly(op) => op.walk_expr_args_mut(f),
            Self::DelDerefQuietly(op) => op.walk_expr_args_mut(f),
            Self::DelDeref(op) => op.walk_expr_args_mut(f),
        }
    }

    pub fn into_call_args(self) -> Vec<E> {
        match self {
            Self::BinOp(op) => op.into_expr_args(),
            Self::UnaryOp(op) => op.into_expr_args(),
            Self::InplaceBinOp(op) => op.into_expr_args(),
            Self::TernaryOp(op) => op.into_expr_args(),
            Self::GetAttr(op) => op.into_expr_args(),
            Self::SetAttr(op) => op.into_expr_args(),
            Self::GetItem(op) => op.into_expr_args(),
            Self::SetItem(op) => op.into_expr_args(),
            Self::DelItem(op) => op.into_expr_args(),
            Self::LoadGlobal(op) => op.into_expr_args(),
            Self::StoreGlobal(op) => op.into_expr_args(),
            Self::LoadName(op) => op.into_expr_args(),
            Self::LoadLocal(op) => op.into_expr_args(),
            Self::LoadCell(op) => op.into_expr_args(),
            Self::MakeCell(op) => op.into_expr_args(),
            Self::MakeString(op) => op.into_expr_args(),
            Self::CellRef(op) => op.into_expr_args(),
            Self::MakeFunction(op) => op.into_expr_args(),
            Self::StoreCell(op) => op.into_expr_args(),
            Self::DelQuietly(op) => op.into_expr_args(),
            Self::DelDerefQuietly(op) => op.into_expr_args(),
            Self::DelDeref(op) => op.into_expr_args(),
        }
    }

    pub fn call_args(&self) -> Vec<&E> {
        match self {
            Self::BinOp(op) => vec![op.arg0.as_ref(), op.arg1.as_ref()],
            Self::UnaryOp(op) => vec![op.arg0.as_ref()],
            Self::InplaceBinOp(op) => vec![op.arg0.as_ref(), op.arg1.as_ref()],
            Self::TernaryOp(op) => vec![op.arg0.as_ref(), op.arg1.as_ref(), op.arg2.as_ref()],
            Self::GetAttr(op) => vec![op.arg0.as_ref()],
            Self::SetAttr(op) => vec![op.arg0.as_ref(), op.arg2.as_ref()],
            Self::GetItem(op) => vec![op.arg0.as_ref(), op.arg1.as_ref()],
            Self::SetItem(op) => vec![op.arg0.as_ref(), op.arg1.as_ref(), op.arg2.as_ref()],
            Self::DelItem(op) => vec![op.arg0.as_ref(), op.arg1.as_ref()],
            Self::LoadGlobal(op) => vec![op.arg0.as_ref()],
            Self::StoreGlobal(op) => vec![op.arg0.as_ref(), op.arg2.as_ref()],
            Self::LoadName(_) => Vec::new(),
            Self::LoadLocal(_) => Vec::new(),
            Self::LoadCell(_) => Vec::new(),
            Self::MakeCell(op) => vec![op.arg0.as_ref()],
            Self::MakeString(_) => Vec::new(),
            Self::CellRef(_) => Vec::new(),
            Self::MakeFunction(op) => vec![op.arg0.as_ref(), op.arg1.as_ref(), op.arg2.as_ref()],
            Self::StoreCell(op) => vec![op.arg1.as_ref()],
            Self::DelQuietly(op) => vec![op.arg0.as_ref()],
            Self::DelDerefQuietly(_) => Vec::new(),
            Self::DelDeref(_) => Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Operation<E, N = E> {
    pub meta: Meta,
    pub detail: OperationDetail<E, N>,
}

impl<E, N> Operation<E, N> {
    pub fn new(detail: impl Into<OperationDetail<E, N>>) -> Self {
        Self {
            meta: Meta::synthetic(),
            detail: detail.into(),
        }
    }

    pub fn detail(&self) -> &OperationDetail<E, N> {
        &self.detail
    }

    pub fn detail_mut(&mut self) -> &mut OperationDetail<E, N> {
        &mut self.detail
    }

    pub fn into_detail(self) -> OperationDetail<E, N> {
        self.detail
    }

    pub fn node_index(&self) -> &ast::AtomicNodeIndex {
        &self.meta.node_index
    }

    pub fn range(&self) -> TextRange {
        self.meta.range
    }

    pub fn map_expr<T>(self, f: &mut impl FnMut(E) -> T) -> Operation<T, N> {
        Operation {
            meta: self.meta,
            detail: self.detail.map_expr(f),
        }
    }

    pub fn map_expr_and_name<T, M>(
        self,
        f_expr: &mut impl FnMut(E) -> T,
        f_name: &mut impl FnMut(N) -> M,
    ) -> Operation<T, M> {
        Operation {
            meta: self.meta,
            detail: self.detail.map_expr_and_name(f_expr, f_name),
        }
    }

    pub fn try_map_expr<T, Error>(
        self,
        f: &mut impl FnMut(E) -> Result<T, Error>,
    ) -> Result<Operation<T, N>, Error> {
        Ok(Operation {
            meta: self.meta,
            detail: self.detail.try_map_expr(f)?,
        })
    }

    pub fn try_map_expr_and_name<T, M, Error>(
        self,
        f_expr: &mut impl FnMut(E) -> Result<T, Error>,
        f_name: &mut impl FnMut(N) -> Result<M, Error>,
    ) -> Result<Operation<T, M>, Error> {
        Ok(Operation {
            meta: self.meta,
            detail: self.detail.try_map_expr_and_name(f_expr, f_name)?,
        })
    }

    pub fn walk_args(&self, f: &mut impl FnMut(&E)) {
        self.detail.walk_args(f)
    }

    pub fn walk_args_mut(&mut self, f: &mut impl FnMut(&mut E)) {
        self.detail.walk_args_mut(f)
    }

    pub fn into_call_args(self) -> Vec<E> {
        self.detail.into_call_args()
    }

    pub fn call_args(&self) -> Vec<&E> {
        self.detail.call_args()
    }
}

impl<E, N> WithMeta for Operation<E, N> {
    fn with_meta(mut self, meta: Meta) -> Self {
        self.meta = meta;
        self
    }
}

impl<E, N> HasMeta for Operation<E, N> {
    fn meta(&self) -> Meta {
        self.meta.clone()
    }
}

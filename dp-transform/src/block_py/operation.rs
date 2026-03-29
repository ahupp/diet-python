use super::{BlockPyFunctionKind, FunctionId};
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
    pub fn helper_name(self) -> &'static str {
        match self {
            Self::Pos => "__dp_pos",
            Self::Neg => "__dp_neg",
            Self::Invert => "__dp_invert",
            Self::Not => "__dp_not_",
            Self::Truth => "__dp_truth",
        }
    }

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
    pub fn helper_name(self) -> &'static str {
        match self {
            Self::Pow => "__dp_pow",
        }
    }

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

    fn name(&self) -> &'static str;
    fn node_index(&self) -> &ast::AtomicNodeIndex;
    fn range(&self) -> TextRange;
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
        $($mapped_ctor)+ {
            node_index: $self.node_index,
            range: $self.range,
            $($out)*
        }
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
        Ok($($mapped_ctor)+ {
            node_index: $self.node_index,
            range: $self.range,
            $($out)*
        })
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
            name($self_ident:ident) = $name_expr:expr;
            $( $field:ident : $ty:ty => $kind:ident ),* $(,)?
        }
    ) => {
        #[derive(Debug, Clone)]
        $vis struct $name $(<$($struct_gen),*>)? {
            pub node_index: ast::AtomicNodeIndex,
            pub range: TextRange,
            $(pub $field: $ty,)*
        }

        impl<$($impl_gen),*> OperationNode<E> for $name $(<$($struct_gen),*>)? {
            type Name = $($name_ty)+;
            type Mapped<$mapped_expr, $mapped_name> = $($mapped_ty)+;

            fn name(&self) -> &'static str {
                let $self_ident = self;
                $name_expr
            }

            fn node_index(&self) -> &ast::AtomicNodeIndex {
                &self.node_index
            }

            fn range(&self) -> TextRange {
                self.range
            }

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
        name(op) = op.kind.helper_name();
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
        name(op) = op.kind.helper_name();
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
        name(op) = op.kind.helper_name();
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
        name(op) = op.kind.helper_name();
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
        name(_op) = "__dp_getattr";
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
        name(_op) = "__dp_setattr";
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
        name(_op) = "__dp_getitem";
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
        name(_op) = "__dp_setitem";
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
        name(_op) = "__dp_delitem";
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
        name(_op) = "__dp_load_global";
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
        name(_op) = "__dp_store_global";
        arg0: Box<E> => expr,
        arg1: String => value,
        arg2: Box<E> => expr,
    }
}

define_operation_node! {
    pub struct LoadCell<N> {
        impl<E, N>;
        name_type = [N];
        mapped_type<T, M> = [LoadCell<M>];
        mapped_ctor<T, M> = [LoadCell::<M>];
        name(_op) = "__dp_load_cell";
        arg0: N => name,
    }
}

define_operation_node! {
    pub struct MakeCell<E> {
        impl<E>;
        name_type = [()];
        mapped_type<T, M> = [MakeCell<T>];
        mapped_ctor<T, M> = [MakeCell::<T>];
        name(_op) = "__dp_make_cell";
        arg0: Box<E> => expr,
    }
}

define_operation_node! {
    pub struct MakeString {
        impl<E>;
        name_type = [()];
        mapped_type<T, M> = [MakeString];
        mapped_ctor<T, M> = [MakeString];
        name(_op) = "__dp_decode_literal_bytes";
        arg0: Vec<u8> => value,
    }
}

define_operation_node! {
    pub struct CellRef<N> {
        impl<E, N>;
        name_type = [N];
        mapped_type<T, M> = [CellRef<M>];
        mapped_ctor<T, M> = [CellRef::<M>];
        name(_op) = "__dp_cell_ref";
        arg0: CellRefTarget<N> => name_target,
    }
}

define_operation_node! {
    pub struct MakeFunction<E> {
        impl<E>;
        name_type = [()];
        mapped_type<T, M> = [MakeFunction<T>];
        mapped_ctor<T, M> = [MakeFunction::<T>];
        name(_op) = "__dp_make_function";
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
        name(_op) = "__dp_store_cell";
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
        name(_op) = "__dp_del_quietly";
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
        name(_op) = "__dp_del_deref_quietly";
        arg0: N => name,
    }
}

define_operation_node! {
    pub struct DelDeref<N> {
        impl<E, N>;
        name_type = [N];
        mapped_type<T, M> = [DelDeref<M>];
        mapped_ctor<T, M> = [DelDeref::<M>];
        name(_op) = "__dp_del_deref";
        arg0: N => name,
    }
}

#[derive(Debug, Clone)]
pub enum Operation<E, N = E> {
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

impl<E, N> Operation<E, N> {
    pub fn helper_name(&self) -> &'static str {
        match self {
            Self::BinOp(op) => op.name(),
            Self::UnaryOp(op) => op.name(),
            Self::InplaceBinOp(op) => op.name(),
            Self::TernaryOp(op) => op.name(),
            Self::GetAttr(op) => op.name(),
            Self::SetAttr(op) => op.name(),
            Self::GetItem(op) => op.name(),
            Self::SetItem(op) => op.name(),
            Self::DelItem(op) => op.name(),
            Self::LoadGlobal(op) => op.name(),
            Self::StoreGlobal(op) => op.name(),
            Self::LoadCell(op) => <LoadCell<N> as OperationNode<E>>::name(op),
            Self::MakeCell(op) => op.name(),
            Self::MakeString(op) => <MakeString as OperationNode<E>>::name(op),
            Self::CellRef(op) => <CellRef<N> as OperationNode<E>>::name(op),
            Self::MakeFunction(op) => op.name(),
            Self::StoreCell(op) => op.name(),
            Self::DelQuietly(op) => op.name(),
            Self::DelDerefQuietly(op) => <DelDerefQuietly<N> as OperationNode<E>>::name(op),
            Self::DelDeref(op) => <DelDeref<N> as OperationNode<E>>::name(op),
        }
    }

    pub fn node_index(&self) -> &ast::AtomicNodeIndex {
        match self {
            Self::BinOp(op) => op.node_index(),
            Self::UnaryOp(op) => op.node_index(),
            Self::InplaceBinOp(op) => op.node_index(),
            Self::TernaryOp(op) => op.node_index(),
            Self::GetAttr(op) => op.node_index(),
            Self::SetAttr(op) => op.node_index(),
            Self::GetItem(op) => op.node_index(),
            Self::SetItem(op) => op.node_index(),
            Self::DelItem(op) => op.node_index(),
            Self::LoadGlobal(op) => op.node_index(),
            Self::StoreGlobal(op) => op.node_index(),
            Self::LoadCell(op) => <LoadCell<N> as OperationNode<E>>::node_index(op),
            Self::MakeCell(op) => op.node_index(),
            Self::MakeString(op) => <MakeString as OperationNode<E>>::node_index(op),
            Self::CellRef(op) => <CellRef<N> as OperationNode<E>>::node_index(op),
            Self::MakeFunction(op) => op.node_index(),
            Self::StoreCell(op) => op.node_index(),
            Self::DelQuietly(op) => op.node_index(),
            Self::DelDerefQuietly(op) => <DelDerefQuietly<N> as OperationNode<E>>::node_index(op),
            Self::DelDeref(op) => <DelDeref<N> as OperationNode<E>>::node_index(op),
        }
    }

    pub fn range(&self) -> TextRange {
        match self {
            Self::BinOp(op) => op.range(),
            Self::UnaryOp(op) => op.range(),
            Self::InplaceBinOp(op) => op.range(),
            Self::TernaryOp(op) => op.range(),
            Self::GetAttr(op) => op.range(),
            Self::SetAttr(op) => op.range(),
            Self::GetItem(op) => op.range(),
            Self::SetItem(op) => op.range(),
            Self::DelItem(op) => op.range(),
            Self::LoadGlobal(op) => op.range(),
            Self::StoreGlobal(op) => op.range(),
            Self::LoadCell(op) => <LoadCell<N> as OperationNode<E>>::range(op),
            Self::MakeCell(op) => op.range(),
            Self::MakeString(op) => <MakeString as OperationNode<E>>::range(op),
            Self::CellRef(op) => <CellRef<N> as OperationNode<E>>::range(op),
            Self::MakeFunction(op) => op.range(),
            Self::StoreCell(op) => op.range(),
            Self::DelQuietly(op) => op.range(),
            Self::DelDerefQuietly(op) => <DelDerefQuietly<N> as OperationNode<E>>::range(op),
            Self::DelDeref(op) => <DelDeref<N> as OperationNode<E>>::range(op),
        }
    }

    pub fn map_expr<T>(self, f: &mut impl FnMut(E) -> T) -> Operation<T, N> {
        match self {
            Self::BinOp(op) => Operation::BinOp(op.map_expr(f)),
            Self::UnaryOp(op) => Operation::UnaryOp(op.map_expr(f)),
            Self::InplaceBinOp(op) => Operation::InplaceBinOp(op.map_expr(f)),
            Self::TernaryOp(op) => Operation::TernaryOp(op.map_expr(f)),
            Self::GetAttr(op) => Operation::GetAttr(op.map_expr(f)),
            Self::SetAttr(op) => Operation::SetAttr(op.map_expr(f)),
            Self::GetItem(op) => Operation::GetItem(op.map_expr(f)),
            Self::SetItem(op) => Operation::SetItem(op.map_expr(f)),
            Self::DelItem(op) => Operation::DelItem(op.map_expr(f)),
            Self::LoadGlobal(op) => Operation::LoadGlobal(op.map_expr(f)),
            Self::StoreGlobal(op) => Operation::StoreGlobal(op.map_expr(f)),
            Self::LoadCell(op) => Operation::LoadCell(op.map_expr(f)),
            Self::MakeCell(op) => Operation::MakeCell(op.map_expr(f)),
            Self::MakeString(op) => Operation::MakeString(op.map_expr(f)),
            Self::CellRef(op) => Operation::CellRef(op.map_expr(f)),
            Self::MakeFunction(op) => Operation::MakeFunction(op.map_expr(f)),
            Self::StoreCell(op) => Operation::StoreCell(op.map_expr(f)),
            Self::DelQuietly(op) => Operation::DelQuietly(op.map_expr(f)),
            Self::DelDerefQuietly(op) => Operation::DelDerefQuietly(op.map_expr(f)),
            Self::DelDeref(op) => Operation::DelDeref(op.map_expr(f)),
        }
    }

    pub fn map_expr_and_name<T, M>(
        self,
        f_expr: &mut impl FnMut(E) -> T,
        f_name: &mut impl FnMut(N) -> M,
    ) -> Operation<T, M> {
        match self {
            Self::BinOp(op) => Operation::BinOp(op.map_expr(f_expr)),
            Self::UnaryOp(op) => Operation::UnaryOp(op.map_expr(f_expr)),
            Self::InplaceBinOp(op) => Operation::InplaceBinOp(op.map_expr(f_expr)),
            Self::TernaryOp(op) => Operation::TernaryOp(op.map_expr(f_expr)),
            Self::GetAttr(op) => Operation::GetAttr(op.map_expr(f_expr)),
            Self::SetAttr(op) => Operation::SetAttr(op.map_expr(f_expr)),
            Self::GetItem(op) => Operation::GetItem(op.map_expr(f_expr)),
            Self::SetItem(op) => Operation::SetItem(op.map_expr(f_expr)),
            Self::DelItem(op) => Operation::DelItem(op.map_expr(f_expr)),
            Self::LoadGlobal(op) => Operation::LoadGlobal(op.map_expr(f_expr)),
            Self::StoreGlobal(op) => Operation::StoreGlobal(op.map_expr(f_expr)),
            Self::LoadCell(op) => Operation::LoadCell(op.map_expr_and_name(f_expr, f_name)),
            Self::MakeCell(op) => Operation::MakeCell(op.map_expr(f_expr)),
            Self::MakeString(op) => Operation::MakeString(op.map_expr(f_expr)),
            Self::CellRef(op) => Operation::CellRef(op.map_expr_and_name(f_expr, f_name)),
            Self::MakeFunction(op) => Operation::MakeFunction(op.map_expr(f_expr)),
            Self::StoreCell(op) => Operation::StoreCell(op.map_expr_and_name(f_expr, f_name)),
            Self::DelQuietly(op) => Operation::DelQuietly(op.map_expr(f_expr)),
            Self::DelDerefQuietly(op) => {
                Operation::DelDerefQuietly(op.map_expr_and_name(f_expr, f_name))
            }
            Self::DelDeref(op) => Operation::DelDeref(op.map_expr_and_name(f_expr, f_name)),
        }
    }

    pub fn try_map_expr<T, Error>(
        self,
        f: &mut impl FnMut(E) -> Result<T, Error>,
    ) -> Result<Operation<T, N>, Error> {
        Ok(match self {
            Self::BinOp(op) => Operation::BinOp(op.try_map_expr(f)?),
            Self::UnaryOp(op) => Operation::UnaryOp(op.try_map_expr(f)?),
            Self::InplaceBinOp(op) => Operation::InplaceBinOp(op.try_map_expr(f)?),
            Self::TernaryOp(op) => Operation::TernaryOp(op.try_map_expr(f)?),
            Self::GetAttr(op) => Operation::GetAttr(op.try_map_expr(f)?),
            Self::SetAttr(op) => Operation::SetAttr(op.try_map_expr(f)?),
            Self::GetItem(op) => Operation::GetItem(op.try_map_expr(f)?),
            Self::SetItem(op) => Operation::SetItem(op.try_map_expr(f)?),
            Self::DelItem(op) => Operation::DelItem(op.try_map_expr(f)?),
            Self::LoadGlobal(op) => Operation::LoadGlobal(op.try_map_expr(f)?),
            Self::StoreGlobal(op) => Operation::StoreGlobal(op.try_map_expr(f)?),
            Self::LoadCell(op) => Operation::LoadCell(op.try_map_expr(f)?),
            Self::MakeCell(op) => Operation::MakeCell(op.try_map_expr(f)?),
            Self::MakeString(op) => Operation::MakeString(op.try_map_expr(f)?),
            Self::CellRef(op) => Operation::CellRef(op.try_map_expr(f)?),
            Self::MakeFunction(op) => Operation::MakeFunction(op.try_map_expr(f)?),
            Self::StoreCell(op) => Operation::StoreCell(op.try_map_expr(f)?),
            Self::DelQuietly(op) => Operation::DelQuietly(op.try_map_expr(f)?),
            Self::DelDerefQuietly(op) => Operation::DelDerefQuietly(op.try_map_expr(f)?),
            Self::DelDeref(op) => Operation::DelDeref(op.try_map_expr(f)?),
        })
    }

    pub fn try_map_expr_and_name<T, M, Error>(
        self,
        f_expr: &mut impl FnMut(E) -> Result<T, Error>,
        f_name: &mut impl FnMut(N) -> Result<M, Error>,
    ) -> Result<Operation<T, M>, Error> {
        Ok(match self {
            Self::BinOp(op) => Operation::BinOp(op.try_map_expr(f_expr)?),
            Self::UnaryOp(op) => Operation::UnaryOp(op.try_map_expr(f_expr)?),
            Self::InplaceBinOp(op) => Operation::InplaceBinOp(op.try_map_expr(f_expr)?),
            Self::TernaryOp(op) => Operation::TernaryOp(op.try_map_expr(f_expr)?),
            Self::GetAttr(op) => Operation::GetAttr(op.try_map_expr(f_expr)?),
            Self::SetAttr(op) => Operation::SetAttr(op.try_map_expr(f_expr)?),
            Self::GetItem(op) => Operation::GetItem(op.try_map_expr(f_expr)?),
            Self::SetItem(op) => Operation::SetItem(op.try_map_expr(f_expr)?),
            Self::DelItem(op) => Operation::DelItem(op.try_map_expr(f_expr)?),
            Self::LoadGlobal(op) => Operation::LoadGlobal(op.try_map_expr(f_expr)?),
            Self::StoreGlobal(op) => Operation::StoreGlobal(op.try_map_expr(f_expr)?),
            Self::LoadCell(op) => Operation::LoadCell(op.try_map_expr_and_name(f_expr, f_name)?),
            Self::MakeCell(op) => Operation::MakeCell(op.try_map_expr(f_expr)?),
            Self::MakeString(op) => Operation::MakeString(op.try_map_expr(f_expr)?),
            Self::CellRef(op) => Operation::CellRef(op.try_map_expr_and_name(f_expr, f_name)?),
            Self::MakeFunction(op) => Operation::MakeFunction(op.try_map_expr(f_expr)?),
            Self::StoreCell(op) => Operation::StoreCell(op.try_map_expr_and_name(f_expr, f_name)?),
            Self::DelQuietly(op) => Operation::DelQuietly(op.try_map_expr(f_expr)?),
            Self::DelDerefQuietly(op) => {
                Operation::DelDerefQuietly(op.try_map_expr_and_name(f_expr, f_name)?)
            }
            Self::DelDeref(op) => Operation::DelDeref(op.try_map_expr_and_name(f_expr, f_name)?),
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

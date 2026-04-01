use super::{
    BlockPyFunctionKind, CellLocation, CoreBlockPyCallArg, CoreBlockPyKeywordArg, FunctionId,
    HasMeta, Meta, NameLocation, WithMeta,
};

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

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum UnaryOpKind {
    Pos,
    Neg,
    Invert,
    Not,
    Truth,
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

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum TernaryOpKind {
    Pow,
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

macro_rules! operation_detail_variants {
    ($macro:ident) => {
        $macro! {
            BinOp(BinOp<E>),
            UnaryOp(UnaryOp<E>),
            InplaceBinOp(InplaceBinOp<E>),
            TernaryOp(TernaryOp<E>),
            Call(Call<E>),
            GetAttr(GetAttr<E>),
            SetAttr(SetAttr<E>),
            GetItem(GetItem<E>),
            SetItem(SetItem<E>),
            DelItem(DelItem<E>),
            LoadRuntime(LoadRuntime),
            LoadName(LoadName),
            StoreName(StoreName<E>),
            DelName(DelName),
            LoadLocation(LoadLocation),
            MakeCell(MakeCell<E>),
            MakeString(MakeString),
            CellRefForName(CellRefForName),
            CellRef(CellRef),
            MakeFunction(MakeFunction<E>),
            StoreLocation(StoreLocation<E>),
            DelLocation(DelLocation),
        }
    };
    ($macro:ident, $($args:tt)*) => {
        $macro! {
            $($args)*
            BinOp(BinOp<E>),
            UnaryOp(UnaryOp<E>),
            InplaceBinOp(InplaceBinOp<E>),
            TernaryOp(TernaryOp<E>),
            Call(Call<E>),
            GetAttr(GetAttr<E>),
            SetAttr(SetAttr<E>),
            GetItem(GetItem<E>),
            SetItem(SetItem<E>),
            DelItem(DelItem<E>),
            LoadRuntime(LoadRuntime),
            LoadName(LoadName),
            StoreName(StoreName<E>),
            DelName(DelName),
            LoadLocation(LoadLocation),
            MakeCell(MakeCell<E>),
            MakeString(MakeString),
            CellRefForName(CellRefForName),
            CellRef(CellRef),
            MakeFunction(MakeFunction<E>),
            StoreLocation(StoreLocation<E>),
            DelLocation(DelLocation),
        }
    };
}

macro_rules! define_operation_detail_enum {
    ($( $variant:ident($variant_ty:ty), )*) => {
        #[derive(Debug, Clone, derive_more::From)]
        pub enum OperationDetail<E> {
            $( $variant($variant_ty), )*
        }
    };
}

macro_rules! impl_operation_detail_has_meta {
    ($( $variant:ident($variant_ty:ty), )*) => {
        impl<E> HasMeta for OperationDetail<E> {
            fn meta(&self) -> Meta {
                match self {
                    $( OperationDetail::$variant(op) => op.meta(), )*
                }
            }
        }
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

#[derive(Debug, Clone)]
pub struct Call<E> {
    _meta: Meta,
    pub func: Box<E>,
    pub args: Vec<CoreBlockPyCallArg<E>>,
    pub keywords: Vec<CoreBlockPyKeywordArg<E>>,
}

impl<E> Call<E> {
    pub fn new(
        func: impl Into<Box<E>>,
        args: impl Into<Vec<CoreBlockPyCallArg<E>>>,
        keywords: impl Into<Vec<CoreBlockPyKeywordArg<E>>>,
    ) -> Self {
        Self {
            _meta: Meta::default(),
            func: func.into(),
            args: args.into(),
            keywords: keywords.into(),
        }
    }
}

impl<E> HasMeta for Call<E> {
    fn meta(&self) -> Meta {
        self._meta.clone()
    }
}

impl<E> WithMeta for Call<E> {
    fn with_meta(mut self, meta: Meta) -> Self {
        self._meta = meta;
        self
    }
}

impl<E> ExprOperationNode<E> for Call<E> {
    type Mapped<T> = Call<T>;

    fn visit_exprs(&self, f: &mut impl FnMut(&E)) {
        f(&self.func);
        for arg in &self.args {
            f(arg.expr());
        }
        for keyword in &self.keywords {
            f(keyword.expr());
        }
    }

    fn visit_exprs_mut(&mut self, f: &mut impl FnMut(&mut E)) {
        f(&mut self.func);
        for arg in &mut self.args {
            f(arg.expr_mut());
        }
        for keyword in &mut self.keywords {
            f(keyword.expr_mut());
        }
    }

    fn map_op<T>(self, f: &mut impl FnMut(E) -> T) -> Self::Mapped<T> {
        Call {
            _meta: self._meta,
            func: Box::new(f(*self.func)),
            args: self
                .args
                .into_iter()
                .map(|arg| arg.map_expr(&mut *f))
                .collect(),
            keywords: self
                .keywords
                .into_iter()
                .map(|keyword| keyword.map_expr(&mut *f))
                .collect(),
        }
    }

    fn try_map_op<T, Error>(
        self,
        f: &mut impl FnMut(E) -> Result<T, Error>,
    ) -> Result<Self::Mapped<T>, Error> {
        Ok(Call {
            _meta: self._meta,
            func: Box::new(f(*self.func)?),
            args: self
                .args
                .into_iter()
                .map(|arg| arg.try_map_expr(&mut *f))
                .collect::<Result<Vec<_>, _>>()?,
            keywords: self
                .keywords
                .into_iter()
                .map(|keyword| keyword.try_map_expr(&mut *f))
                .collect::<Result<Vec<_>, _>>()?,
        })
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
    pub struct StoreName<E> {
        name: String,
        value: Box<E>,
    }
}

define_operation! {
    pub struct DelName {
        name: String,
        quietly: bool,
    }
}

define_operation! {
    pub struct LoadLocation {
        location: NameLocation,
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
    pub struct StoreLocation<E> {
        location: NameLocation,
        value: Box<E>,
    }
}

define_operation! {
    pub struct DelLocation {
        location: NameLocation,
        quietly: bool,
    }
}

operation_detail_variants!(define_operation_detail_enum);
operation_detail_variants!(impl_operation_detail_has_meta);

impl<E> OperationDetail<E> {
    pub fn map_expr<T>(self, f: &mut impl FnMut(E) -> T) -> OperationDetail<T> {
        match self {
            Self::BinOp(op) => OperationDetail::BinOp(op.map_op(f)),
            Self::UnaryOp(op) => OperationDetail::UnaryOp(op.map_op(f)),
            Self::InplaceBinOp(op) => OperationDetail::InplaceBinOp(op.map_op(f)),
            Self::TernaryOp(op) => OperationDetail::TernaryOp(op.map_op(f)),
            Self::Call(op) => OperationDetail::Call(op.map_op(f)),
            Self::GetAttr(op) => OperationDetail::GetAttr(op.map_op(f)),
            Self::SetAttr(op) => OperationDetail::SetAttr(op.map_op(f)),
            Self::GetItem(op) => OperationDetail::GetItem(op.map_op(f)),
            Self::SetItem(op) => OperationDetail::SetItem(op.map_op(f)),
            Self::DelItem(op) => OperationDetail::DelItem(op.map_op(f)),
            Self::LoadRuntime(op) => OperationDetail::LoadRuntime(op.map_op(f)),
            Self::LoadName(op) => OperationDetail::LoadName(op.map_op(f)),
            Self::StoreName(op) => OperationDetail::StoreName(op.map_op(f)),
            Self::DelName(op) => OperationDetail::DelName(op.map_op(f)),
            Self::LoadLocation(op) => OperationDetail::LoadLocation(op.map_op(f)),
            Self::MakeCell(op) => OperationDetail::MakeCell(op.map_op(f)),
            Self::MakeString(op) => OperationDetail::MakeString(op.map_op(f)),
            Self::CellRefForName(op) => OperationDetail::CellRefForName(op.map_op(f)),
            Self::CellRef(op) => OperationDetail::CellRef(op.map_op(f)),
            Self::MakeFunction(op) => OperationDetail::MakeFunction(op.map_op(f)),
            Self::StoreLocation(op) => OperationDetail::StoreLocation(op.map_op(f)),
            Self::DelLocation(op) => OperationDetail::DelLocation(op.map_op(f)),
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
            Self::Call(op) => OperationDetail::Call(op.try_map_op(f)?),
            Self::GetAttr(op) => OperationDetail::GetAttr(op.try_map_op(f)?),
            Self::SetAttr(op) => OperationDetail::SetAttr(op.try_map_op(f)?),
            Self::GetItem(op) => OperationDetail::GetItem(op.try_map_op(f)?),
            Self::SetItem(op) => OperationDetail::SetItem(op.try_map_op(f)?),
            Self::DelItem(op) => OperationDetail::DelItem(op.try_map_op(f)?),
            Self::LoadRuntime(op) => OperationDetail::LoadRuntime(op.try_map_op(f)?),
            Self::LoadName(op) => OperationDetail::LoadName(op.try_map_op(f)?),
            Self::StoreName(op) => OperationDetail::StoreName(op.try_map_op(f)?),
            Self::DelName(op) => OperationDetail::DelName(op.try_map_op(f)?),
            Self::LoadLocation(op) => OperationDetail::LoadLocation(op.try_map_op(f)?),
            Self::MakeCell(op) => OperationDetail::MakeCell(op.try_map_op(f)?),
            Self::MakeString(op) => OperationDetail::MakeString(op.try_map_op(f)?),
            Self::CellRefForName(op) => OperationDetail::CellRefForName(op.try_map_op(f)?),
            Self::CellRef(op) => OperationDetail::CellRef(op.try_map_op(f)?),
            Self::MakeFunction(op) => OperationDetail::MakeFunction(op.try_map_op(f)?),
            Self::StoreLocation(op) => OperationDetail::StoreLocation(op.try_map_op(f)?),
            Self::DelLocation(op) => OperationDetail::DelLocation(op.try_map_op(f)?),
        })
    }

    pub fn walk_args(&self, f: &mut impl FnMut(&E)) {
        match self {
            Self::BinOp(op) => op.visit_exprs(f),
            Self::UnaryOp(op) => op.visit_exprs(f),
            Self::InplaceBinOp(op) => op.visit_exprs(f),
            Self::TernaryOp(op) => op.visit_exprs(f),
            Self::Call(op) => op.visit_exprs(f),
            Self::GetAttr(op) => op.visit_exprs(f),
            Self::SetAttr(op) => op.visit_exprs(f),
            Self::GetItem(op) => op.visit_exprs(f),
            Self::SetItem(op) => op.visit_exprs(f),
            Self::DelItem(op) => op.visit_exprs(f),
            Self::LoadRuntime(op) => op.visit_exprs(f),
            Self::LoadName(op) => op.visit_exprs(f),
            Self::StoreName(op) => op.visit_exprs(f),
            Self::DelName(op) => op.visit_exprs(f),
            Self::LoadLocation(op) => op.visit_exprs(f),
            Self::MakeCell(op) => op.visit_exprs(f),
            Self::MakeString(op) => op.visit_exprs(f),
            Self::CellRefForName(op) => op.visit_exprs(f),
            Self::CellRef(op) => op.visit_exprs(f),
            Self::MakeFunction(op) => op.visit_exprs(f),
            Self::StoreLocation(op) => op.visit_exprs(f),
            Self::DelLocation(op) => op.visit_exprs(f),
        }
    }

    pub fn walk_args_mut(&mut self, f: &mut impl FnMut(&mut E)) {
        match self {
            Self::BinOp(op) => op.visit_exprs_mut(f),
            Self::UnaryOp(op) => op.visit_exprs_mut(f),
            Self::InplaceBinOp(op) => op.visit_exprs_mut(f),
            Self::TernaryOp(op) => op.visit_exprs_mut(f),
            Self::Call(op) => op.visit_exprs_mut(f),
            Self::GetAttr(op) => op.visit_exprs_mut(f),
            Self::SetAttr(op) => op.visit_exprs_mut(f),
            Self::GetItem(op) => op.visit_exprs_mut(f),
            Self::SetItem(op) => op.visit_exprs_mut(f),
            Self::DelItem(op) => op.visit_exprs_mut(f),
            Self::LoadRuntime(op) => op.visit_exprs_mut(f),
            Self::LoadName(op) => op.visit_exprs_mut(f),
            Self::StoreName(op) => op.visit_exprs_mut(f),
            Self::DelName(op) => op.visit_exprs_mut(f),
            Self::LoadLocation(op) => op.visit_exprs_mut(f),
            Self::MakeCell(op) => op.visit_exprs_mut(f),
            Self::MakeString(op) => op.visit_exprs_mut(f),
            Self::CellRefForName(op) => op.visit_exprs_mut(f),
            Self::CellRef(op) => op.visit_exprs_mut(f),
            Self::MakeFunction(op) => op.visit_exprs_mut(f),
            Self::StoreLocation(op) => op.visit_exprs_mut(f),
            Self::DelLocation(op) => op.visit_exprs_mut(f),
        }
    }
}

impl<E> WithMeta for OperationDetail<E> {
    fn with_meta(self, meta: Meta) -> Self {
        match self {
            Self::BinOp(op) => Self::BinOp(op.with_meta(meta.clone())),
            Self::UnaryOp(op) => Self::UnaryOp(op.with_meta(meta.clone())),
            Self::InplaceBinOp(op) => Self::InplaceBinOp(op.with_meta(meta.clone())),
            Self::TernaryOp(op) => Self::TernaryOp(op.with_meta(meta.clone())),
            Self::Call(op) => Self::Call(op.with_meta(meta.clone())),
            Self::GetAttr(op) => Self::GetAttr(op.with_meta(meta.clone())),
            Self::SetAttr(op) => Self::SetAttr(op.with_meta(meta.clone())),
            Self::GetItem(op) => Self::GetItem(op.with_meta(meta.clone())),
            Self::SetItem(op) => Self::SetItem(op.with_meta(meta.clone())),
            Self::DelItem(op) => Self::DelItem(op.with_meta(meta.clone())),
            Self::LoadRuntime(op) => Self::LoadRuntime(op.with_meta(meta.clone())),
            Self::LoadName(op) => Self::LoadName(op.with_meta(meta.clone())),
            Self::StoreName(op) => Self::StoreName(op.with_meta(meta.clone())),
            Self::DelName(op) => Self::DelName(op.with_meta(meta.clone())),
            Self::LoadLocation(op) => Self::LoadLocation(op.with_meta(meta.clone())),
            Self::MakeCell(op) => Self::MakeCell(op.with_meta(meta.clone())),
            Self::MakeString(op) => Self::MakeString(op.with_meta(meta.clone())),
            Self::CellRefForName(op) => Self::CellRefForName(op.with_meta(meta.clone())),
            Self::CellRef(op) => Self::CellRef(op.with_meta(meta.clone())),
            Self::MakeFunction(op) => Self::MakeFunction(op.with_meta(meta.clone())),
            Self::StoreLocation(op) => Self::StoreLocation(op.with_meta(meta.clone())),
            Self::DelLocation(op) => Self::DelLocation(op.with_meta(meta)),
        }
    }
}

use super::{
    BlockPyFunctionKind, CellLocation, CoreBlockPyCallArg, CoreBlockPyKeywordArg, FunctionId,
    HasMeta, Instr, InstrName, Meta, WithMeta,
};
use soac_macros::{with_match_default, DelegateMatchDefault};

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
    Is,
    InplaceAdd,
    InplaceSub,
    InplaceMul,
    InplaceMatMul,
    InplaceTrueDiv,
    InplaceFloorDiv,
    InplaceMod,
    InplacePow,
    InplaceLShift,
    InplaceRShift,
    InplaceOr,
    InplaceXor,
    InplaceAnd,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum UnaryOpKind {
    Pos,
    Neg,
    Invert,
    Not,
    Truth,
}

pub trait InstrOperationNode<I>: Sized
where
    I: Instr,
{
    type Mapped<T: Instr>;

    fn visit_exprs(&self, f: &mut impl FnMut(&I));
    fn visit_exprs_mut(&mut self, f: &mut impl FnMut(&mut I));
    fn map_op<T>(self, f: &mut impl FnMut(I) -> T) -> Self::Mapped<T>
    where
        T: Instr,
        InstrName<T>: From<InstrName<I>>;
    fn try_map_op<T, Error>(
        self,
        f: &mut impl FnMut(I) -> Result<T, Error>,
    ) -> Result<Self::Mapped<T>, Error>
    where
        T: Instr,
        InstrName<T>: From<InstrName<I>>;
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
        $vis struct $name<$expr_ty: Instr> {
            _meta: Meta,
            $($struct_fields)*
        }

        impl<$expr_ty: Instr> $name<$expr_ty> {
            pub fn new($($ctor_args)*) -> Self {
                Self {
                    _meta: Meta::default(),
                    $($ctor_init)*
                }
            }
        }

        impl<$expr_ty: Instr> HasMeta for $name<$expr_ty> {
            fn meta(&self) -> Meta {
                self._meta.clone()
            }
        }

        impl<$expr_ty: Instr> WithMeta for $name<$expr_ty> {
            fn with_meta(mut self, meta: Meta) -> Self {
                self._meta = meta;
                self
            }
        }

        impl<$expr_ty: Instr> InstrOperationNode<$expr_ty> for $name<$expr_ty> {
            type Mapped<T: Instr> = $name<T>;

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

            fn map_op<T>(self, f: &mut impl FnMut($expr_ty) -> T) -> Self::Mapped<T>
            where
                T: Instr,
                InstrName<T>: From<InstrName<$expr_ty>>,
            {
                #[allow(unused_variables)]
                let _ = &f;
                define_operation!(@build_mapped [$name::<T>] [] self, f, $($raw_fields)*)
            }

            fn try_map_op<T, Error>(
                self,
                f: &mut impl FnMut($expr_ty) -> Result<T, Error>,
            ) -> Result<Self::Mapped<T>, Error>
            where
                T: Instr,
                InstrName<T>: From<InstrName<$expr_ty>>,
            {
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

        impl<E: Instr> InstrOperationNode<E> for $name {
            type Mapped<T: Instr> = $name;

            fn visit_exprs(&self, f: &mut impl FnMut(&E)) {
                let _ = &f;
            }

            fn visit_exprs_mut(&mut self, f: &mut impl FnMut(&mut E)) {
                let _ = &f;
            }

            fn map_op<T>(self, f: &mut impl FnMut(E) -> T) -> Self::Mapped<T>
            where
                T: Instr,
                InstrName<T>: From<InstrName<E>>,
            {
                let _ = &f;
                self
            }

            fn try_map_op<T, Error>(
                self,
                f: &mut impl FnMut(E) -> Result<T, Error>,
            ) -> Result<Self::Mapped<T>, Error>
            where
                T: Instr,
                InstrName<T>: From<InstrName<E>>,
            {
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

impl<E: Instr> InstrOperationNode<E> for Call<E> {
    type Mapped<T: Instr> = Call<T>;

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

    fn map_op<T>(self, f: &mut impl FnMut(E) -> T) -> Self::Mapped<T>
    where
        T: Instr,
        InstrName<T>: From<InstrName<E>>,
    {
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
    ) -> Result<Self::Mapped<T>, Error>
    where
        T: Instr,
        InstrName<T>: From<InstrName<E>>,
    {
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
        attr: Box<E>,
    }
}

define_operation! {
    pub struct SetAttr<E> {
        value: Box<E>,
        attr: Box<E>,
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

#[derive(Debug, Clone)]
pub struct Load<I: Instr> {
    _meta: Meta,
    pub name: InstrName<I>,
}

impl<I: Instr> Load<I> {
    pub fn new(name: impl Into<InstrName<I>>) -> Self {
        Self {
            _meta: Meta::default(),
            name: name.into(),
        }
    }
}

impl<I: Instr> HasMeta for Load<I> {
    fn meta(&self) -> Meta {
        self._meta.clone()
    }
}

impl<I: Instr> WithMeta for Load<I> {
    fn with_meta(mut self, meta: Meta) -> Self {
        self._meta = meta;
        self
    }
}

impl<I: Instr> InstrOperationNode<I> for Load<I> {
    type Mapped<T: Instr> = Load<T>;

    fn visit_exprs(&self, _f: &mut impl FnMut(&I)) {}

    fn visit_exprs_mut(&mut self, _f: &mut impl FnMut(&mut I)) {}

    fn map_op<T>(self, _f: &mut impl FnMut(I) -> T) -> Self::Mapped<T>
    where
        T: Instr,
        InstrName<T>: From<InstrName<I>>,
    {
        Load {
            _meta: self._meta,
            name: <T as Instr>::Name::from(self.name),
        }
    }

    fn try_map_op<T, Error>(
        self,
        _f: &mut impl FnMut(I) -> Result<T, Error>,
    ) -> Result<Self::Mapped<T>, Error>
    where
        T: Instr,
        InstrName<T>: From<InstrName<I>>,
    {
        Ok(Load {
            _meta: self._meta,
            name: <T as Instr>::Name::from(self.name),
        })
    }
}

#[derive(Debug, Clone)]
pub struct Store<I: Instr> {
    _meta: Meta,
    pub name: InstrName<I>,
    pub value: Box<I>,
}

impl<I: Instr> Store<I> {
    pub fn new(name: impl Into<InstrName<I>>, value: impl Into<Box<I>>) -> Self {
        Self {
            _meta: Meta::default(),
            name: name.into(),
            value: value.into(),
        }
    }
}

impl<I: Instr> HasMeta for Store<I> {
    fn meta(&self) -> Meta {
        self._meta.clone()
    }
}

impl<I: Instr> WithMeta for Store<I> {
    fn with_meta(mut self, meta: Meta) -> Self {
        self._meta = meta;
        self
    }
}

impl<I: Instr> InstrOperationNode<I> for Store<I> {
    type Mapped<T: Instr> = Store<T>;

    fn visit_exprs(&self, f: &mut impl FnMut(&I)) {
        f(&self.value);
    }

    fn visit_exprs_mut(&mut self, f: &mut impl FnMut(&mut I)) {
        f(&mut self.value);
    }

    fn map_op<T>(self, f: &mut impl FnMut(I) -> T) -> Self::Mapped<T>
    where
        T: Instr,
        InstrName<T>: From<InstrName<I>>,
    {
        Store {
            _meta: self._meta,
            name: <T as Instr>::Name::from(self.name),
            value: Box::new(f(*self.value)),
        }
    }

    fn try_map_op<T, Error>(
        self,
        f: &mut impl FnMut(I) -> Result<T, Error>,
    ) -> Result<Self::Mapped<T>, Error>
    where
        T: Instr,
        InstrName<T>: From<InstrName<I>>,
    {
        Ok(Store {
            _meta: self._meta,
            name: <T as Instr>::Name::from(self.name),
            value: Box::new(f(*self.value)?),
        })
    }
}

#[derive(Debug, Clone)]
pub struct Del<I: Instr> {
    _meta: Meta,
    pub name: InstrName<I>,
    pub quietly: bool,
}

impl<I: Instr> Del<I> {
    pub fn new(name: impl Into<InstrName<I>>, quietly: bool) -> Self {
        Self {
            _meta: Meta::default(),
            name: name.into(),
            quietly,
        }
    }
}

impl<I: Instr> HasMeta for Del<I> {
    fn meta(&self) -> Meta {
        self._meta.clone()
    }
}

impl<I: Instr> WithMeta for Del<I> {
    fn with_meta(mut self, meta: Meta) -> Self {
        self._meta = meta;
        self
    }
}

impl<I: Instr> InstrOperationNode<I> for Del<I> {
    type Mapped<T: Instr> = Del<T>;

    fn visit_exprs(&self, _f: &mut impl FnMut(&I)) {}

    fn visit_exprs_mut(&mut self, _f: &mut impl FnMut(&mut I)) {}

    fn map_op<T>(self, _f: &mut impl FnMut(I) -> T) -> Self::Mapped<T>
    where
        T: Instr,
        InstrName<T>: From<InstrName<I>>,
    {
        Del {
            _meta: self._meta,
            name: <T as Instr>::Name::from(self.name),
            quietly: self.quietly,
        }
    }

    fn try_map_op<T, Error>(
        self,
        _f: &mut impl FnMut(I) -> Result<T, Error>,
    ) -> Result<Self::Mapped<T>, Error>
    where
        T: Instr,
        InstrName<T>: From<InstrName<I>>,
    {
        Ok(Del {
            _meta: self._meta,
            name: <T as Instr>::Name::from(self.name),
            quietly: self.quietly,
        })
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

#[derive(Debug, Clone, derive_more::From, DelegateMatchDefault)]
pub enum OperationDetail<E: Instr> {
    BinOp(BinOp<E>),
    UnaryOp(UnaryOp<E>),
    Call(Call<E>),
    GetAttr(GetAttr<E>),
    SetAttr(SetAttr<E>),
    GetItem(GetItem<E>),
    SetItem(SetItem<E>),
    DelItem(DelItem<E>),
    LoadRuntime(LoadRuntime),
    Load(Load<E>),
    Store(Store<E>),
    Del(Del<E>),
    MakeCell(MakeCell<E>),
    MakeString(MakeString),
    CellRefForName(CellRefForName),
    CellRef(CellRef),
    MakeFunction(MakeFunction<E>),
}

#[with_match_default]
impl<E: Instr> HasMeta for OperationDetail<E> {
    fn meta(&self) -> Meta {
        match self {
            match_rest(op) => op.meta(),
        }
    }
}

#[with_match_default]
impl<E: Instr> OperationDetail<E> {
    pub fn map_expr<T>(self, f: &mut impl FnMut(E) -> T) -> OperationDetail<T>
    where
        T: Instr,
        InstrName<T>: From<InstrName<E>>,
    {
        match self {
            match_rest(op) => op.map_op(f).into(),
        }
    }

    pub fn try_map_expr<T, Error>(
        self,
        f: &mut impl FnMut(E) -> Result<T, Error>,
    ) -> Result<OperationDetail<T>, Error>
    where
        T: Instr,
        InstrName<T>: From<InstrName<E>>,
    {
        Ok(match self {
            match_rest(op) => op.try_map_op(f)?.into(),
        })
    }

    pub fn walk_args(&self, f: &mut impl FnMut(&E)) {
        match self {
            match_rest(op) => op.visit_exprs(f),
        }
    }

    pub fn walk_args_mut(&mut self, f: &mut impl FnMut(&mut E)) {
        match self {
            match_rest(op) => op.visit_exprs_mut(f),
        }
    }
}

#[with_match_default]
impl<E: Instr> WithMeta for OperationDetail<E> {
    fn with_meta(self, meta: Meta) -> Self {
        match self {
            match_rest(op) => op.with_meta(meta.clone()).into(),
        }
    }
}

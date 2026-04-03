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
        #[derive(Clone)]
        $vis struct $name<$expr_ty: Instr> {
            _meta: Meta,
            $($struct_fields)*
        }

        impl<$expr_ty: Instr> std::fmt::Debug for $name<$expr_ty> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                let mut debug = f.debug_struct(stringify!($name));
                define_operation!(@debug_fields debug, self, $($raw_fields)*);
                debug.finish()
            }
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

        impl<$expr_ty: Instr> InstrExprNode<$expr_ty> for $name<$expr_ty> {
            type Mapped<T: Instr> = $name<T>;

            fn visit_children(&self, f: &mut impl FnMut(&$expr_ty)) {
                #[allow(unused_variables)]
                let _ = &f;
                define_operation!(@visit_expr_fields self, f, $($raw_fields)*);
            }

            fn visit_children_mut(&mut self, f: &mut impl FnMut(&mut $expr_ty)) {
                #[allow(unused_variables)]
                let _ = &f;
                define_operation!(@visit_expr_fields_mut self, f, $($raw_fields)*);
            }

            fn map_children<T>(self, f: &mut impl FnMut($expr_ty) -> T) -> Self::Mapped<T>
            where
                T: Instr,
                InstrName<T>: From<InstrName<$expr_ty>>,
            {
                #[allow(unused_variables)]
                let _ = &f;
                define_operation!(@build_mapped [$name::<T>] [] self, f, $($raw_fields)*)
            }

            fn try_map_children<T, Error>(
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
            []
            $($fields)*
        );
    };
    (
        @collect_value_fields
        [$vis:vis]
        [$name:ident]
        [$($raw_fields:tt)*]
        [$($struct_fields:tt)*]
        [$($ctor_args:tt)*]
        [$($ctor_init:tt)*]
    ) => {
        #[derive(Clone)]
        $vis struct $name {
            _meta: Meta,
            $($struct_fields)*
        }

        impl std::fmt::Debug for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                let mut debug = f.debug_struct(stringify!($name));
                define_operation!(@debug_fields debug, self, $($raw_fields)*);
                debug.finish()
            }
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

        impl<E: Instr> InstrExprNode<E> for $name {
            type Mapped<T: Instr> = $name;

            fn visit_children(&self, f: &mut impl FnMut(&E)) {
                let _ = &f;
            }

            fn visit_children_mut(&mut self, f: &mut impl FnMut(&mut E)) {
                let _ = &f;
            }

            fn map_children<T>(self, f: &mut impl FnMut(E) -> T) -> Self::Mapped<T>
            where
                T: Instr,
                InstrName<T>: From<InstrName<E>>,
            {
                let _ = &f;
                self
            }

            fn try_map_children<T, Error>(
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
        [$($raw_fields:tt)*]
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
            [$($raw_fields)* $field: $ty,]
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
        [$($raw_fields:tt)*]
        [$($struct_fields:tt)*]
        [$($ctor_args:tt)*]
        [$($ctor_init:tt)*]
        $field:ident : $ty:ty
    ) => {
        define_operation!(
            @collect_value_fields
            [$vis]
            [$name]
            [$($raw_fields)* $field: $ty,]
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
    (@debug_fields $builder:ident, $self:ident,) => {};
    (@debug_fields $builder:ident, $self:ident, $field:ident : Box<$expr_ty:ident>, $($rest:tt)*) => {
        $builder.field(stringify!($field), &$self.$field);
        define_operation!(@debug_fields $builder, $self, $($rest)*);
    };
    (@debug_fields $builder:ident, $self:ident, $field:ident : Box<$expr_ty:ident>) => {
        $builder.field(stringify!($field), &$self.$field);
    };
    (@debug_fields $builder:ident, $self:ident, $field:ident : $ty:ty, $($rest:tt)*) => {
        $builder.field(stringify!($field), &$self.$field);
        define_operation!(@debug_fields $builder, $self, $($rest)*);
    };
    (@debug_fields $builder:ident, $self:ident, $field:ident : $ty:ty) => {
        $builder.field(stringify!($field), &$self.$field);
    };
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

pub(crate) use define_operation;

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

macro_rules! define_operations {
    ($( $variant:ident => $name:literal { $( $field:ident : $field_ty:ty ),* $(,)? } ),+ $(,)?) => {
        #[derive(Debug, Clone)]
        pub enum Operation<E> {
            $(
                $variant {
                    node_index: ast::AtomicNodeIndex,
                    range: TextRange,
                    $( $field: $field_ty, )*
                }
            ),+
        }

        impl<E> Operation<E> {
            pub fn helper_name(&self) -> &'static str {
                match self {
                    $( Self::$variant { .. } => $name, )+
                }
            }

            pub fn node_index(&self) -> &ast::AtomicNodeIndex {
                match self {
                    $( Self::$variant { node_index, .. } => node_index, )+
                }
            }

            pub fn range(&self) -> TextRange {
                match self {
                    $( Self::$variant { range, .. } => *range, )+
                }
            }

            pub fn map_expr<T>(self, f: &mut impl FnMut(E) -> T) -> Operation<T> {
                match self {
                    $(
                        Self::$variant {
                            node_index,
                            range,
                            $( $field, )*
                        } => Operation::$variant {
                            node_index,
                            range,
                            $( $field: <$field_ty as OperationArg<E>>::map_operation_arg($field, f), )*
                        },
                    )+
                }
            }

            pub fn try_map_expr<T, Error>(
                self,
                f: &mut impl FnMut(E) -> Result<T, Error>,
            ) -> Result<Operation<T>, Error> {
                match self {
                    $(
                        Self::$variant {
                            node_index,
                            range,
                            $( $field, )*
                        } => Ok(Operation::$variant {
                            node_index,
                            range,
                            $( $field: <$field_ty as OperationArg<E>>::try_map_operation_arg($field, f)?, )*
                        }),
                    )+
                }
            }

            pub fn walk_args(&self, f: &mut impl FnMut(&E)) {
                match self {
                    $(
                        Self::$variant { $( $field, )* .. } => {
                            $( <$field_ty as OperationArg<E>>::walk_operation_arg($field, f); )*
                        }
                    )+
                }
            }

            pub fn walk_args_mut(&mut self, f: &mut impl FnMut(&mut E)) {
                match self {
                    $(
                        Self::$variant { $( $field, )* .. } => {
                            $( <$field_ty as OperationArg<E>>::walk_operation_arg_mut($field, f); )*
                        }
                    )+
                }
            }

            pub fn into_call_args(self) -> Vec<E> {
                let mut out = Vec::new();
                match self {
                    $(
                        Self::$variant { $( $field, )* .. } => {
                            $( <$field_ty as OperationArg<E>>::push_operation_args($field, &mut out); )*
                        }
                    )+
                }
                out
            }

            pub fn call_args(&self) -> Vec<&E> {
                let mut out = Vec::new();
                match self {
                    $(
                        Self::$variant { $( $field, )* .. } => {
                            $( <$field_ty as OperationArg<E>>::push_operation_arg_refs($field, &mut out); )*
                        }
                    )+
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
            let operation = match name {
                $(
                    $name => Operation::$variant {
                        node_index,
                        range,
                        $( $field: <$field_ty as OperationArg<E>>::take_operation_arg(&mut args)?, )*
                    },
                )+
                _ => return None,
            };
            if args.next().is_some() {
                return None;
            }
            Some(operation)
        }
    };
}

define_operations!(
    Add => "__dp_add" { arg0: E, arg1: E },
    GetAttr => "__dp_getattr" { arg0: E, arg1: E },
    SetAttr => "__dp_setattr" { arg0: E, arg1: E, arg2: E },
    GetItem => "__dp_getitem" { arg0: E, arg1: E },
    SetItem => "__dp_setitem" { arg0: E, arg1: E, arg2: E },
    DelItem => "__dp_delitem" { arg0: E, arg1: E },
    LoadGlobal => "__dp_load_global" { arg0: E, arg1: E },
    StoreGlobal => "__dp_store_global" { arg0: E, arg1: E, arg2: E },
    LoadCell => "__dp_load_cell" { arg0: E },
    MakeCell => "__dp_make_cell" { arg0: E },
    CellRef => "__dp_cell_ref" { arg0: E },
    StoreCell => "__dp_store_cell" { arg0: E, arg1: E },
    DelQuietly => "__dp_del_quietly" { arg0: E, arg1: E },
    DelDerefQuietly => "__dp_del_deref_quietly" { arg0: E },
    DelDeref => "__dp_del_deref" { arg0: E },
    Sub => "__dp_sub" { arg0: E, arg1: E },
    Mul => "__dp_mul" { arg0: E, arg1: E },
    MatMul => "__dp_matmul" { arg0: E, arg1: E },
    TrueDiv => "__dp_truediv" { arg0: E, arg1: E },
    FloorDiv => "__dp_floordiv" { arg0: E, arg1: E },
    Mod => "__dp_mod" { arg0: E, arg1: E },
    Pow => "__dp_pow" { arg0: E, arg1: E, arg2: Option<E> },
    LShift => "__dp_lshift" { arg0: E, arg1: E },
    RShift => "__dp_rshift" { arg0: E, arg1: E },
    Or => "__dp_or_" { arg0: E, arg1: E },
    Xor => "__dp_xor" { arg0: E, arg1: E },
    And => "__dp_and_" { arg0: E, arg1: E },
    IAdd => "__dp_iadd" { arg0: E, arg1: E },
    ISub => "__dp_isub" { arg0: E, arg1: E },
    IMul => "__dp_imul" { arg0: E, arg1: E },
    IMatMul => "__dp_imatmul" { arg0: E, arg1: E },
    ITrueDiv => "__dp_itruediv" { arg0: E, arg1: E },
    IFloorDiv => "__dp_ifloordiv" { arg0: E, arg1: E },
    IMod => "__dp_imod" { arg0: E, arg1: E },
    IPow => "__dp_ipow" { arg0: E, arg1: E, arg2: Option<E> },
    ILShift => "__dp_ilshift" { arg0: E, arg1: E },
    IRShift => "__dp_irshift" { arg0: E, arg1: E },
    IOr => "__dp_ior" { arg0: E, arg1: E },
    IXor => "__dp_ixor" { arg0: E, arg1: E },
    IAnd => "__dp_iand" { arg0: E, arg1: E },
    Pos => "__dp_pos" { arg0: E },
    Neg => "__dp_neg" { arg0: E },
    Invert => "__dp_invert" { arg0: E },
    Not => "__dp_not_" { arg0: E },
    Truth => "__dp_truth" { arg0: E },
    Eq => "__dp_eq" { arg0: E, arg1: E },
    Ne => "__dp_ne" { arg0: E, arg1: E },
    Lt => "__dp_lt" { arg0: E, arg1: E },
    Le => "__dp_le" { arg0: E, arg1: E },
    Gt => "__dp_gt" { arg0: E, arg1: E },
    Ge => "__dp_ge" { arg0: E, arg1: E },
    Contains => "__dp_contains" { arg0: E, arg1: E },
    Is => "__dp_is_" { arg0: E, arg1: E },
    IsNot => "__dp_is_not" { arg0: E, arg1: E }
);

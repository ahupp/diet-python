use std::any::Any;
use std::fmt;

pub trait Intrinsic: Any + fmt::Debug + Sync {
    fn name(&self) -> &'static str;

    fn arity(&self) -> usize;

    fn as_any(&self) -> &dyn Any;
}

#[derive(Debug)]
pub struct AddIntrinsic;

impl Intrinsic for AddIntrinsic {
    fn name(&self) -> &'static str {
        "__dp_add"
    }

    fn arity(&self) -> usize {
        2
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub static ADD_INTRINSIC: AddIntrinsic = AddIntrinsic;

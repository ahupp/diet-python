use std::any::Any;
use std::fmt;

pub trait Intrinsic: Any + fmt::Debug + Sync {
    fn name(&self) -> &'static str;

    fn as_any(&self) -> &dyn Any;
}

#[derive(Debug)]
pub struct AddIntrinsic;

impl Intrinsic for AddIntrinsic {
    fn name(&self) -> &'static str {
        "__dp_add"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub static ADD_INTRINSIC: AddIntrinsic = AddIntrinsic;

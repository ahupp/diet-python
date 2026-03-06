#![allow(unsafe_op_in_unsafe_fn)]
#![allow(unused_unsafe)]

pub mod jit;
pub mod tree_walk;

include!(concat!(env!("OUT_DIR"), "/soac_clif.rs"));

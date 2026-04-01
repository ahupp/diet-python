#![allow(unsafe_op_in_unsafe_fn)]
#![allow(unused_unsafe)]

pub mod jit;
pub mod module_constants;
pub mod module_type;
pub mod session;
pub mod tree_walk;

include!(concat!(env!("OUT_DIR"), "/soac_clif.rs"));

pub use session::{CompileSession, CompileSessionId, allocate_compile_session_id};

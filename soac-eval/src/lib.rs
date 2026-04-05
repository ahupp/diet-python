#![allow(unsafe_op_in_unsafe_fn)]
#![allow(unused_unsafe)]

include!(concat!(env!("OUT_DIR"), "/soac_runtime_clif.rs"));

#[cfg(test)]
use std::sync::{Mutex, OnceLock};
pub mod counter;
pub mod counter_dump;
pub mod jit;
pub mod module_constants;
pub mod module_globals;
pub mod module_type;
pub mod session;
pub mod tree_walk;

pub use session::{CompileSession, CompileSessionId, allocate_compile_session_id};

#[cfg(test)]
pub(crate) fn python_runtime_test_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

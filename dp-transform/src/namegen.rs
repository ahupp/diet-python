use std::cell::RefCell;
use std::collections::HashSet;
use std::sync::atomic::{AtomicUsize, Ordering};

thread_local! {
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    static CLASS_NS_GLOBAL: RefCell<HashSet<String>> = RefCell::new(HashSet::new());
}

pub(crate) fn fresh_name(prefix: &str) -> String {
    let id = COUNTER.with(|counter| counter.fetch_add(1, Ordering::Relaxed) + 1);
    format!("_dp_{prefix}_{id}")
}

pub(crate) fn reset_namegen_state() {
    COUNTER.with(|counter| counter.store(0, Ordering::Relaxed));
}

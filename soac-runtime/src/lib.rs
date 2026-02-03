#![no_std]

extern crate alloc;

/*

__add__(PyLong, *)
__add__(int, int)


*/


use core::ffi::c_long;
use pyo3_ffi::{PyLong_CheckExact, PyLong_FromLong, PyNumber_Add, PyObject};

#[allow(
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    dead_code
)]
mod cpython_bindings;

use cpython_bindings::{
    PyLongObject, PyLong_SHIFT, _PyLong_NON_SIZE_BITS, _PyLong_SIGN_MASK,
};

pub use scope::{Scope, ScopeLayout};

#[inline]
unsafe fn compact_long_value(op: *mut PyLongObject) -> Option<i64> {
    let tag = unsafe { (*op).long_value.lv_tag as u64 };
    let compact_limit = 2u64 << (_PyLong_NON_SIZE_BITS as u64);
    if tag >= compact_limit {
        return None;
    }

    let sign = 1i64 - ((tag & _PyLong_SIGN_MASK as u64) as i64);
    let digit = unsafe { (*op).long_value.ob_digit[0] as i64 };
    Some(sign * digit)
}

/* ---------- FAST PATH ---------- */

#[inline]
unsafe fn try_fast_add(
    a: *mut PyObject,
    b: *mut PyObject,
) -> Option<*mut PyObject> {
    if unsafe { PyLong_CheckExact(a) } == 0 || unsafe { PyLong_CheckExact(b) } == 0 {
        return None;
    }

    let la = a as *mut PyLongObject;
    let lb = b as *mut PyLongObject;

    let va = unsafe { compact_long_value(la)? };
    let vb = unsafe { compact_long_value(lb)? };

    let sum = va.checked_add(vb)?;

    /* must still fit in one digit */
    if sum.abs() >= (1i64 << (PyLong_SHIFT as i64)) {
        return None;
    }

    Some(unsafe { PyLong_FromLong(sum as c_long) })
}


#[unsafe(no_mangle)]
pub unsafe extern "C" fn fast_int_add(
    a: *mut PyObject,
    b: *mut PyObject,
) -> *mut PyObject {
    if let Some(res) = unsafe { try_fast_add(a, b) } {
        return res;
    }

    /* slow path */
    unsafe { PyNumber_Add(a, b) }
}


// #[panic_handler]
// fn panic(_info: &PanicInfo) -> ! {
//     core::intrinsics::abort()
// }

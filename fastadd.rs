#![feature(core_intrinsics)]
#![no_std]

use core::panic::PanicInfo;
use core::ffi::c_void;

/* ---------- C ABI TYPES ---------- */

#[repr(C)]
pub struct PyObject {
    ob_refcnt: isize,
    ob_type: *mut PyTypeObject,
}

#[repr(C)]
pub struct PyTypeObject {
    _private: [u8; 0],
}

/* CPython PyLongObject (minimal, stable fields only) */
#[repr(C)]
pub struct PyLongObject {
    ob_base: PyObject,
    ob_size: isize,        // digit count incl. sign
    ob_digit: [u32; 1],   // variable-length
}

/* ---------- EXTERNAL PYTHON SYMBOLS ---------- */

extern "C" {
    /* Global type object */
    static mut PyLong_Type: PyTypeObject;

    /* Public C-API functions */
    pub fn PyLong_Add(a: *mut PyObject, b: *mut PyObject) -> *mut PyObject;
    pub fn PyLong_FromLong(val: isize) -> *mut PyObject;
}

/* ---------- PyLong_Check DEFINITION ---------- */

#[inline]
pub unsafe fn PyLong_Check(op: *mut PyObject) -> bool {
    if op.is_null() {
        return false;
    }
    (*op).ob_type == &mut PyLong_Type
}

/* ---------- CONSTANTS ---------- */

const PYLONG_BASE_BITS: u32 = 30;
const PYLONG_BASE: i64 = 1 << PYLONG_BASE_BITS;

/* ---------- FAST PATH ---------- */

#[inline]
unsafe fn try_fast_add(
    a: *mut PyObject,
    b: *mut PyObject,
) -> Option<*mut PyObject> {
    if !PyLong_Check(a) || !PyLong_Check(b) {
        return None;
    }

    let la = a as *mut PyLongObject;
    let lb = b as *mut PyLongObject;

    let sa = (*la).ob_size;
    let sb = (*lb).ob_size;

    /* must be compact (single digit) */
    if sa.abs() != 1 || sb.abs() != 1 {
        return None;
    }

    let da = (*la).ob_digit[0] as i64;
    let db = (*lb).ob_digit[0] as i64;

    let va = if sa < 0 { -da } else { da };
    let vb = if sb < 0 { -db } else { db };

    let sum = va.checked_add(vb)?;

    /* must still fit in one digit */
    if sum.abs() >= PYLONG_BASE {
        return None;
    }

    Some(PyLong_FromLong(sum as isize))
}

/* ---------- PUBLIC ENTRY POINT ---------- */

#[no_mangle]
pub unsafe extern "C" fn fast_int_add(
    a: *mut PyObject,
    b: *mut PyObject,
) -> *mut PyObject {
    if let Some(res) = try_fast_add(a, b) {
        return res;
    }

    /* slow path */
    PyLong_Add(a, b)
}


#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    core::intrinsics::abort()
}

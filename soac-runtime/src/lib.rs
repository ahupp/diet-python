#![no_std]

use core::ffi::c_void;

#[repr(C)]
#[cfg(all(target_pointer_width = "64", target_endian = "little"))]
#[derive(Clone, Copy)]
union PyObjectObRefcnt {
    ob_refcnt_full: i64,
    refcnt_and_flags: PyObjectObFlagsAndRefcnt,
}

#[repr(C)]
#[cfg(all(target_pointer_width = "64", target_endian = "big"))]
#[derive(Clone, Copy)]
union PyObjectObRefcnt {
    ob_refcnt_full: i64,
    refcnt_and_flags: PyObjectObFlagsAndRefcnt,
}

#[repr(C)]
#[cfg(all(target_pointer_width = "64", target_endian = "little"))]
#[derive(Clone, Copy)]
struct PyObjectObFlagsAndRefcnt {
    ob_refcnt: u32,
    ob_overflow: u16,
    ob_flags: u16,
}

#[repr(C)]
#[cfg(all(target_pointer_width = "64", target_endian = "big"))]
#[derive(Clone, Copy)]
struct PyObjectObFlagsAndRefcnt {
    ob_flags: u16,
    ob_overflow: u16,
    ob_refcnt: u32,
}

#[repr(C)]
struct PyObject {
    #[cfg(target_pointer_width = "64")]
    ob_refcnt: PyObjectObRefcnt,
    #[cfg(target_pointer_width = "32")]
    ob_refcnt: isize,
    ob_type: *mut c_void,
}

unsafe extern "C" {
    fn _Py_Dealloc(obj: *mut PyObject);
}

#[inline(always)]
unsafe fn can_skip_incref(obj: *mut PyObject) -> bool {
    #[cfg(target_pointer_width = "64")]
    {
        const PY_IMMORTAL_INITIAL_REFCNT: u32 = 3u32 << 30;
        unsafe { (*obj).ob_refcnt.refcnt_and_flags.ob_refcnt >= PY_IMMORTAL_INITIAL_REFCNT }
    }

    #[cfg(target_pointer_width = "32")]
    {
        const PY_IMMORTAL_MINIMUM_REFCNT: isize = 1isize << 30;
        unsafe { (*obj).ob_refcnt >= PY_IMMORTAL_MINIMUM_REFCNT }
    }
}

#[inline(always)]
unsafe fn can_skip_decref(obj: *mut PyObject) -> bool {
    #[cfg(target_pointer_width = "64")]
    {
        unsafe { ((*obj).ob_refcnt.refcnt_and_flags.ob_refcnt as i32) < 0 }
    }

    #[cfg(target_pointer_width = "32")]
    {
        const PY_IMMORTAL_MINIMUM_REFCNT: isize = 1isize << 30;
        unsafe { (*obj).ob_refcnt >= PY_IMMORTAL_MINIMUM_REFCNT }
    }
}

#[inline(always)]
unsafe fn incref_impl(obj: *mut PyObject) {
    if obj.is_null() || unsafe { can_skip_incref(obj) } {
        return;
    }

    #[cfg(target_pointer_width = "64")]
    unsafe {
        let cur_refcnt = (*obj).ob_refcnt.refcnt_and_flags.ob_refcnt;
        (*obj).ob_refcnt.refcnt_and_flags.ob_refcnt = cur_refcnt.wrapping_add(1);
    }

    #[cfg(target_pointer_width = "32")]
    unsafe {
        (*obj).ob_refcnt = (*obj).ob_refcnt.wrapping_add(1);
    }
}

#[inline(always)]
unsafe fn decref_impl(obj: *mut PyObject) {
    if obj.is_null() || unsafe { can_skip_decref(obj) } {
        return;
    }

    #[cfg(target_pointer_width = "64")]
    unsafe {
        let next_refcnt = (*obj).ob_refcnt.refcnt_and_flags.ob_refcnt.wrapping_sub(1);
        (*obj).ob_refcnt.refcnt_and_flags.ob_refcnt = next_refcnt;
        if next_refcnt == 0 {
            _Py_Dealloc(obj);
        }
    }

    #[cfg(target_pointer_width = "32")]
    unsafe {
        let next_refcnt = (*obj).ob_refcnt.wrapping_sub(1);
        (*obj).ob_refcnt = next_refcnt;
        if next_refcnt == 0 {
            _Py_Dealloc(obj);
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn soac_runtime_incref(obj: *mut c_void) {
    unsafe { incref_impl(obj.cast::<PyObject>()) };
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn soac_runtime_decref(obj: *mut c_void) {
    unsafe { decref_impl(obj.cast::<PyObject>()) };
}

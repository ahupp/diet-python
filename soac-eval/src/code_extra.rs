use pyo3::ffi;
use std::ffi::c_void;
use std::os::raw::c_int;
use std::ptr;
use std::sync::Once;

unsafe extern "C" {
    fn PyUnstable_Eval_RequestCodeExtraIndex(f: ffi::freefunc) -> ffi::Py_ssize_t;
    fn PyUnstable_Code_GetExtra(
        code: *mut ffi::PyObject,
        index: ffi::Py_ssize_t,
        extra: *mut *mut c_void,
    ) -> c_int;
    fn PyUnstable_Code_SetExtra(
        code: *mut ffi::PyObject,
        index: ffi::Py_ssize_t,
        extra: *mut c_void,
    ) -> c_int;
}

pub(crate) const SOAC_CODE_EXTRA_KIND_CLIF_WRAPPER: u64 = 1;

const SOAC_CODE_EXTRA_MAGIC: u64 = 0x4450_5f53_4f41_435f;

static INIT_CODE_EXTRA_INDEX: Once = Once::new();
static mut SOAC_CODE_EXTRA_INDEX: ffi::Py_ssize_t = -1;

pub(crate) type CodeExtraDataFreeFn = unsafe extern "C" fn(*mut c_void);

#[repr(C)]
struct SoacCodeExtra {
    magic: u64,
    kind: u64,
    data: *mut c_void,
    free_data: Option<CodeExtraDataFreeFn>,
}

impl Drop for SoacCodeExtra {
    fn drop(&mut self) {
        if self.data.is_null() {
            return;
        }
        if let Some(free_data) = self.free_data {
            unsafe {
                free_data(self.data);
            }
        }
        self.data = ptr::null_mut();
    }
}

pub(crate) struct CodeExtraView {
    pub(crate) kind: u64,
    pub(crate) data: *mut c_void,
}

unsafe extern "C" fn soac_code_extra_free(ptr: *mut c_void) {
    if ptr.is_null() {
        return;
    }
    drop(unsafe { Box::from_raw(ptr as *mut SoacCodeExtra) });
}

pub(crate) unsafe fn code_extra_index() -> Result<ffi::Py_ssize_t, ()> {
    if unsafe { SOAC_CODE_EXTRA_INDEX } >= 0 {
        return Ok(unsafe { SOAC_CODE_EXTRA_INDEX });
    }
    INIT_CODE_EXTRA_INDEX.call_once(|| {
        let index = unsafe { PyUnstable_Eval_RequestCodeExtraIndex(soac_code_extra_free) };
        if index >= 0 {
            unsafe {
                SOAC_CODE_EXTRA_INDEX = index as ffi::Py_ssize_t;
            }
        }
    });
    if unsafe { SOAC_CODE_EXTRA_INDEX } < 0 {
        return Err(());
    }
    Ok(unsafe { SOAC_CODE_EXTRA_INDEX })
}

pub(crate) unsafe fn get_code_extra(code: *mut ffi::PyObject) -> Option<CodeExtraView> {
    if unsafe { ffi::PyObject_TypeCheck(code, std::ptr::addr_of_mut!(ffi::PyCode_Type)) } == 0 {
        return None;
    }
    let index = match unsafe { code_extra_index() } {
        Ok(index) => index,
        Err(()) => return None,
    };
    let mut extra: *mut c_void = ptr::null_mut();
    let status = unsafe { PyUnstable_Code_GetExtra(code, index, &mut extra as *mut *mut c_void) };
    if status != 0 || extra.is_null() {
        return None;
    }
    let tagged = extra as *const SoacCodeExtra;
    if tagged.is_null() || unsafe { (*tagged).magic != SOAC_CODE_EXTRA_MAGIC } {
        return None;
    }
    Some(CodeExtraView {
        kind: unsafe { (*tagged).kind },
        data: unsafe { (*tagged).data },
    })
}

pub(crate) unsafe fn set_code_extra(
    code: *mut ffi::PyObject,
    kind: u64,
    data: *mut c_void,
    free_data: Option<CodeExtraDataFreeFn>,
) -> Result<(), ()> {
    let index = unsafe { code_extra_index() }?;
    let code_extra = Box::new(SoacCodeExtra {
        magic: SOAC_CODE_EXTRA_MAGIC,
        kind,
        data,
        free_data,
    });
    let code_extra_ptr = Box::into_raw(code_extra);
    if unsafe { PyUnstable_Code_SetExtra(code, index, code_extra_ptr as *mut c_void) } != 0 {
        drop(unsafe { Box::from_raw(code_extra_ptr) });
        return Err(());
    }
    Ok(())
}

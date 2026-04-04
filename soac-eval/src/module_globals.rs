use pyo3::ffi;
use std::collections::HashMap;
use std::ffi::c_int;
use std::ptr;
use std::sync::atomic::{AtomicPtr, Ordering};
use std::sync::{Arc, Mutex, OnceLock, Weak};

type ObjPtr = *mut ffi::PyObject;

unsafe extern "C" {
    fn PyDict_AddWatcher(
        callback: unsafe extern "C" fn(c_int, *mut ffi::PyObject, *mut ffi::PyObject, *mut ffi::PyObject) -> c_int,
    ) -> c_int;
    fn PyDict_ClearWatcher(watcher_id: c_int) -> c_int;
    fn PyDict_Watch(watcher_id: c_int, dict: *mut ffi::PyObject) -> c_int;
    fn PyDict_Unwatch(watcher_id: c_int, dict: *mut ffi::PyObject) -> c_int;
}

fn watcher_id() -> Option<c_int> {
    static WATCHER_ID: OnceLock<Option<c_int>> = OnceLock::new();
    *WATCHER_ID.get_or_init(|| {
        let watcher_id = unsafe { PyDict_AddWatcher(dict_watcher_callback) };
        (watcher_id >= 0).then_some(watcher_id)
    })
}

fn cache_registry() -> &'static Mutex<HashMap<usize, Weak<ModuleGlobalCache>>> {
    static REGISTRY: OnceLock<Mutex<HashMap<usize, Weak<ModuleGlobalCache>>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

unsafe extern "C" fn dict_watcher_callback(
    _event: c_int,
    dict: *mut ffi::PyObject,
    _key: *mut ffi::PyObject,
    _new_value: *mut ffi::PyObject,
) -> c_int {
    if dict.is_null() {
        return 0;
    }
    let maybe_cache = {
        let registry = cache_registry()
            .lock()
            .expect("module global cache registry mutex poisoned");
        registry.get(&(dict as usize)).and_then(Weak::upgrade)
    };
    if let Some(cache) = maybe_cache {
        unsafe { cache.clear_all() };
    }
    0
}

pub struct ModuleGlobalCache {
    dict_obj: ObjPtr,
    slots: Box<[AtomicPtr<ffi::PyObject>]>,
}

unsafe impl Send for ModuleGlobalCache {}
unsafe impl Sync for ModuleGlobalCache {}

impl ModuleGlobalCache {
    pub unsafe fn new(dict_obj: ObjPtr, slot_count: usize) -> Result<Arc<Self>, ()> {
        if dict_obj.is_null() {
            ffi::PyErr_SetString(
                ffi::PyExc_RuntimeError,
                b"invalid module global cache initialization\0".as_ptr() as *const i8,
            );
            return Err(());
        }
        let slots = (0..slot_count)
            .map(|_| AtomicPtr::new(ptr::null_mut()))
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let cache = Arc::new(Self { dict_obj, slots });
        let Some(watcher_id) = watcher_id() else {
            ffi::PyErr_SetString(
                ffi::PyExc_RuntimeError,
                b"failed to allocate dict watcher for module globals\0".as_ptr() as *const i8,
            );
            return Err(());
        };
        cache_registry()
            .lock()
            .expect("module global cache registry mutex poisoned")
            .insert(dict_obj as usize, Arc::downgrade(&cache));
        if PyDict_Watch(watcher_id, dict_obj) != 0 {
            cache_registry()
                .lock()
                .expect("module global cache registry mutex poisoned")
                .remove(&(dict_obj as usize));
            return Err(());
        }
        Ok(cache)
    }

    pub fn slots_ptr(&self) -> *mut ffi::PyObject {
        self.slots.as_ptr().cast_mut().cast::<ffi::PyObject>()
    }

    pub unsafe fn store_loaded_value_steal(&self, slot: u32, value: ObjPtr) {
        self.swap_slot(slot, value);
    }

    pub unsafe fn clear_all(&self) {
        for entry in &self.slots {
            let old = entry.swap(ptr::null_mut(), Ordering::AcqRel);
            if !old.is_null() {
                ffi::Py_DECREF(old);
            }
        }
    }

    unsafe fn swap_slot(&self, slot: u32, value: ObjPtr) {
        let Some(entry) = self.slots.get(slot as usize) else {
            return;
        };
        let old = entry.swap(value, Ordering::AcqRel);
        if !old.is_null() {
            ffi::Py_DECREF(old);
        }
    }
}

impl Drop for ModuleGlobalCache {
    fn drop(&mut self) {
        if let Some(watcher_id) = watcher_id() {
            unsafe {
                PyDict_Unwatch(watcher_id, self.dict_obj);
            }
        }
        cache_registry()
            .lock()
            .expect("module global cache registry mutex poisoned")
            .remove(&(self.dict_obj as usize));
        unsafe {
            self.clear_all();
        }
    }
}

#[allow(dead_code)]
pub unsafe fn clear_module_global_watcher_for_testing() {
    if let Some(watcher_id) = watcher_id() {
        let _ = unsafe { PyDict_ClearWatcher(watcher_id) };
    }
}

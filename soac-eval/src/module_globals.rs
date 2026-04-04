use pyo3::ffi;
use std::ffi::CStr;
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

const PYDICT_EVENT_ADDED: c_int = 0;
const PYDICT_EVENT_MODIFIED: c_int = 1;
const PYDICT_EVENT_DELETED: c_int = 2;
const PYDICT_EVENT_CLONED: c_int = 3;
const PYDICT_EVENT_CLEARED: c_int = 4;
const PYDICT_EVENT_DEALLOCATED: c_int = 5;

unsafe extern "C" fn dict_watcher_callback(
    event: c_int,
    dict: *mut ffi::PyObject,
    key: *mut ffi::PyObject,
    new_value: *mut ffi::PyObject,
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
        unsafe {
            cache.handle_dict_watcher_event(event, key, new_value);
        }
    }
    0
}

pub struct ModuleGlobalCache {
    dict_obj: ObjPtr,
    slots: Box<[AtomicPtr<ffi::PyObject>]>,
    slot_by_name: HashMap<String, u32>,
    pending_self_updates: Mutex<Vec<Vec<ObjPtr>>>,
}

unsafe impl Send for ModuleGlobalCache {}
unsafe impl Sync for ModuleGlobalCache {}

impl ModuleGlobalCache {
    pub fn lookup(dict_obj: ObjPtr) -> Option<Arc<Self>> {
        if dict_obj.is_null() {
            return None;
        }
        cache_registry()
            .lock()
            .expect("module global cache registry mutex poisoned")
            .get(&(dict_obj as usize))
            .and_then(Weak::upgrade)
    }

    pub unsafe fn new(dict_obj: ObjPtr, global_names: &[String]) -> Result<Arc<Self>, ()> {
        if dict_obj.is_null() {
            ffi::PyErr_SetString(
                ffi::PyExc_RuntimeError,
                b"invalid module global cache initialization\0".as_ptr() as *const i8,
            );
            return Err(());
        }
        let slots = (0..global_names.len())
            .map(|_| AtomicPtr::new(ptr::null_mut()))
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let mut slot_by_name = HashMap::with_capacity(global_names.len());
        for (slot, name) in global_names.iter().enumerate() {
            slot_by_name.insert(
                name.clone(),
                u32::try_from(slot).expect("global slot index should fit in u32"),
            );
        }
        let pending_self_updates = (0..global_names.len())
            .map(|_| Vec::new())
            .collect::<Vec<_>>();
        let cache = Arc::new(Self {
            dict_obj,
            slots,
            slot_by_name,
            pending_self_updates: Mutex::new(pending_self_updates),
        });
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

    pub unsafe fn store_global_write_through(&self, name: ObjPtr, slot: u32, value: ObjPtr) -> ObjPtr {
        if name.is_null() || value.is_null() {
            ffi::PyErr_SetString(
                ffi::PyExc_RuntimeError,
                b"invalid arguments to module global store cache write-through\0".as_ptr()
                    as *const i8,
            );
            return ptr::null_mut();
        }
        self.push_pending_self_update(slot, value);
        let rc = ffi::PyObject_SetItem(
            self.dict_obj as *mut ffi::PyObject,
            name as *mut ffi::PyObject,
            value as *mut ffi::PyObject,
        );
        if rc != 0 {
            self.remove_pending_self_update(slot, value);
            return ptr::null_mut();
        }
        self.store_borrowed_value(slot, value);
        ffi::Py_INCREF(value as *mut ffi::PyObject);
        value
    }

    pub unsafe fn del_global_write_through(&self, name: ObjPtr, slot: u32, quietly: bool) -> ObjPtr {
        if name.is_null() {
            ffi::PyErr_SetString(
                ffi::PyExc_RuntimeError,
                b"invalid arguments to module global delete cache write-through\0".as_ptr()
                    as *const i8,
            );
            return ptr::null_mut();
        }
        self.push_pending_self_update(slot, ptr::null_mut());
        let rc = ffi::PyObject_DelItem(
            self.dict_obj as *mut ffi::PyObject,
            name as *mut ffi::PyObject,
        );
        if rc != 0 {
            self.remove_pending_self_update(slot, ptr::null_mut());
            let suppress = quietly
                && (ffi::PyErr_ExceptionMatches(ffi::PyExc_NameError) != 0
                    || ffi::PyErr_ExceptionMatches(ffi::PyExc_KeyError) != 0);
            if !suppress {
                return ptr::null_mut();
            }
            ffi::PyErr_Clear();
        }
        self.clear_slot(slot);
        let none = ffi::Py_None();
        ffi::Py_INCREF(none);
        none as ObjPtr
    }

    pub unsafe fn clear_all(&self) {
        for entry in &self.slots {
            let old = entry.swap(ptr::null_mut(), Ordering::AcqRel);
            if !old.is_null() {
                ffi::Py_DECREF(old);
            }
        }
    }

    unsafe fn handle_dict_watcher_event(&self, event: c_int, key: ObjPtr, new_value: ObjPtr) {
        match event {
            PYDICT_EVENT_ADDED | PYDICT_EVENT_MODIFIED => {
                let Some(slot) = self.slot_for_key_obj(key) else {
                    return;
                };
                if self.try_ignore_pending_self_update(slot, new_value) {
                    return;
                }
                self.store_borrowed_value(slot, new_value);
            }
            PYDICT_EVENT_DELETED => {
                let Some(slot) = self.slot_for_key_obj(key) else {
                    return;
                };
                if self.try_ignore_pending_self_update(slot, ptr::null_mut()) {
                    return;
                }
                self.clear_slot(slot);
            }
            PYDICT_EVENT_CLONED | PYDICT_EVENT_CLEARED | PYDICT_EVENT_DEALLOCATED => {
                self.clear_all();
            }
            _ => {}
        }
    }

    unsafe fn store_borrowed_value(&self, slot: u32, value: ObjPtr) {
        if value.is_null() {
            self.clear_slot(slot);
            return;
        }
        ffi::Py_INCREF(value);
        self.swap_slot(slot, value);
    }

    unsafe fn clear_slot(&self, slot: u32) {
        self.swap_slot(slot, ptr::null_mut());
    }

    unsafe fn slot_for_key_obj(&self, key: ObjPtr) -> Option<u32> {
        if key.is_null() || ffi::PyUnicode_Check(key) == 0 {
            return None;
        }
        let key_utf8 = ffi::PyUnicode_AsUTF8(key);
        if key_utf8.is_null() {
            ffi::PyErr_Clear();
            return None;
        }
        let key_str = CStr::from_ptr(key_utf8).to_str().ok()?;
        self.slot_by_name.get(key_str).copied()
    }

    fn push_pending_self_update(&self, slot: u32, expected_new_value: ObjPtr) {
        let mut pending = self
            .pending_self_updates
            .lock()
            .expect("module global cache pending-self-updates mutex poisoned");
        let Some(slot_updates) = pending.get_mut(slot as usize) else {
            return;
        };
        slot_updates.push(expected_new_value);
    }

    fn remove_pending_self_update(&self, slot: u32, expected_new_value: ObjPtr) {
        let mut pending = self
            .pending_self_updates
            .lock()
            .expect("module global cache pending-self-updates mutex poisoned");
        let Some(slot_updates) = pending.get_mut(slot as usize) else {
            return;
        };
        if slot_updates.last().copied() == Some(expected_new_value) {
            slot_updates.pop();
        }
    }

    fn try_ignore_pending_self_update(&self, slot: u32, new_value: ObjPtr) -> bool {
        let mut pending = self
            .pending_self_updates
            .lock()
            .expect("module global cache pending-self-updates mutex poisoned");
        let Some(slot_updates) = pending.get_mut(slot as usize) else {
            return false;
        };
        if slot_updates.last().copied() == Some(new_value) {
            slot_updates.pop();
            true
        } else {
            false
        }
    }

    #[cfg(test)]
    fn has_pending_self_update(&self, slot: u32, expected_new_value: ObjPtr) -> bool {
        self.pending_self_updates
            .lock()
            .expect("module global cache pending-self-updates mutex poisoned")
            .get(slot as usize)
            .and_then(|slot_updates| slot_updates.last().copied())
            == Some(expected_new_value)
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

#[cfg(test)]
mod tests {
    use super::*;
    use pyo3::{Python, ffi};
    use std::os::raw::c_longlong;

    unsafe fn cached_long_value(cache: &ModuleGlobalCache, slot: u32) -> Option<i64> {
        let value = cache.slots.get(slot as usize)?.load(Ordering::Acquire);
        if value.is_null() {
            return None;
        }
        let out = ffi::PyLong_AsLongLong(value);
        assert!(ffi::PyErr_Occurred().is_null(), "cached slot should hold a PyLong");
        Some(out as i64)
    }

    #[test]
    fn external_dict_mutation_updates_only_matching_slot() {
        Python::attach(|py| unsafe {
            let globals = ffi::PyDict_New();
            assert!(!globals.is_null());
            {
                let cache = ModuleGlobalCache::new(globals, &["x".into(), "y".into()])
                    .expect("global cache should initialize");
                let x_name = ffi::PyUnicode_FromString(b"x\0".as_ptr() as *const i8);
                let y_name = ffi::PyUnicode_FromString(b"y\0".as_ptr() as *const i8);
                let x_value = ffi::PyLong_FromLongLong(1 as c_longlong);
                let y_value = ffi::PyLong_FromLongLong(2 as c_longlong);
                assert_eq!(ffi::PyObject_SetItem(globals, x_name, x_value), 0);
                assert_eq!(cached_long_value(&cache, 0), Some(1));
                assert_eq!(cached_long_value(&cache, 1), None);
                assert_eq!(ffi::PyObject_SetItem(globals, y_name, y_value), 0);
                assert_eq!(cached_long_value(&cache, 0), Some(1));
                assert_eq!(cached_long_value(&cache, 1), Some(2));
                ffi::Py_DECREF(x_name);
                ffi::Py_DECREF(y_name);
                ffi::Py_DECREF(x_value);
                ffi::Py_DECREF(y_value);
                drop(cache);
            }
            ffi::Py_DECREF(globals);
            let _ = py;
        });
    }

    #[test]
    fn write_through_store_and_delete_consume_self_watcher_events() {
        Python::attach(|py| unsafe {
            let globals = ffi::PyDict_New();
            assert!(!globals.is_null());
            {
                let cache = ModuleGlobalCache::new(globals, &["x".into()])
                    .expect("global cache should initialize");
                let x_name = ffi::PyUnicode_FromString(b"x\0".as_ptr() as *const i8);
                let first_value = ffi::PyLong_FromLongLong(7 as c_longlong);
                let stored = cache.store_global_write_through(x_name, 0, first_value);
                assert!(!stored.is_null());
                assert_eq!(cached_long_value(&cache, 0), Some(7));
                assert!(
                    !cache.has_pending_self_update(0, first_value),
                    "store should consume its own watcher callback"
                );
                ffi::Py_DECREF(stored);

                let second_value = ffi::PyLong_FromLongLong(9 as c_longlong);
                let replaced = cache.store_global_write_through(x_name, 0, second_value);
                assert!(!replaced.is_null());
                assert_eq!(cached_long_value(&cache, 0), Some(9));
                assert!(
                    !cache.has_pending_self_update(0, second_value),
                    "replace should consume its own watcher callback"
                );
                ffi::Py_DECREF(replaced);

                let deleted = cache.del_global_write_through(x_name, 0, false);
                assert!(!deleted.is_null());
                assert_eq!(cached_long_value(&cache, 0), None);
                assert!(
                    !cache.has_pending_self_update(0, ptr::null_mut()),
                    "delete should consume its own watcher callback"
                );

                ffi::Py_DECREF(first_value);
                ffi::Py_DECREF(second_value);
                ffi::Py_DECREF(deleted);
                ffi::Py_DECREF(x_name);
                drop(cache);
            }
            ffi::Py_DECREF(globals);
            let _ = py;
        });
    }
}

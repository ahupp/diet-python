use super::eval::set_name_error;
use super::*;

pub struct ScopeLayout {
    pub(crate) names: Vec<String>,
    map: HashMap<String, usize>,
}

impl ScopeLayout {
    pub fn new(names: HashSet<String>) -> Self {
        let mut names: Vec<String> = names.into_iter().collect();
        names.sort();
        let mut map = HashMap::new();
        for (idx, name) in names.iter().enumerate() {
            map.insert(name.clone(), idx + 1);
        }
        Self { names, map }
    }

    pub(crate) fn slot_for(&self, name: &str) -> Option<usize> {
        self.map.get(name).copied()
    }

    fn slot_count(&self) -> usize {
        self.names.len() + 1
    }
}

pub struct ScopeInstance {
    layout: *const ScopeLayout,
    pub(crate) slots: Vec<*mut ffi::PyObject>,
}

impl ScopeInstance {
    pub fn new(layout: *const ScopeLayout) -> Self {
        let count = unsafe { (*layout).slot_count() };
        Self {
            layout,
            slots: vec![ptr::null_mut(); count],
        }
    }

    pub(crate) fn layout(&self) -> &ScopeLayout {
        unsafe { &*self.layout }
    }
}

impl Drop for ScopeInstance {
    fn drop(&mut self) {
        unsafe {
            for slot in self.slots.iter_mut() {
                if !slot.is_null() {
                    ffi::Py_DECREF(*slot);
                    *slot = ptr::null_mut();
                }
            }
        }
    }
}

unsafe fn scope_ensure_dict(scope: &mut ScopeInstance) -> Result<*mut ffi::PyObject, ()> {
    if scope.slots[0].is_null() {
        let dict = ffi::PyDict_New();
        if dict.is_null() {
            return Err(());
        }
        scope.slots[0] = dict;
    }
    Ok(scope.slots[0])
}

pub(crate) unsafe fn scope_get_slot(scope: &ScopeInstance, slot: usize) -> *mut ffi::PyObject {
    let value = scope.slots[slot];
    if value.is_null() {
        return ptr::null_mut();
    }
    ffi::Py_INCREF(value);
    value
}

unsafe fn scope_set_slot(
    scope: &mut ScopeInstance,
    slot: usize,
    value: *mut ffi::PyObject,
) -> Result<(), ()> {
    if value.is_null() {
        return Err(());
    }
    if !scope.slots[slot].is_null() {
        ffi::Py_DECREF(scope.slots[slot]);
    }
    ffi::Py_INCREF(value);
    scope.slots[slot] = value;
    Ok(())
}

unsafe fn scope_clear_slot(scope: &mut ScopeInstance, slot: usize) -> Result<(), ()> {
    if scope.slots[slot].is_null() {
        return set_name_error(scope.layout().names[slot - 1].as_str());
    }
    ffi::Py_DECREF(scope.slots[slot]);
    scope.slots[slot] = ptr::null_mut();
    Ok(())
}

pub(crate) unsafe fn scope_get_dynamic(scope: &ScopeInstance, name: &str) -> *mut ffi::PyObject {
    let dict = scope.slots[0];
    if dict.is_null() {
        return ptr::null_mut();
    }
    let value = ffi::PyDict_GetItemString(dict, CString::new(name).unwrap().as_ptr());
    if value.is_null() {
        return ptr::null_mut();
    }
    ffi::Py_INCREF(value);
    value
}

unsafe fn scope_set_dynamic(
    scope: &mut ScopeInstance,
    name: &str,
    value: *mut ffi::PyObject,
) -> Result<(), ()> {
    let dict = scope_ensure_dict(scope)?;
    if ffi::PyDict_SetItemString(dict, CString::new(name).unwrap().as_ptr(), value) != 0 {
        return Err(());
    }
    Ok(())
}

unsafe fn scope_del_dynamic(scope: &mut ScopeInstance, name: &str) -> Result<(), ()> {
    let dict = scope.slots[0];
    if dict.is_null() {
        return set_name_error(name);
    }
    if ffi::PyDict_DelItemString(dict, CString::new(name).unwrap().as_ptr()) != 0 {
        return Err(());
    }
    Ok(())
}

pub unsafe fn scope_lookup_name(scope: &ScopeInstance, name: &str) -> *mut ffi::PyObject {
    if let Some(slot) = scope.layout().slot_for(name) {
        return scope_get_slot(scope, slot);
    }
    scope_get_dynamic(scope, name)
}

pub unsafe fn scope_assign_name(
    scope: &mut ScopeInstance,
    name: &str,
    value: *mut ffi::PyObject,
) -> Result<(), ()> {
    if let Some(slot) = scope.layout().slot_for(name) {
        scope_set_slot(scope, slot, value)
    } else {
        scope_set_dynamic(scope, name, value)
    }
}

pub unsafe fn scope_delete_name(scope: &mut ScopeInstance, name: &str) -> Result<(), ()> {
    if let Some(slot) = scope.layout().slot_for(name) {
        scope_clear_slot(scope, slot)
    } else {
        scope_del_dynamic(scope, name)
    }
}

pub unsafe fn scope_to_dict(scope: &ScopeInstance) -> Result<*mut ffi::PyObject, ()> {
    let dict = ffi::PyDict_New();
    if dict.is_null() {
        return Err(());
    }
    for (idx, name) in scope.layout().names.iter().enumerate() {
        let slot = idx + 1;
        let value = scope.slots[slot];
        if !value.is_null() {
            if ffi::PyDict_SetItemString(dict, CString::new(name.as_str()).unwrap().as_ptr(), value)
                != 0
            {
                ffi::Py_DECREF(dict);
                return Err(());
            }
        }
    }
    if !scope.slots[0].is_null() {
        if ffi::PyDict_Update(dict, scope.slots[0]) != 0 {
            ffi::Py_DECREF(dict);
            return Err(());
        }
    }
    Ok(dict)
}

pub unsafe fn scope_traverse_objects(
    scope: &ScopeInstance,
    visit: ffi::visitproc,
    arg: *mut c_void,
) -> c_int {
    for slot in &scope.slots {
        if !slot.is_null() {
            let result = visit(*slot, arg);
            if result != 0 {
                return result;
            }
        }
    }
    0
}

pub unsafe fn scope_clear_objects(scope: &mut ScopeInstance) {
    for slot in scope.slots.iter_mut() {
        if !slot.is_null() {
            ffi::Py_DECREF(*slot);
            *slot = ptr::null_mut();
        }
    }
}

use dp_transform::min_ast;
use pyo3::ffi;
use std::collections::{HashMap, HashSet};
use std::ffi::CString;
use std::os::raw::{c_char, c_int, c_void};
use std::ptr;
use std::sync::Once;

pub struct RuntimeFns {
    builtins_globals: *mut ffi::PyObject,
    builtins_locals: *mut ffi::PyObject,
    dp_globals: *mut ffi::PyObject,
    dp_locals: *mut ffi::PyObject,
}

const SOAC_FUNCTION_CAPSULE: &[u8] = b"diet_python.soac_function\0";

impl RuntimeFns {
    pub unsafe fn new(
        builtins: *mut ffi::PyObject,
        dp_module: *mut ffi::PyObject,
    ) -> Result<Self, ()> {
        let builtins_globals =
            ffi::PyDict_GetItemString(builtins, CString::new("globals").unwrap().as_ptr());
        let builtins_locals =
            ffi::PyDict_GetItemString(builtins, CString::new("locals").unwrap().as_ptr());
        if builtins_globals.is_null() || builtins_locals.is_null() {
            ffi::PyErr_SetString(
                ffi::PyExc_RuntimeError,
                b"missing builtins globals/locals\0".as_ptr() as *const c_char,
            );
            return Err(());
        }
        let dp_globals =
            ffi::PyObject_GetAttrString(dp_module, CString::new("globals").unwrap().as_ptr());
        if dp_globals.is_null() {
            return Err(());
        }
        let dp_locals =
            ffi::PyObject_GetAttrString(dp_module, CString::new("locals").unwrap().as_ptr());
        if dp_locals.is_null() {
            ffi::Py_DECREF(dp_globals);
            return Err(());
        }
        ffi::Py_INCREF(builtins_globals);
        ffi::Py_INCREF(builtins_locals);
        Ok(Self {
            builtins_globals,
            builtins_locals,
            dp_globals,
            dp_locals,
        })
    }

    fn inc_ref_all(&self) {
        unsafe {
            ffi::Py_INCREF(self.builtins_globals);
            ffi::Py_INCREF(self.builtins_locals);
            ffi::Py_INCREF(self.dp_globals);
            ffi::Py_INCREF(self.dp_locals);
        }
    }
}

impl Drop for RuntimeFns {
    fn drop(&mut self) {
        unsafe {
            ffi::Py_DECREF(self.builtins_globals);
            ffi::Py_DECREF(self.builtins_locals);
            ffi::Py_DECREF(self.dp_globals);
            ffi::Py_DECREF(self.dp_locals);
        }
    }
}

impl Clone for RuntimeFns {
    fn clone(&self) -> Self {
        self.inc_ref_all();
        Self {
            builtins_globals: self.builtins_globals,
            builtins_locals: self.builtins_locals,
            dp_globals: self.dp_globals,
            dp_locals: self.dp_locals,
        }
    }
}

pub struct ScopeLayout {
    names: Vec<String>,
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

    fn slot_for(&self, name: &str) -> Option<usize> {
        self.map.get(name).copied()
    }

    fn slot_count(&self) -> usize {
        self.names.len() + 1
    }
}

pub struct ScopeInstance {
    layout: *const ScopeLayout,
    slots: Vec<*mut ffi::PyObject>,
}

impl ScopeInstance {
    pub fn new(layout: *const ScopeLayout) -> Self {
        let count = unsafe { (*layout).slot_count() };
        Self {
            layout,
            slots: vec![ptr::null_mut(); count],
        }
    }

    fn layout(&self) -> &ScopeLayout {
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

unsafe fn scope_get_slot(scope: &ScopeInstance, slot: usize) -> *mut ffi::PyObject {
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

unsafe fn scope_get_dynamic(scope: &ScopeInstance, name: &str) -> *mut ffi::PyObject {
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

pub(crate) unsafe fn scope_lookup_name(scope: &ScopeInstance, name: &str) -> *mut ffi::PyObject {
    if let Some(slot) = scope.layout().slot_for(name) {
        return scope_get_slot(scope, slot);
    }
    scope_get_dynamic(scope, name)
}

pub(crate) unsafe fn scope_assign_name(
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

pub(crate) unsafe fn scope_delete_name(scope: &mut ScopeInstance, name: &str) -> Result<(), ()> {
    if let Some(slot) = scope.layout().slot_for(name) {
        scope_clear_slot(scope, slot)
    } else {
        scope_del_dynamic(scope, name)
    }
}

pub(crate) unsafe fn scope_to_dict(scope: &ScopeInstance) -> Result<*mut ffi::PyObject, ()> {
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

#[repr(C)]
struct ScopeDictProxy {
    ob_base: ffi::PyObject,
    scope: *mut ScopeInstance,
    owner: *mut ffi::PyObject,
}

unsafe extern "C" fn scope_dictproxy_dealloc(obj: *mut ffi::PyObject) {
    let proxy = obj as *mut ScopeDictProxy;
    if !(*proxy).owner.is_null() {
        ffi::Py_DECREF((*proxy).owner);
    }
    ffi::PyObject_Free(obj as *mut c_void);
}

unsafe fn scope_dictproxy_lookup(
    scope: &ScopeInstance,
    key: *mut ffi::PyObject,
) -> *mut ffi::PyObject {
    if ffi::PyUnicode_Check(key) == 0 {
        ffi::PyErr_SetString(
            ffi::PyExc_TypeError,
            b"mapping key must be str\0".as_ptr() as *const c_char,
        );
        return ptr::null_mut();
    }
    let mut len: ffi::Py_ssize_t = 0;
    let ptr = ffi::PyUnicode_AsUTF8AndSize(key, &mut len);
    if ptr.is_null() {
        return ptr::null_mut();
    }
    let name = std::str::from_utf8_unchecked(std::slice::from_raw_parts(
        ptr as *const u8,
        len as usize,
    ));
    let value = scope_lookup_name(scope, name);
    if value.is_null() {
        ffi::PyErr_SetObject(ffi::PyExc_KeyError, key);
    }
    value
}

unsafe fn scope_dictproxy_store(
    scope: &mut ScopeInstance,
    key: *mut ffi::PyObject,
    value: *mut ffi::PyObject,
) -> c_int {
    if ffi::PyUnicode_Check(key) == 0 {
        ffi::PyErr_SetString(
            ffi::PyExc_TypeError,
            b"mapping key must be str\0".as_ptr() as *const c_char,
        );
        return -1;
    }
    let mut len: ffi::Py_ssize_t = 0;
    let ptr = ffi::PyUnicode_AsUTF8AndSize(key, &mut len);
    if ptr.is_null() {
        return -1;
    }
    let name = std::str::from_utf8_unchecked(std::slice::from_raw_parts(
        ptr as *const u8,
        len as usize,
    ));
    let result = if value.is_null() {
        scope_delete_name(scope, name)
    } else {
        scope_assign_name(scope, name, value)
    };
    if result.is_err() {
        if value.is_null() && ffi::PyErr_ExceptionMatches(ffi::PyExc_NameError) != 0 {
            ffi::PyErr_Clear();
            ffi::PyErr_SetObject(ffi::PyExc_KeyError, key);
        }
        return -1;
    }
    0
}

unsafe extern "C" fn scope_dictproxy_subscript(
    obj: *mut ffi::PyObject,
    key: *mut ffi::PyObject,
) -> *mut ffi::PyObject {
    let proxy = obj as *mut ScopeDictProxy;
    if (*proxy).scope.is_null() {
        ffi::PyErr_SetString(
            ffi::PyExc_RuntimeError,
            b"scope unavailable\0".as_ptr() as *const c_char,
        );
        return ptr::null_mut();
    }
    scope_dictproxy_lookup(&*(*proxy).scope, key)
}

unsafe extern "C" fn scope_dictproxy_ass_subscript(
    obj: *mut ffi::PyObject,
    key: *mut ffi::PyObject,
    value: *mut ffi::PyObject,
) -> c_int {
    let proxy = obj as *mut ScopeDictProxy;
    if (*proxy).scope.is_null() {
        ffi::PyErr_SetString(
            ffi::PyExc_RuntimeError,
            b"scope unavailable\0".as_ptr() as *const c_char,
        );
        return -1;
    }
    scope_dictproxy_store(&mut *(*proxy).scope, key, value)
}

static mut SCOPE_DICTPROXY_MAPPING: ffi::PyMappingMethods = ffi::PyMappingMethods {
    mp_length: None,
    mp_subscript: Some(scope_dictproxy_subscript),
    mp_ass_subscript: Some(scope_dictproxy_ass_subscript),
};

#[allow(clippy::uninit_assumed_init)]
static mut SCOPE_DICTPROXY_TYPE: ffi::PyTypeObject = ffi::PyTypeObject {
    ob_base: ffi::PyVarObject {
        ob_base: ffi::PyObject_HEAD_INIT,
        ob_size: 0,
    },
    tp_name: b"diet_python.ScopeDictProxy\0".as_ptr() as *const _,
    tp_basicsize: std::mem::size_of::<ScopeDictProxy>() as ffi::Py_ssize_t,
    tp_itemsize: 0,
    tp_dealloc: Some(scope_dictproxy_dealloc),
    tp_as_mapping: std::ptr::addr_of_mut!(SCOPE_DICTPROXY_MAPPING),
    tp_flags: ffi::Py_TPFLAGS_DEFAULT,
    ..unsafe { std::mem::zeroed() }
};

static INIT_SCOPE_DICTPROXY_TYPE: Once = Once::new();

unsafe fn init_scope_dictproxy_type() -> Result<(), ()> {
    let mut result = Ok(());
    INIT_SCOPE_DICTPROXY_TYPE.call_once(|| {
        if ffi::PyType_Ready(std::ptr::addr_of_mut!(SCOPE_DICTPROXY_TYPE)) < 0 {
            result = Err(());
        }
    });
    result
}

pub(crate) unsafe fn scope_dictproxy_new(
    scope: *mut ScopeInstance,
    owner: *mut ffi::PyObject,
) -> *mut ffi::PyObject {
    if init_scope_dictproxy_type().is_err() {
        return ptr::null_mut();
    }
    let proxy = ffi::_PyObject_New(std::ptr::addr_of_mut!(SCOPE_DICTPROXY_TYPE))
        as *mut ScopeDictProxy;
    if proxy.is_null() {
        return ptr::null_mut();
    }
    (*proxy).scope = scope;
    (*proxy).owner = owner;
    if !owner.is_null() {
        ffi::Py_INCREF(owner);
    }
    proxy as *mut ffi::PyObject
}

struct ClosureScope {
    _layout: Box<ScopeLayout>,
    scope: ScopeInstance,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ParamKind {
    Positional,
    VarArg,
    KwOnly,
    KwArg,
}

struct ParamSpec {
    name: String,
    kind: ParamKind,
    default: Option<*mut ffi::PyObject>,
}

impl ParamSpec {
    fn drop_default(&mut self) {
        if let Some(value) = self.default.take() {
            unsafe { ffi::Py_DECREF(value) };
        }
    }
}

pub struct FunctionData {
    def: min_ast::FunctionDef,
    params: Vec<ParamSpec>,
    param_layout: Box<ScopeLayout>,
    local_layout: Box<ScopeLayout>,
    closure: Option<ClosureScope>,
    globals: *mut ScopeInstance,
    globals_owner: *mut ffi::PyObject,
    builtins: *mut ffi::PyObject,
    runtime_fns: RuntimeFns,
    method_def: Box<ffi::PyMethodDef>,
    _name_cstr: CString,
}

impl Drop for FunctionData {
    fn drop(&mut self) {
        unsafe {
            for param in &mut self.params {
                param.drop_default();
            }
            if !self.globals_owner.is_null() {
                ffi::Py_DECREF(self.globals_owner);
            }
            ffi::Py_DECREF(self.builtins);
        }
    }
}

unsafe extern "C" fn soac_function_call(
    slf: *mut ffi::PyObject,
    args: *mut ffi::PyObject,
    kwargs: *mut ffi::PyObject,
) -> *mut ffi::PyObject {
    let capsule = slf;
    let ptr = ffi::PyCapsule_GetPointer(
        capsule,
        SOAC_FUNCTION_CAPSULE.as_ptr() as *const c_char,
    );
    if ptr.is_null() {
        ffi::PyErr_SetString(
            ffi::PyExc_RuntimeError,
            b"soac function capsule missing\0".as_ptr() as *const c_char,
        );
        return ptr::null_mut();
    }
    let func = &*(ptr as *const FunctionData);
    func.call_from_python(args, kwargs)
}

unsafe extern "C" fn soac_function_capsule_destructor(obj: *mut ffi::PyObject) {
    let ptr =
        ffi::PyCapsule_GetPointer(obj, SOAC_FUNCTION_CAPSULE.as_ptr() as *const c_char);
    if ptr.is_null() {
        return;
    }
    drop(Box::from_raw(ptr as *mut FunctionData));
}

pub struct ExecContext<'a> {
    globals: *mut ScopeInstance,
    globals_owner: *mut ffi::PyObject,
    params: *mut ScopeInstance,
    locals: *mut ScopeInstance,
    builtins: *mut ffi::PyObject,
    closure: Option<&'a ScopeInstance>,
    runtime_fns: &'a RuntimeFns,
}

#[derive(Debug, PartialEq, Eq)]
enum StmtFlow {
    Normal,
    Break,
    Continue,
    Return(*mut ffi::PyObject),
}

fn set_type_error<T>(msg: &str) -> Result<T, ()> {
    unsafe {
        ffi::PyErr_SetString(ffi::PyExc_TypeError, CString::new(msg).unwrap().as_ptr());
    }
    Err(())
}

fn set_runtime_error<T>(msg: &str) -> Result<T, ()> {
    unsafe {
        ffi::PyErr_SetString(
            ffi::PyExc_RuntimeError,
            CString::new(msg).unwrap().as_ptr(),
        );
    }
    Err(())
}

fn set_name_error<T>(name: &str) -> Result<T, ()> {
    unsafe {
        let msg = CString::new(format!("name '{name}' is not defined")).unwrap();
        ffi::PyErr_SetString(ffi::PyExc_NameError, msg.as_ptr());
    }
    Err(())
}

fn set_unbound_local<T>(name: &str) -> Result<T, ()> {
    unsafe {
        let msg = CString::new(format!("local variable '{name}' referenced before assignment"))
            .unwrap();
        ffi::PyErr_SetString(ffi::PyExc_UnboundLocalError, msg.as_ptr());
    }
    Err(())
}

fn set_not_implemented<T>(msg: &str) -> Result<T, ()> {
    unsafe {
        ffi::PyErr_SetString(
            ffi::PyExc_NotImplementedError,
            CString::new(msg).unwrap().as_ptr(),
        );
    }
    Err(())
}

fn collect_bound_names(stmts: &[min_ast::StmtNode], names: &mut HashSet<String>) {
    for stmt in stmts {
        match stmt {
            min_ast::StmtNode::Assign { target, .. }
            | min_ast::StmtNode::Delete { target, .. } => {
                names.insert(target.clone());
            }
            min_ast::StmtNode::ImportFrom { names: imports, .. } => {
                for name in imports {
                    names.insert(name.clone());
                }
            }
            min_ast::StmtNode::FunctionDef(func) => {
                names.insert(func.name.clone());
            }
            min_ast::StmtNode::While { body, orelse, .. }
            | min_ast::StmtNode::If { body, orelse, .. } => {
                collect_bound_names(body, names);
                collect_bound_names(orelse, names);
            }
            min_ast::StmtNode::Try {
                body,
                handler,
                orelse,
                finalbody,
                ..
            } => {
                collect_bound_names(body, names);
                if let Some(handler) = handler {
                    collect_bound_names(handler, names);
                }
                collect_bound_names(orelse, names);
                collect_bound_names(finalbody, names);
            }
            _ => {}
        }
    }
}

fn collect_local_names(def: &min_ast::FunctionDef) -> HashSet<String> {
    let mut locals = HashSet::new();
    for param in &def.params {
        match param {
            min_ast::Parameter::Positional { name, .. }
            | min_ast::Parameter::VarArg { name, .. }
            | min_ast::Parameter::KwOnly { name, .. }
            | min_ast::Parameter::KwArg { name, .. } => {
                locals.insert(name.clone());
            }
        }
    }
    collect_bound_names(&def.body, &mut locals);
    locals
}

pub(crate) fn build_module_layout(module: &min_ast::Module) -> ScopeLayout {
    let mut names = HashSet::new();
    collect_bound_names(&module.body, &mut names);
    ScopeLayout::new(names)
}

fn collect_used_names(def: &min_ast::FunctionDef) -> HashSet<String> {
    fn visit_expr(expr: &min_ast::ExprNode, names: &mut HashSet<String>) {
        match expr {
            min_ast::ExprNode::Name { id, .. } => {
                names.insert(id.clone());
            }
            min_ast::ExprNode::Attribute { value, .. } => {
                visit_expr(value, names);
            }
            min_ast::ExprNode::Tuple { elts, .. } => {
                for elt in elts {
                    visit_expr(elt, names);
                }
            }
            min_ast::ExprNode::Await { value, .. } => {
                visit_expr(value, names);
            }
            min_ast::ExprNode::Yield { value, .. } => {
                if let Some(value) = value {
                    visit_expr(value, names);
                }
            }
            min_ast::ExprNode::Call { func, args, .. } => {
                visit_expr(func, names);
                for arg in args {
                    match arg {
                        min_ast::Arg::Positional(expr)
                        | min_ast::Arg::Starred(expr)
                        | min_ast::Arg::KwStarred(expr) => visit_expr(expr, names),
                        min_ast::Arg::Keyword { value, .. } => visit_expr(value, names),
                    }
                }
            }
            min_ast::ExprNode::Number { .. }
            | min_ast::ExprNode::String { .. }
            | min_ast::ExprNode::Bytes { .. } => {}
        }
    }

    fn visit_stmt(stmt: &min_ast::StmtNode, names: &mut HashSet<String>) {
        match stmt {
            min_ast::StmtNode::FunctionDef(_) => {}
            min_ast::StmtNode::While { test, body, orelse, .. }
            | min_ast::StmtNode::If { test, body, orelse, .. } => {
                visit_expr(test, names);
                for stmt in body {
                    visit_stmt(stmt, names);
                }
                for stmt in orelse {
                    visit_stmt(stmt, names);
                }
            }
            min_ast::StmtNode::Try {
                body,
                handler,
                orelse,
                finalbody,
                ..
            } => {
                for stmt in body {
                    visit_stmt(stmt, names);
                }
                if let Some(handler) = handler {
                    for stmt in handler {
                        visit_stmt(stmt, names);
                    }
                }
                for stmt in orelse {
                    visit_stmt(stmt, names);
                }
                for stmt in finalbody {
                    visit_stmt(stmt, names);
                }
            }
            min_ast::StmtNode::Raise { exc, .. } => {
                if let Some(expr) = exc {
                    visit_expr(expr, names);
                }
            }
            min_ast::StmtNode::Return { value, .. } => {
                if let Some(expr) = value {
                    visit_expr(expr, names);
                }
            }
            min_ast::StmtNode::Expr { value, .. } => visit_expr(value, names),
            min_ast::StmtNode::Assign { value, .. } => visit_expr(value, names),
            min_ast::StmtNode::ImportFrom { .. }
            | min_ast::StmtNode::Delete { .. }
            | min_ast::StmtNode::Break(_)
            | min_ast::StmtNode::Continue(_)
            | min_ast::StmtNode::Pass(_) => {}
        }
    }

    let mut names = HashSet::new();
    for stmt in &def.body {
        visit_stmt(stmt, &mut names);
    }
    names
}

fn capture_closure(
    used_names: &HashSet<String>,
    locals: &HashSet<String>,
    ctx: &ExecContext<'_>,
) -> Result<Option<ClosureScope>, ()> {
    let mut captured = Vec::new();
    for name in used_names {
        if locals.contains(name) {
            continue;
        }
        unsafe {
            if ctx.locals != ctx.globals {
                let locals_scope = &*ctx.locals;
                let value = scope_lookup_name(locals_scope, name);
                if !value.is_null() {
                    captured.push((name.clone(), value));
                    continue;
                }
            }
            if !ctx.params.is_null() {
                let params_scope = &*ctx.params;
                let value = scope_lookup_name(params_scope, name);
                if !value.is_null() {
                    captured.push((name.clone(), value));
                    continue;
                }
            }
            if let Some(closure) = ctx.closure {
                let value = scope_lookup_name(closure, name);
                if !value.is_null() {
                    captured.push((name.clone(), value));
                }
            }
        }
    }

    if captured.is_empty() {
        return Ok(None);
    }

    let mut names = HashSet::new();
    for (name, _) in &captured {
        names.insert(name.clone());
    }
    let layout = Box::new(ScopeLayout::new(names));
    let mut scope = ScopeInstance::new(&*layout);
    for (idx, (name, value)) in captured.iter().enumerate() {
        let result = unsafe { scope_assign_name(&mut scope, name.as_str(), *value) };
        if result.is_err() {
            for (_, value) in captured[idx..].iter() {
                unsafe { ffi::Py_DECREF(*value) };
            }
            return Err(());
        }
        unsafe { ffi::Py_DECREF(*value) };
    }

    Ok(Some(ClosureScope { _layout: layout, scope }))
}

impl FunctionData {
    unsafe fn call_from_python(
        &self,
        args: *mut ffi::PyObject,
        kwargs: *mut ffi::PyObject,
    ) -> *mut ffi::PyObject {
        if self.def.is_async {
            ffi::PyErr_SetString(
                ffi::PyExc_NotImplementedError,
                b"async functions not supported\0".as_ptr() as *const c_char,
            );
            return ptr::null_mut();
        }

        let mut params_scope = ScopeInstance::new(&*self.param_layout);
        let params_ptr = &mut params_scope as *mut ScopeInstance;

        if bind_args(&self.params, args, kwargs, params_ptr).is_err() {
            return ptr::null_mut();
        }

        self.call(params_ptr)
    }

    unsafe fn call(&self, params_scope: *mut ScopeInstance) -> *mut ffi::PyObject {
        if apply_param_defaults(&self.params, params_scope).is_err() {
            return ptr::null_mut();
        }

        let mut locals_box = Box::new(ScopeInstance::new(&*self.local_layout));
        let locals_ptr = locals_box.as_mut() as *mut ScopeInstance;

        let ctx = ExecContext {
            globals: self.globals,
            globals_owner: self.globals_owner,
            params: params_scope,
            locals: locals_ptr,
            builtins: self.builtins,
            closure: self.closure.as_ref().map(|closure| &closure.scope),
            runtime_fns: &self.runtime_fns,
        };

        let result = match eval_block(&self.def.body, &ctx) {
            Ok(StmtFlow::Return(value)) => value,
            Ok(StmtFlow::Normal) => {
                ffi::Py_INCREF(ffi::Py_None());
                ffi::Py_None()
            }
            Ok(StmtFlow::Break) | Ok(StmtFlow::Continue) => {
                let _ = set_runtime_error::<()>("break/continue outside loop");
                ptr::null_mut()
            }
            Err(()) => ptr::null_mut(),
        };

        drop(locals_box);
        result
    }
}

pub unsafe fn eval_module(
    module: &min_ast::Module,
    globals: *mut ScopeInstance,
    globals_owner: *mut ffi::PyObject,
    builtins: *mut ffi::PyObject,
    runtime_fns: &RuntimeFns,
) -> Result<(), ()> {
    let ctx = ExecContext {
        globals,
        globals_owner,
        params: ptr::null_mut(),
        locals: globals,
        builtins,
        closure: None,
        runtime_fns,
    };
    match eval_block(&module.body, &ctx) {
        Ok(StmtFlow::Normal) => Ok(()),
        Ok(StmtFlow::Return(_)) => set_runtime_error("return outside function"),
        Ok(StmtFlow::Break) | Ok(StmtFlow::Continue) => {
            set_runtime_error("break/continue outside loop")
        }
        Err(()) => Err(()),
    }
}

pub unsafe fn build_function(
    def: min_ast::FunctionDef,
    ctx: &ExecContext<'_>,
    module_name: *mut ffi::PyObject,
) -> Result<*mut ffi::PyObject, ()> {
    let local_names = collect_local_names(&def);
    let used_names = collect_used_names(&def);
    let closure = capture_closure(&used_names, &local_names, ctx)?;
    let local_layout = Box::new(ScopeLayout::new(local_names));

    let mut param_names = HashSet::new();
    for param in &def.params {
        match param {
            min_ast::Parameter::Positional { name, .. }
            | min_ast::Parameter::VarArg { name, .. }
            | min_ast::Parameter::KwOnly { name, .. }
            | min_ast::Parameter::KwArg { name, .. } => {
                param_names.insert(name.clone());
            }
        }
    }
    let param_layout = Box::new(ScopeLayout::new(param_names));

    let mut params = Vec::new();
    for param in &def.params {
        match param {
            min_ast::Parameter::Positional { name, default, annotation } => {
                if let Some(annotation) = annotation {
                    eval_expr(annotation, ctx)?;
                }
                let default_value = if let Some(expr) = default {
                    Some(eval_expr(expr, ctx)?)
                } else {
                    None
                };
                params.push(ParamSpec {
                    name: name.clone(),
                    kind: ParamKind::Positional,
                    default: default_value,
                });
            }
            min_ast::Parameter::VarArg { name, annotation } => {
                if let Some(annotation) = annotation {
                    eval_expr(annotation, ctx)?;
                }
                params.push(ParamSpec {
                    name: name.clone(),
                    kind: ParamKind::VarArg,
                    default: None,
                });
            }
            min_ast::Parameter::KwOnly { name, default, annotation } => {
                if let Some(annotation) = annotation {
                    eval_expr(annotation, ctx)?;
                }
                let default_value = if let Some(expr) = default {
                    Some(eval_expr(expr, ctx)?)
                } else {
                    None
                };
                params.push(ParamSpec {
                    name: name.clone(),
                    kind: ParamKind::KwOnly,
                    default: default_value,
                });
            }
            min_ast::Parameter::KwArg { name, annotation } => {
                if let Some(annotation) = annotation {
                    eval_expr(annotation, ctx)?;
                }
                params.push(ParamSpec {
                    name: name.clone(),
                    kind: ParamKind::KwArg,
                    default: None,
                });
            }
        }
    }

    if let Some(returns) = &def.returns {
        eval_expr(returns, ctx)?;
    }

    let name_cstr = CString::new(def.name.as_str()).unwrap();
    let method_def = Box::new(ffi::PyMethodDef {
        ml_name: name_cstr.as_ptr(),
        ml_meth: ffi::PyMethodDefPointer {
            PyCFunctionWithKeywords: soac_function_call,
        },
        ml_flags: ffi::METH_VARARGS | ffi::METH_KEYWORDS,
        ml_doc: ptr::null(),
    });

    let globals = ctx.globals;
    let globals_owner = ctx.globals_owner;
    let builtins = ctx.builtins;
    if !globals_owner.is_null() {
        ffi::Py_INCREF(globals_owner);
    }
    ffi::Py_INCREF(builtins);

    let mut data = Box::new(FunctionData {
        def,
        params,
        param_layout,
        local_layout,
        closure,
        globals,
        globals_owner,
        builtins,
        runtime_fns: ctx.runtime_fns.clone(),
        method_def,
        _name_cstr: name_cstr,
    });
    let method_def_ptr = data.method_def.as_mut() as *mut ffi::PyMethodDef;

    let capsule = ffi::PyCapsule_New(
        Box::into_raw(data) as *mut c_void,
        SOAC_FUNCTION_CAPSULE.as_ptr() as *const c_char,
        Some(soac_function_capsule_destructor),
    );
    if capsule.is_null() {
        return Err(());
    }

    let module_name_ptr = if module_name.is_null() {
        ptr::null_mut()
    } else {
        module_name
    };

    let func = ffi::PyCFunction_NewEx(method_def_ptr, capsule, module_name_ptr);
    if func.is_null() {
        ffi::Py_DECREF(capsule);
        return Err(());
    }

    // PyCFunction holds a reference to capsule; release our ref.
    ffi::Py_DECREF(capsule);

    Ok(func)
}

fn bind_args(
    params: &[ParamSpec],
    args: *mut ffi::PyObject,
    kwargs: *mut ffi::PyObject,
    param_scope: *mut ScopeInstance,
) -> Result<(), ()> {
    unsafe {
        let args_tuple = if args.is_null() {
            ffi::PyTuple_New(0)
        } else {
            ffi::Py_INCREF(args);
            args
        };
        if args_tuple.is_null() {
            return Err(());
        }
        let args_len = ffi::PyTuple_Size(args_tuple);
        if args_len < 0 {
            ffi::Py_DECREF(args_tuple);
            return Err(());
        }
        let mut kw_map = HashMap::new();
        if !kwargs.is_null() {
            let mut pos: ffi::Py_ssize_t = 0;
            let mut key: *mut ffi::PyObject = ptr::null_mut();
            let mut value: *mut ffi::PyObject = ptr::null_mut();
            while ffi::PyDict_Next(kwargs, &mut pos, &mut key, &mut value) != 0 {
                if ffi::PyUnicode_Check(key) == 0 {
                    ffi::Py_DECREF(args_tuple);
                    return set_type_error("keywords must be strings");
                }
                let mut len: ffi::Py_ssize_t = 0;
                let ptr = ffi::PyUnicode_AsUTF8AndSize(key, &mut len);
                if ptr.is_null() {
                    ffi::Py_DECREF(args_tuple);
                    return Err(());
                }
                let name = std::str::from_utf8_unchecked(std::slice::from_raw_parts(ptr as *const u8, len as usize));
                kw_map.insert(name.to_string(), value);
            }
        }

        let mut arg_index: ffi::Py_ssize_t = 0;
        let mut has_vararg = false;

        for param in params {
            match param.kind {
                ParamKind::Positional => {
                    if arg_index < args_len {
                        if kw_map.contains_key(&param.name) {
                            ffi::Py_DECREF(args_tuple);
                            return set_type_error("multiple values for argument");
                        }
                        let value = ffi::PyTuple_GetItem(args_tuple, arg_index);
                        if value.is_null() {
                            ffi::Py_DECREF(args_tuple);
                            return Err(());
                        }
                        if scope_assign_name(&mut *param_scope, param.name.as_str(), value)
                            .is_err()
                        {
                            ffi::Py_DECREF(args_tuple);
                            return Err(());
                        }
                        arg_index += 1;
                    } else if let Some(value) = kw_map.remove(&param.name) {
                        if scope_assign_name(&mut *param_scope, param.name.as_str(), value)
                            .is_err()
                        {
                            ffi::Py_DECREF(args_tuple);
                            return Err(());
                        }
                    } else {
                        ffi::Py_DECREF(args_tuple);
                        return set_type_error("missing required positional argument");
                    }
                }
                ParamKind::VarArg => {
                    has_vararg = true;
                    let remaining = args_len - arg_index;
                    let tuple = ffi::PyTuple_New(remaining);
                    if tuple.is_null() {
                        ffi::Py_DECREF(args_tuple);
                        return Err(());
                    }
                    for idx in 0..remaining {
                        let value = ffi::PyTuple_GetItem(args_tuple, arg_index + idx);
                        if value.is_null() {
                            ffi::Py_DECREF(tuple);
                            ffi::Py_DECREF(args_tuple);
                            return Err(());
                        }
                        ffi::Py_INCREF(value);
                        if ffi::PyTuple_SetItem(tuple, idx, value) != 0 {
                            ffi::Py_DECREF(tuple);
                            ffi::Py_DECREF(args_tuple);
                            return Err(());
                        }
                    }
                    arg_index = args_len;
                    if scope_assign_name(&mut *param_scope, param.name.as_str(), tuple).is_err() {
                        ffi::Py_DECREF(tuple);
                        ffi::Py_DECREF(args_tuple);
                        return Err(());
                    }
                    ffi::Py_DECREF(tuple);
                }
                ParamKind::KwOnly => {
                    if let Some(value) = kw_map.remove(&param.name) {
                        if scope_assign_name(&mut *param_scope, param.name.as_str(), value)
                            .is_err()
                        {
                            ffi::Py_DECREF(args_tuple);
                            return Err(());
                        }
                    } else {
                        ffi::Py_DECREF(args_tuple);
                        return set_type_error("missing required keyword-only argument");
                    }
                }
                ParamKind::KwArg => {
                    let dict = ffi::PyDict_New();
                    if dict.is_null() {
                        ffi::Py_DECREF(args_tuple);
                        return Err(());
                    }
                    for (key, value) in &kw_map {
                        if ffi::PyDict_SetItemString(
                            dict,
                            CString::new(key.as_str()).unwrap().as_ptr(),
                            *value,
                        ) != 0
                        {
                            ffi::Py_DECREF(dict);
                            ffi::Py_DECREF(args_tuple);
                            return Err(());
                        }
                    }
                    kw_map.clear();
                    if scope_assign_name(&mut *param_scope, param.name.as_str(), dict).is_err() {
                        ffi::Py_DECREF(dict);
                        ffi::Py_DECREF(args_tuple);
                        return Err(());
                    }
                    ffi::Py_DECREF(dict);
                }
            }
        }

        if arg_index < args_len && !has_vararg {
            ffi::Py_DECREF(args_tuple);
            return set_type_error("too many positional arguments");
        }

        if !kw_map.is_empty() {
            ffi::Py_DECREF(args_tuple);
            return set_type_error("unexpected keyword argument");
        }

        ffi::Py_DECREF(args_tuple);
        Ok(())
    }
}

fn apply_param_defaults(params: &[ParamSpec], param_scope: *mut ScopeInstance) -> Result<(), ()> {
    unsafe {
        let scope = &mut *param_scope;
        for param in params {
            let Some(default) = param.default else {
                continue;
            };
            let Some(slot) = scope.layout().slot_for(param.name.as_str()) else {
                continue;
            };
            if scope.slots[slot].is_null() {
                if scope_assign_name(scope, param.name.as_str(), default).is_err() {
                    return Err(());
                }
            }
        }
    }
    Ok(())
}

struct CallArgs {
    positional: Vec<*mut ffi::PyObject>,
    kw_map: HashMap<String, *mut ffi::PyObject>,
}

impl CallArgs {
    unsafe fn cleanup(self) {
        for value in self.positional {
            if !value.is_null() {
                ffi::Py_DECREF(value);
            }
        }
        for (_, value) in self.kw_map {
            if !value.is_null() {
                ffi::Py_DECREF(value);
            }
        }
    }
}

fn collect_call_args(args: &[min_ast::Arg], ctx: &ExecContext<'_>) -> Result<CallArgs, ()> {
    let mut positional: Vec<*mut ffi::PyObject> = Vec::new();
    let mut kw_map: HashMap<String, *mut ffi::PyObject> = HashMap::new();

    for arg in args {
        match arg {
            min_ast::Arg::Positional(expr) => {
                let value = eval_expr(expr, ctx)?;
                positional.push(value);
            }
            min_ast::Arg::Starred(expr) => unsafe {
                let value = eval_expr(expr, ctx)?;
                let seq = ffi::PySequence_Fast(
                    value,
                    b"argument after * must be iterable\0".as_ptr() as *const c_char,
                );
                ffi::Py_DECREF(value);
                if seq.is_null() {
                    CallArgs { positional, kw_map }.cleanup();
                    return Err(());
                }
                let seq_len = ffi::PySequence_Size(seq);
                if seq_len < 0 {
                    ffi::Py_DECREF(seq);
                    CallArgs { positional, kw_map }.cleanup();
                    return Err(());
                }
                for idx in 0..seq_len {
                    let item = ffi::PySequence_GetItem(seq, idx);
                    if item.is_null() {
                        ffi::Py_DECREF(seq);
                        CallArgs { positional, kw_map }.cleanup();
                        return Err(());
                    }
                    positional.push(item);
                }
                ffi::Py_DECREF(seq);
            },
            min_ast::Arg::Keyword { name, value } => unsafe {
                let val = eval_expr(value, ctx)?;
                if kw_map.contains_key(name) {
                    ffi::Py_DECREF(val);
                    CallArgs { positional, kw_map }.cleanup();
                    return set_type_error("multiple values for keyword argument");
                }
                kw_map.insert(name.clone(), val);
            },
            min_ast::Arg::KwStarred(expr) => unsafe {
                let mapping = eval_expr(expr, ctx)?;
                let items = ffi::PyMapping_Items(mapping);
                ffi::Py_DECREF(mapping);
                if items.is_null() {
                    CallArgs { positional, kw_map }.cleanup();
                    return Err(());
                }
                let items_len = ffi::PySequence_Size(items);
                if items_len < 0 {
                    ffi::Py_DECREF(items);
                    CallArgs { positional, kw_map }.cleanup();
                    return Err(());
                }
                for idx in 0..items_len {
                    let item = ffi::PySequence_GetItem(items, idx);
                    if item.is_null() {
                        ffi::Py_DECREF(items);
                        CallArgs { positional, kw_map }.cleanup();
                        return Err(());
                    }
                    let key = ffi::PySequence_GetItem(item, 0);
                    let val = ffi::PySequence_GetItem(item, 1);
                    ffi::Py_DECREF(item);
                    if key.is_null() || val.is_null() {
                        ffi::Py_XDECREF(key);
                        ffi::Py_XDECREF(val);
                        ffi::Py_DECREF(items);
                        CallArgs { positional, kw_map }.cleanup();
                        return Err(());
                    }
                    if ffi::PyUnicode_Check(key) == 0 {
                        ffi::Py_DECREF(key);
                        ffi::Py_DECREF(val);
                        ffi::Py_DECREF(items);
                        CallArgs { positional, kw_map }.cleanup();
                        return set_type_error("keywords must be strings");
                    }
                    let mut len: ffi::Py_ssize_t = 0;
                    let ptr = ffi::PyUnicode_AsUTF8AndSize(key, &mut len);
                    if ptr.is_null() {
                        ffi::Py_DECREF(key);
                        ffi::Py_DECREF(val);
                        ffi::Py_DECREF(items);
                        CallArgs { positional, kw_map }.cleanup();
                        return Err(());
                    }
                    let key_str = std::str::from_utf8_unchecked(std::slice::from_raw_parts(
                        ptr as *const u8,
                        len as usize,
                    ));
                    if kw_map.contains_key(key_str) {
                        ffi::Py_DECREF(key);
                        ffi::Py_DECREF(val);
                        ffi::Py_DECREF(items);
                        CallArgs { positional, kw_map }.cleanup();
                        return set_type_error("multiple values for keyword argument");
                    }
                    kw_map.insert(key_str.to_string(), val);
                    ffi::Py_DECREF(key);
                }
                ffi::Py_DECREF(items);
            },
        }
    }

    Ok(CallArgs { positional, kw_map })
}

fn bind_args_direct(
    params: &[ParamSpec],
    mut call_args: CallArgs,
    param_scope: *mut ScopeInstance,
) -> Result<(), ()> {
    unsafe {
        let mut arg_index: usize = 0;
        let mut has_vararg = false;

        for param in params {
            match param.kind {
                ParamKind::Positional => {
                    if arg_index < call_args.positional.len() {
                        if call_args.kw_map.contains_key(&param.name) {
                            call_args.cleanup();
                            return set_type_error("multiple values for argument");
                        }
                        let value = call_args.positional[arg_index];
                        if scope_assign_name(&mut *param_scope, param.name.as_str(), value).is_err()
                        {
                            call_args.cleanup();
                            return Err(());
                        }
                        ffi::Py_DECREF(value);
                        call_args.positional[arg_index] = ptr::null_mut();
                        arg_index += 1;
                    } else if let Some(value) = call_args.kw_map.remove(&param.name) {
                        if scope_assign_name(&mut *param_scope, param.name.as_str(), value).is_err()
                        {
                            ffi::Py_DECREF(value);
                            call_args.cleanup();
                            return Err(());
                        }
                        ffi::Py_DECREF(value);
                    } else {
                        call_args.cleanup();
                        return set_type_error("missing required positional argument");
                    }
                }
                ParamKind::VarArg => {
                    has_vararg = true;
                    let remaining = call_args.positional.len().saturating_sub(arg_index);
                    let tuple = ffi::PyTuple_New(remaining as _);
                    if tuple.is_null() {
                        call_args.cleanup();
                        return Err(());
                    }
                    for idx in 0..remaining {
                        let pos = arg_index + idx;
                        let value = call_args.positional[pos];
                        if value.is_null() {
                            ffi::Py_DECREF(tuple);
                            call_args.cleanup();
                            return Err(());
                        }
                        if ffi::PyTuple_SetItem(tuple, idx as _, value) != 0 {
                            ffi::Py_DECREF(tuple);
                            call_args.cleanup();
                            return Err(());
                        }
                        call_args.positional[pos] = ptr::null_mut();
                    }
                    arg_index = call_args.positional.len();
                    if scope_assign_name(&mut *param_scope, param.name.as_str(), tuple).is_err() {
                        ffi::Py_DECREF(tuple);
                        call_args.cleanup();
                        return Err(());
                    }
                    ffi::Py_DECREF(tuple);
                }
                ParamKind::KwOnly => {
                    if let Some(value) = call_args.kw_map.remove(&param.name) {
                        if scope_assign_name(&mut *param_scope, param.name.as_str(), value).is_err()
                        {
                            ffi::Py_DECREF(value);
                            call_args.cleanup();
                            return Err(());
                        }
                        ffi::Py_DECREF(value);
                    } else {
                        call_args.cleanup();
                        return set_type_error("missing required keyword-only argument");
                    }
                }
                ParamKind::KwArg => {
                    let dict = ffi::PyDict_New();
                    if dict.is_null() {
                        call_args.cleanup();
                        return Err(());
                    }
                    let mut kw_map = std::mem::take(&mut call_args.kw_map);
                    let mut iter = kw_map.drain();
                    while let Some((key, value)) = iter.next() {
                        if ffi::PyDict_SetItemString(
                            dict,
                            CString::new(key.as_str()).unwrap().as_ptr(),
                            value,
                        ) != 0
                        {
                            ffi::Py_DECREF(value);
                            for (_, value) in iter {
                                ffi::Py_DECREF(value);
                            }
                            ffi::Py_DECREF(dict);
                            call_args.cleanup();
                            return Err(());
                        }
                        ffi::Py_DECREF(value);
                    }
                    if scope_assign_name(&mut *param_scope, param.name.as_str(), dict).is_err() {
                        ffi::Py_DECREF(dict);
                        call_args.cleanup();
                        return Err(());
                    }
                    ffi::Py_DECREF(dict);
                }
            }
        }

        if arg_index < call_args.positional.len() && !has_vararg {
            call_args.cleanup();
            return set_type_error("too many positional arguments");
        }

        if !call_args.kw_map.is_empty() {
            call_args.cleanup();
            return set_type_error("unexpected keyword argument");
        }
    }
    Ok(())
}

fn eval_block(stmts: &[min_ast::StmtNode], ctx: &ExecContext<'_>) -> Result<StmtFlow, ()> {
    for stmt in stmts {
        match eval_stmt(stmt, ctx)? {
            StmtFlow::Normal => {}
            flow => return Ok(flow),
        }
    }
    Ok(StmtFlow::Normal)
}

fn eval_stmt(stmt: &min_ast::StmtNode, ctx: &ExecContext<'_>) -> Result<StmtFlow, ()> {
    match stmt {
        min_ast::StmtNode::FunctionDef(func) => unsafe {
            let module_name = scope_lookup_name(&*ctx.globals, "__name__");
            let function = build_function(func.clone(), ctx, module_name)?;
            if !module_name.is_null() {
                ffi::Py_DECREF(module_name);
            }
            if scope_assign_name(&mut *ctx.locals, func.name.as_str(), function).is_err() {
                ffi::Py_DECREF(function);
                return Err(());
            }
            ffi::Py_DECREF(function);
            Ok(StmtFlow::Normal)
        },
        min_ast::StmtNode::ImportFrom { module, names, level, .. } => unsafe {
            let globals_dict = scope_to_dict(&*ctx.globals)?;
            let locals_dict = if ctx.locals == ctx.globals {
                ffi::Py_INCREF(globals_dict);
                globals_dict
            } else {
                scope_to_dict(&*ctx.locals)?
            };
            let name_obj = if let Some(name) = module {
                ffi::PyUnicode_FromString(CString::new(name.as_str()).unwrap().as_ptr())
            } else {
                ffi::Py_INCREF(ffi::Py_None());
                ffi::Py_None()
            };
            if name_obj.is_null() {
                return Err(());
            }

            let fromlist = ffi::PyTuple_New(names.len() as ffi::Py_ssize_t);
            if fromlist.is_null() {
                ffi::Py_DECREF(name_obj);
                return Err(());
            }
            for (idx, name) in names.iter().enumerate() {
                let item = ffi::PyUnicode_FromString(CString::new(name.as_str()).unwrap().as_ptr());
                if item.is_null() {
                    ffi::Py_DECREF(fromlist);
                    ffi::Py_DECREF(name_obj);
                    return Err(());
                }
                if ffi::PyTuple_SetItem(fromlist, idx as _, item) != 0 {
                    ffi::Py_DECREF(fromlist);
                    ffi::Py_DECREF(name_obj);
                    return Err(());
                }
            }

            let module_obj = ffi::PyImport_ImportModuleLevelObject(
                name_obj,
                globals_dict,
                locals_dict,
                fromlist,
                *level as c_int,
            );
            ffi::Py_DECREF(fromlist);
            ffi::Py_DECREF(name_obj);
            ffi::Py_DECREF(globals_dict);
            ffi::Py_DECREF(locals_dict);
            if module_obj.is_null() {
                return Err(());
            }

            for name in names {
                let value = ffi::PyObject_GetAttrString(
                    module_obj,
                    CString::new(name.as_str()).unwrap().as_ptr(),
                );
                if value.is_null() {
                    ffi::Py_DECREF(module_obj);
                    return Err(());
                }
                if scope_assign_name(&mut *ctx.locals, name.as_str(), value).is_err() {
                    ffi::Py_DECREF(value);
                    ffi::Py_DECREF(module_obj);
                    return Err(());
                }
                ffi::Py_DECREF(value);
            }
            ffi::Py_DECREF(module_obj);
            Ok(StmtFlow::Normal)
        },
        min_ast::StmtNode::While { test, body, orelse, .. } => {
            loop {
                let condition = eval_expr(test, ctx)?;
                let truthy = unsafe { ffi::PyObject_IsTrue(condition) };
                unsafe { ffi::Py_DECREF(condition); }
                if truthy < 0 {
                    return Err(());
                }
                if truthy == 0 {
                    break;
                }
                match eval_block(body, ctx)? {
                    StmtFlow::Normal => {}
                    StmtFlow::Continue => continue,
                    StmtFlow::Break => {
                        return Ok(StmtFlow::Normal);
                    }
                    StmtFlow::Return(value) => return Ok(StmtFlow::Return(value)),
                }
            }
            eval_block(orelse, ctx)
        }
        min_ast::StmtNode::If { test, body, orelse, .. } => {
            let condition = eval_expr(test, ctx)?;
            let truthy = unsafe { ffi::PyObject_IsTrue(condition) };
            unsafe { ffi::Py_DECREF(condition); }
            if truthy < 0 {
                return Err(());
            }
            if truthy == 0 {
                eval_block(orelse, ctx)
            } else {
                eval_block(body, ctx)
            }
        }
        min_ast::StmtNode::Try { body, handler, orelse, finalbody, .. } => {
            let mut flow = match eval_block(body, ctx) {
                Ok(flow) => flow,
                Err(()) => {
                    if let Some(handler) = handler {
                        unsafe {
                            let raised = ffi::PyErr_GetRaisedException();
                            ffi::Py_XDECREF(raised);
                        }
                        match eval_block(handler, ctx) {
                            Ok(flow) => flow,
                            Err(()) => return Err(()),
                        }
                    } else {
                        return Err(());
                    }
                }
            };

            if matches!(flow, StmtFlow::Normal) {
                flow = eval_block(orelse, ctx)?;
            }

            let final_flow = eval_block(finalbody, ctx)?;
            if matches!(final_flow, StmtFlow::Normal) {
                Ok(flow)
            } else {
                Ok(final_flow)
            }
        }
        min_ast::StmtNode::Raise { exc, .. } => unsafe {
            if let Some(expr) = exc {
                let value = eval_expr(expr, ctx)?;
                let typ = if ffi::PyExceptionInstance_Check(value) != 0 {
                    ffi::Py_TYPE(value) as *mut ffi::PyObject
                } else {
                    value
                };
                ffi::PyErr_SetObject(typ, value);
                ffi::Py_DECREF(value);
                Err(())
            } else {
                if ffi::PyErr_Occurred().is_null() {
                    ffi::PyErr_SetString(
                        ffi::PyExc_RuntimeError,
                        b"No active exception to reraise\0".as_ptr() as *const c_char,
                    );
                }
                Err(())
            }
        },
        min_ast::StmtNode::Break(_) => Ok(StmtFlow::Break),
        min_ast::StmtNode::Continue(_) => Ok(StmtFlow::Continue),
        min_ast::StmtNode::Return { value, .. } => {
            let result = if let Some(expr) = value {
                eval_expr(expr, ctx)?
            } else {
                unsafe { ffi::Py_INCREF(ffi::Py_None()); }
                unsafe { ffi::Py_None() }
            };
            Ok(StmtFlow::Return(result))
        }
        min_ast::StmtNode::Expr { value, .. } => {
            let result = eval_expr(value, ctx)?;
            unsafe { ffi::Py_DECREF(result); }
            Ok(StmtFlow::Normal)
        }
        min_ast::StmtNode::Assign { target, value, .. } => {
            let result = eval_expr(value, ctx)?;
            let status = unsafe { scope_assign_name(&mut *ctx.locals, target.as_str(), result) };
            unsafe { ffi::Py_DECREF(result); }
            if status.is_err() {
                return Err(());
            }
            Ok(StmtFlow::Normal)
        }
        min_ast::StmtNode::Delete { target, .. } => {
            if unsafe { scope_delete_name(&mut *ctx.locals, target.as_str()) }.is_err() {
                return Err(());
            }
            Ok(StmtFlow::Normal)
        }
        min_ast::StmtNode::Pass(_) => Ok(StmtFlow::Normal),
    }
}

fn eval_expr(expr: &min_ast::ExprNode, ctx: &ExecContext<'_>) -> Result<*mut ffi::PyObject, ()> {
    match expr {
        min_ast::ExprNode::Name { id, .. } => lookup_name(id.as_str(), ctx),
        min_ast::ExprNode::Attribute { value, attr, .. } => {
            let base = eval_expr(value, ctx)?;
            let name = CString::new(attr.as_str()).unwrap();
            let result = unsafe { ffi::PyObject_GetAttrString(base, name.as_ptr()) };
            unsafe { ffi::Py_DECREF(base); }
            if result.is_null() {
                Err(())
            } else {
                Ok(result)
            }
        }
        min_ast::ExprNode::Number { value, .. } => match value {
            min_ast::Number::Int(text) => unsafe {
                let cstr = CString::new(text.as_str()).unwrap();
                let result = ffi::PyLong_FromString(cstr.as_ptr(), ptr::null_mut(), 0);
                if result.is_null() { Err(()) } else { Ok(result) }
            },
            min_ast::Number::Float(text) => unsafe {
                let py_str = ffi::PyUnicode_FromString(CString::new(text.as_str()).unwrap().as_ptr());
                if py_str.is_null() {
                    return Err(());
                }
                let result = ffi::PyFloat_FromString(py_str);
                ffi::Py_DECREF(py_str);
                if result.is_null() { Err(()) } else { Ok(result) }
            },
        },
        min_ast::ExprNode::String { value, .. } => unsafe {
            let cstr = CString::new(value.as_str()).unwrap();
            let result = ffi::PyUnicode_FromString(cstr.as_ptr());
            if result.is_null() { Err(()) } else { Ok(result) }
        },
        min_ast::ExprNode::Bytes { value, .. } => unsafe {
            let result = ffi::PyBytes_FromStringAndSize(value.as_ptr() as *const c_char, value.len() as _);
            if result.is_null() { Err(()) } else { Ok(result) }
        },
        min_ast::ExprNode::Tuple { elts, .. } => {
            let mut values = Vec::with_capacity(elts.len());
            for elt in elts {
                match eval_expr(elt, ctx) {
                    Ok(value) => values.push(value),
                    Err(()) => {
                        unsafe {
                            for value in values {
                                ffi::Py_DECREF(value);
                            }
                        }
                        return Err(());
                    }
                }
            }
            unsafe {
                let tuple = ffi::PyTuple_New(values.len() as _);
                if tuple.is_null() {
                    for value in values {
                        ffi::Py_DECREF(value);
                    }
                    return Err(());
                }
                for (idx, value) in values.into_iter().enumerate() {
                    if ffi::PyTuple_SetItem(tuple, idx as _, value) != 0 {
                        ffi::Py_DECREF(tuple);
                        return Err(());
                    }
                }
                Ok(tuple)
            }
        }
        min_ast::ExprNode::Await { .. } => set_not_implemented("await not supported"),
        min_ast::ExprNode::Yield { .. } => set_not_implemented("yield not supported"),
        min_ast::ExprNode::Call { func, args, .. } => eval_call(func, args, ctx),
    }
}

fn lookup_name(name: &str, ctx: &ExecContext<'_>) -> Result<*mut ffi::PyObject, ()> {
    unsafe {
        let locals = &*ctx.locals;
        if ctx.locals != ctx.globals {
            if let Some(slot) = locals.layout().slot_for(name) {
                let value = scope_get_slot(locals, slot);
                if value.is_null() {
                    if !ctx.params.is_null() {
                        let param_value = scope_lookup_name(&*ctx.params, name);
                        if !param_value.is_null() {
                            return Ok(param_value);
                        }
                    }
                    return set_unbound_local(name);
                }
                return Ok(value);
            }
            let value = scope_get_dynamic(locals, name);
            if !value.is_null() {
                return Ok(value);
            }
            if !ctx.params.is_null() {
                let param_value = scope_lookup_name(&*ctx.params, name);
                if !param_value.is_null() {
                    return Ok(param_value);
                }
            }
        } else {
            let value = scope_lookup_name(locals, name);
            if !value.is_null() {
                return Ok(value);
            }
        }

        if let Some(closure) = ctx.closure {
            let value = scope_lookup_name(closure, name);
            if !value.is_null() {
                return Ok(value);
            }
        }

        if ctx.locals != ctx.globals {
            let value = scope_lookup_name(&*ctx.globals, name);
            if !value.is_null() {
                return Ok(value);
            }
        }

        let value = ffi::PyDict_GetItemString(ctx.builtins, CString::new(name).unwrap().as_ptr());
        if !value.is_null() {
            ffi::Py_INCREF(value);
            return Ok(value);
        }
    }

    set_name_error(name)
}

unsafe fn locals_snapshot(ctx: &ExecContext<'_>) -> Result<*mut ffi::PyObject, ()> {
    let dict = ffi::PyDict_New();
    if dict.is_null() {
        return Err(());
    }
    if !ctx.params.is_null() {
        let params = scope_to_dict(&*ctx.params)?;
        if ffi::PyDict_Update(dict, params) != 0 {
            ffi::Py_DECREF(params);
            ffi::Py_DECREF(dict);
            return Err(());
        }
        ffi::Py_DECREF(params);
    }
    let locals = scope_to_dict(&*ctx.locals)?;
    if ffi::PyDict_Update(dict, locals) != 0 {
        ffi::Py_DECREF(locals);
        ffi::Py_DECREF(dict);
        return Err(());
    }
    ffi::Py_DECREF(locals);
    Ok(dict)
}

fn eval_call(
    func: &min_ast::ExprNode,
    args: &[min_ast::Arg],
    ctx: &ExecContext<'_>,
) -> Result<*mut ffi::PyObject, ()> {
    let func_obj = eval_expr(func, ctx)?;

    unsafe {
        if (func_obj == ctx.runtime_fns.builtins_globals
            || func_obj == ctx.runtime_fns.builtins_locals
            || func_obj == ctx.runtime_fns.dp_globals
            || func_obj == ctx.runtime_fns.dp_locals)
            && args.is_empty()
        {
            let result = if func_obj == ctx.runtime_fns.builtins_locals
                || func_obj == ctx.runtime_fns.dp_locals
            {
                if ctx.locals == ctx.globals {
                    scope_dictproxy_new(ctx.globals, ctx.globals_owner)
                } else {
                    match locals_snapshot(ctx) {
                        Ok(dict) => dict,
                        Err(()) => {
                            ffi::Py_DECREF(func_obj);
                            return Err(());
                        }
                    }
                }
            } else {
                scope_dictproxy_new(ctx.globals, ctx.globals_owner)
            };
            ffi::Py_DECREF(func_obj);
            if result.is_null() {
                return Err(());
            }
            return Ok(result);
        }
    }

    let soac_func = unsafe { get_soac_function(func_obj) };
    if let Some(soac_func) = soac_func {
        let call_args = match collect_call_args(args, ctx) {
            Ok(call_args) => call_args,
            Err(()) => unsafe {
                ffi::Py_DECREF(func_obj);
                return Err(());
            },
        };
        unsafe {
            let mut params_scope = ScopeInstance::new(&*(*soac_func).param_layout);
            let params_ptr = &mut params_scope as *mut ScopeInstance;
            if bind_args_direct(&(*soac_func).params, call_args, params_ptr).is_err() {
                ffi::Py_DECREF(func_obj);
                return Err(());
            }
            let result = (*soac_func).call(params_ptr);
            ffi::Py_DECREF(func_obj);
            return if result.is_null() { Err(()) } else { Ok(result) };
        }
    }

    let mut positional: Vec<*mut ffi::PyObject> = Vec::new();
    let kwargs = unsafe { ffi::PyDict_New() };
    if kwargs.is_null() {
        unsafe { ffi::Py_DECREF(func_obj); }
        return Err(());
    }

    for arg in args {
        match arg {
            min_ast::Arg::Positional(expr) => {
                let value = eval_expr(expr, ctx)?;
                positional.push(value);
            }
            min_ast::Arg::Starred(expr) => unsafe {
                let value = eval_expr(expr, ctx)?;
                let seq = ffi::PySequence_Fast(
                    value,
                    b"argument after * must be iterable\0".as_ptr() as *const c_char,
                );
                ffi::Py_DECREF(value);
                if seq.is_null() {
                    cleanup_call_args(func_obj, kwargs, positional);
                    return Err(());
                }
                let seq_len = ffi::PySequence_Size(seq);
                if seq_len < 0 {
                    ffi::Py_DECREF(seq);
                    cleanup_call_args(func_obj, kwargs, positional);
                    return Err(());
                }
                for idx in 0..seq_len {
                    let item = ffi::PySequence_GetItem(seq, idx);
                    if item.is_null() {
                        ffi::Py_DECREF(seq);
                        cleanup_call_args(func_obj, kwargs, positional);
                        return Err(());
                    }
                    positional.push(item);
                }
                ffi::Py_DECREF(seq);
            },
            min_ast::Arg::Keyword { name, value } => unsafe {
                let val = eval_expr(value, ctx)?;
                let key = CString::new(name.as_str()).unwrap();
                let key_obj = ffi::PyUnicode_FromString(key.as_ptr());
                if key_obj.is_null() {
                    ffi::Py_DECREF(val);
                    cleanup_call_args(func_obj, kwargs, positional);
                    return Err(());
                }
                let contains = ffi::PyDict_Contains(kwargs, key_obj);
                if contains == 1 {
                    ffi::Py_DECREF(val);
                    ffi::Py_DECREF(key_obj);
                    cleanup_call_args(func_obj, kwargs, positional);
                    return set_type_error("multiple values for keyword argument");
                } else if contains < 0 {
                    ffi::Py_DECREF(val);
                    ffi::Py_DECREF(key_obj);
                    cleanup_call_args(func_obj, kwargs, positional);
                    return Err(());
                }
                if ffi::PyDict_SetItem(kwargs, key_obj, val) != 0 {
                    ffi::Py_DECREF(val);
                    ffi::Py_DECREF(key_obj);
                    cleanup_call_args(func_obj, kwargs, positional);
                    return Err(());
                }
                ffi::Py_DECREF(val);
                ffi::Py_DECREF(key_obj);
            },
            min_ast::Arg::KwStarred(expr) => unsafe {
                let mapping = eval_expr(expr, ctx)?;
                let items = ffi::PyMapping_Items(mapping);
                ffi::Py_DECREF(mapping);
                if items.is_null() {
                    cleanup_call_args(func_obj, kwargs, positional);
                    return Err(());
                }
                let items_len = ffi::PySequence_Size(items);
                if items_len < 0 {
                    ffi::Py_DECREF(items);
                    cleanup_call_args(func_obj, kwargs, positional);
                    return Err(());
                }
                for idx in 0..items_len {
                    let item = ffi::PySequence_GetItem(items, idx);
                    if item.is_null() {
                        ffi::Py_DECREF(items);
                        cleanup_call_args(func_obj, kwargs, positional);
                        return Err(());
                    }
                    let key = ffi::PySequence_GetItem(item, 0);
                    let val = ffi::PySequence_GetItem(item, 1);
                    ffi::Py_DECREF(item);
                    if key.is_null() || val.is_null() {
                        ffi::Py_XDECREF(key);
                        ffi::Py_XDECREF(val);
                        ffi::Py_DECREF(items);
                        cleanup_call_args(func_obj, kwargs, positional);
                        return Err(());
                    }
                    if ffi::PyUnicode_Check(key) == 0 {
                        ffi::Py_DECREF(key);
                        ffi::Py_DECREF(val);
                        ffi::Py_DECREF(items);
                        cleanup_call_args(func_obj, kwargs, positional);
                        return set_type_error("keywords must be strings");
                    }
                    let contains = ffi::PyDict_Contains(kwargs, key);
                    if contains == 1 {
                        ffi::Py_DECREF(key);
                        ffi::Py_DECREF(val);
                        ffi::Py_DECREF(items);
                        cleanup_call_args(func_obj, kwargs, positional);
                        return set_type_error("multiple values for keyword argument");
                    } else if contains < 0 {
                        ffi::Py_DECREF(key);
                        ffi::Py_DECREF(val);
                        ffi::Py_DECREF(items);
                        cleanup_call_args(func_obj, kwargs, positional);
                        return Err(());
                    }
                    if ffi::PyDict_SetItem(kwargs, key, val) != 0 {
                        ffi::Py_DECREF(key);
                        ffi::Py_DECREF(val);
                        ffi::Py_DECREF(items);
                        cleanup_call_args(func_obj, kwargs, positional);
                        return Err(());
                    }
                    ffi::Py_DECREF(key);
                    ffi::Py_DECREF(val);
                }
                ffi::Py_DECREF(items);
            },
        }
    }

    let args_tuple = unsafe { ffi::PyTuple_New(positional.len() as _) };
    if args_tuple.is_null() {
        cleanup_call_args(func_obj, kwargs, positional);
        return Err(());
    }

    for (idx, value) in positional.into_iter().enumerate() {
        unsafe {
            if ffi::PyTuple_SetItem(args_tuple, idx as _, value) != 0 {
                ffi::Py_DECREF(args_tuple);
                ffi::Py_DECREF(func_obj);
                ffi::Py_DECREF(kwargs);
                return Err(());
            }
        }
    }

    let call_kwargs = unsafe {
        if ffi::PyDict_Size(kwargs) == 0 {
            ffi::Py_DECREF(kwargs);
            ptr::null_mut()
        } else {
            kwargs
        }
    };

    let result = unsafe { ffi::PyObject_Call(func_obj, args_tuple, call_kwargs) };
    unsafe {
        ffi::Py_DECREF(func_obj);
        ffi::Py_DECREF(args_tuple);
        if !call_kwargs.is_null() {
            ffi::Py_DECREF(call_kwargs);
        }
    }

    if result.is_null() {
        Err(())
    } else {
        Ok(result)
    }
}

fn cleanup_call_args(
    func: *mut ffi::PyObject,
    kwargs: *mut ffi::PyObject,
    positional: Vec<*mut ffi::PyObject>,
) {
    unsafe {
        ffi::Py_DECREF(func);
        ffi::Py_DECREF(kwargs);
        for value in positional {
            ffi::Py_DECREF(value);
        }
    }
}

unsafe fn get_soac_function(func_obj: *mut ffi::PyObject) -> Option<*const FunctionData> {
    if ffi::PyCFunction_Check(func_obj) == 0 {
        return None;
    }
    let cfunc = ffi::PyCFunction_GetFunction(func_obj);
    let soac_ptr = soac_function_call as *const c_void;
    let Some(cfunc) = cfunc else {
        return None;
    };
    let cfunc_ptr = cfunc as *const c_void;
    if cfunc_ptr != soac_ptr {
        return None;
    }
    let capsule = ffi::PyCFunction_GetSelf(func_obj);
    if capsule.is_null() {
        return None;
    }
    if ffi::PyCapsule_IsValid(capsule, SOAC_FUNCTION_CAPSULE.as_ptr() as *const c_char) == 0 {
        return None;
    }
    let ptr =
        ffi::PyCapsule_GetPointer(capsule, SOAC_FUNCTION_CAPSULE.as_ptr() as *const c_char);
    if ptr.is_null() {
        return None;
    }
    Some(ptr as *const FunctionData)
}

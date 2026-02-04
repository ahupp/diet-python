use super::eval_genawait::stmt_has_yield;
use super::*;

type TypeParamScope = HashMap<String, *mut ffi::PyObject>;

unsafe extern "C" {
    fn PyUnstable_InterpreterFrame_GetCode(
        frame: *mut ffi::_PyInterpreterFrame,
    ) -> *mut ffi::PyObject;
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
    fn _PyInterpreterState_SetEvalFrameFunc(
        interp: *mut ffi::PyInterpreterState,
        eval_frame: extern "C" fn(
            tstate: *mut ffi::PyThreadState,
            frame: *mut ffi::_PyInterpreterFrame,
            throwflag: c_int,
        ) -> *mut ffi::PyObject,
    );
    fn _PyEval_EvalFrameDefault(
        tstate: *mut ffi::PyThreadState,
        frame: *mut ffi::_PyInterpreterFrame,
        throwflag: c_int,
    ) -> *mut ffi::PyObject;
    fn _PyEval_FrameClearAndPop(
        tstate: *mut ffi::PyThreadState,
        frame: *mut ffi::_PyInterpreterFrame,
    );
    fn _PyFrame_MakeAndSetFrameObject(
        frame: *mut ffi::_PyInterpreterFrame,
    ) -> *mut ffi::PyFrameObject;
    fn PyCell_New(obj: *mut ffi::PyObject) -> *mut ffi::PyObject;
}

static INIT_EVAL_FRAME_HOOK: Once = Once::new();
static mut SOAC_CODE_EXTRA_INDEX: ffi::Py_ssize_t = -1;
const SOAC_CODE_EXTRA_MAGIC: u64 = 0x44505f534f41435f;
#[repr(C)]
struct SoacCodeExtra {
    magic: u64,
    data: *mut FunctionData,
}

impl Drop for SoacCodeExtra {
    fn drop(&mut self) {
        unsafe {
            if !self.data.is_null() {
                drop(Box::from_raw(self.data));
                self.data = ptr::null_mut();
            }
        }
    }
}

pub(crate) struct ClosureScope {
    pub(crate) _layout: Box<ScopeLayout>,
    pub(crate) scope: ScopeInstance,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ParamKind {
    Positional,
    VarArg,
    KwOnly,
    KwArg,
}

pub(crate) struct ParamSpec {
    pub(crate) name: String,
    pub(crate) kind: ParamKind,
    pub(crate) default: Option<*mut ffi::PyObject>,
}

impl ParamSpec {
    fn drop_default(&mut self) {
        if let Some(value) = self.default.take() {
            unsafe { ffi::Py_DECREF(value) };
        }
    }
}

pub(crate) struct FunctionData {
    pub(crate) def: min_ast::FunctionDef,
    pub(crate) params: Vec<ParamSpec>,
    pub(crate) param_layout: Box<ScopeLayout>,
    pub(crate) local_layout: Box<ScopeLayout>,
    pub(crate) closure: Option<ClosureScope>,
    pub(crate) type_params: Option<TypeParamState>,
    pub(crate) has_yield: bool,
    pub(crate) globals_scope: *mut ScopeInstance,
    pub(crate) globals_dict: *mut ffi::PyObject,
    pub(crate) builtins: *mut ffi::PyObject,
    pub(crate) function_codes: *mut ffi::PyObject,
    pub(crate) runtime_fns: RuntimeFns,
}

struct TypeParamState {
    map: TypeParamScope,
    ordered: Vec<*mut ffi::PyObject>,
}

impl Drop for TypeParamState {
    fn drop(&mut self) {
        unsafe {
            for value in self.ordered.drain(..) {
                ffi::Py_DECREF(value);
            }
        }
    }
}

impl Drop for FunctionData {
    fn drop(&mut self) {
        unsafe {
            for param in &mut self.params {
                param.drop_default();
            }
            if !self.globals_dict.is_null() {
                ffi::Py_DECREF(self.globals_dict);
            }
            ffi::Py_DECREF(self.builtins);
            if !self.function_codes.is_null() {
                ffi::Py_DECREF(self.function_codes);
            }
        }
    }
}

pub struct ExecContext<'a> {
    globals_scope: *mut ScopeInstance,
    globals_dict: *mut ffi::PyObject,
    params: *mut ScopeInstance,
    locals: *mut ScopeInstance,
    builtins: *mut ffi::PyObject,
    function_codes: *mut ffi::PyObject,
    closure: Option<&'a ScopeInstance>,
    runtime_fns: &'a RuntimeFns,
    type_params: Option<&'a TypeParamScope>,
}

#[derive(Debug, PartialEq, Eq)]
enum StmtFlow {
    Normal,
    Break,
    Continue,
    Return(*mut ffi::PyObject),
}

pub(crate) fn set_type_error<T>(msg: &str) -> Result<T, ()> {
    unsafe {
        ffi::PyErr_SetString(ffi::PyExc_TypeError, CString::new(msg).unwrap().as_ptr());
    }
    Err(())
}

fn set_runtime_error<T>(msg: &str) -> Result<T, ()> {
    unsafe {
        ffi::PyErr_SetString(ffi::PyExc_RuntimeError, CString::new(msg).unwrap().as_ptr());
    }
    Err(())
}

pub(crate) fn set_name_error<T>(name: &str) -> Result<T, ()> {
    unsafe {
        let msg = CString::new(format!("name '{name}' is not defined")).unwrap();
        ffi::PyErr_SetString(ffi::PyExc_NameError, msg.as_ptr());
    }
    Err(())
}

fn set_unbound_local<T>(name: &str) -> Result<T, ()> {
    unsafe {
        let msg = CString::new(format!(
            "local variable '{name}' referenced before assignment"
        ))
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

unsafe extern "C" fn soac_code_extra_free(ptr: *mut c_void) {
    let _ = ptr;
}

unsafe fn soac_code_extra_index() -> Result<ffi::Py_ssize_t, ()> {
    if SOAC_CODE_EXTRA_INDEX >= 0 {
        return Ok(SOAC_CODE_EXTRA_INDEX);
    }
    let index = PyUnstable_Eval_RequestCodeExtraIndex(soac_code_extra_free);
    if index < 0 {
        return Err(());
    }
    SOAC_CODE_EXTRA_INDEX = index as ffi::Py_ssize_t;
    Ok(SOAC_CODE_EXTRA_INDEX)
}

unsafe fn get_frame_data_for_code(code: *mut ffi::PyObject) -> Option<*const FunctionData> {
    if ffi::PyObject_TypeCheck(code, std::ptr::addr_of_mut!(ffi::PyCode_Type)) == 0 {
        return None;
    }
    let index = match soac_code_extra_index() {
        Ok(index) => index,
        Err(()) => return None,
    };
    let mut extra: *mut c_void = ptr::null_mut();
    let status = PyUnstable_Code_GetExtra(code, index, &mut extra as *mut *mut c_void);
    if status != 0 || extra.is_null() {
        return None;
    }
    let tagged = extra as *const SoacCodeExtra;
    if tagged.is_null() || (*tagged).magic != SOAC_CODE_EXTRA_MAGIC || (*tagged).data.is_null() {
        return None;
    }
    Some((*tagged).data as *const FunctionData)
}

unsafe fn frame_var_get_optional(
    frame_obj: *mut ffi::PyFrameObject,
    name: &str,
) -> Result<*mut ffi::PyObject, ()> {
    let c_name = match CString::new(name) {
        Ok(name) => name,
        Err(_) => return Err(()),
    };
    let value = ffi::PyFrame_GetVarString(frame_obj, c_name.as_ptr() as *mut c_char);
    if value.is_null() {
        if ffi::PyErr_ExceptionMatches(ffi::PyExc_NameError) != 0
            || ffi::PyErr_ExceptionMatches(ffi::PyExc_UnboundLocalError) != 0
        {
            ffi::PyErr_Clear();
            return Ok(ptr::null_mut());
        }
        return Err(());
    }
    Ok(value)
}

unsafe fn eval_frame_with_data(
    data: &FunctionData,
    frame_obj: *mut ffi::PyObject,
) -> *mut ffi::PyObject {
    let frame = frame_obj as *mut ffi::PyFrameObject;
    // Use the active Python frame mappings so eval-mode name resolution matches
    // real FunctionType execution when globals/builtins differ per call frame.
    let globals_dict = ffi::PyFrame_GetGlobals(frame);
    if globals_dict.is_null() {
        return ptr::null_mut();
    }
    let builtins = ffi::PyFrame_GetBuiltins(frame);
    if builtins.is_null() {
        ffi::Py_DECREF(globals_dict);
        return ptr::null_mut();
    }

    let mut params_scope = ScopeInstance::new(&*data.param_layout);
    let params_ptr = &mut params_scope as *mut ScopeInstance;
    for param in &data.params {
        let value = match frame_var_get_optional(frame, param.name.as_str()) {
            Ok(value) => value,
            Err(()) => {
                ffi::Py_DECREF(globals_dict);
                ffi::Py_DECREF(builtins);
                return ptr::null_mut();
            }
        };
        if value.is_null() {
            continue;
        }
        if scope_assign_name(&mut params_scope, param.name.as_str(), value).is_err() {
            ffi::Py_DECREF(value);
            ffi::Py_DECREF(globals_dict);
            ffi::Py_DECREF(builtins);
            return ptr::null_mut();
        }
        ffi::Py_DECREF(value);
    }

    if apply_param_defaults(&data.params, params_ptr).is_err() {
        ffi::Py_DECREF(globals_dict);
        ffi::Py_DECREF(builtins);
        return ptr::null_mut();
    }

    let mut locals_box = Box::new(ScopeInstance::new(&*data.local_layout));
    let locals_ptr = locals_box.as_mut() as *mut ScopeInstance;
    let ctx = ExecContext {
        globals_scope: data.globals_scope,
        globals_dict,
        params: params_ptr,
        locals: locals_ptr,
        builtins,
        function_codes: data.function_codes,
        closure: data.closure.as_ref().map(|closure| &closure.scope),
        runtime_fns: &data.runtime_fns,
        type_params: None,
    };

    let result = match eval_block(&data.def.body, &ctx) {
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
    ffi::Py_DECREF(globals_dict);
    ffi::Py_DECREF(builtins);
    result
}

unsafe fn eval_frame_with_data_no_frame(data: &FunctionData) -> *mut ffi::PyObject {
    let mut params_scope = ScopeInstance::new(&*data.param_layout);
    let params_ptr = &mut params_scope as *mut ScopeInstance;
    if apply_param_defaults(&data.params, params_ptr).is_err() {
        return ptr::null_mut();
    }

    let mut locals_box = Box::new(ScopeInstance::new(&*data.local_layout));
    let locals_ptr = locals_box.as_mut() as *mut ScopeInstance;
    let ctx = ExecContext {
        globals_scope: data.globals_scope,
        globals_dict: data.globals_dict,
        params: params_ptr,
        locals: locals_ptr,
        builtins: data.builtins,
        function_codes: data.function_codes,
        closure: data.closure.as_ref().map(|closure| &closure.scope),
        runtime_fns: &data.runtime_fns,
        type_params: None,
    };

    let result = match eval_block(&data.def.body, &ctx) {
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

unsafe fn find_matching_soac_frame(
    tstate: *mut ffi::PyThreadState,
    data_ptr: *const FunctionData,
) -> *mut ffi::PyFrameObject {
    let mut frame_obj = ffi::PyThreadState_GetFrame(tstate);
    while !frame_obj.is_null() {
        let code = ffi::PyFrame_GetCode(frame_obj);
        if !code.is_null() {
            let code_obj = code as *mut ffi::PyObject;
            let frame_data = get_frame_data_for_code(code_obj);
            ffi::Py_DECREF(code_obj);
            if let Some(candidate) = frame_data {
                if std::ptr::eq(candidate, data_ptr) {
                    return frame_obj;
                }
            }
        } else {
            ffi::PyErr_Clear();
        }

        let back = ffi::PyFrame_GetBack(frame_obj);
        ffi::Py_DECREF(frame_obj as *mut ffi::PyObject);
        frame_obj = back;
    }
    ptr::null_mut()
}

unsafe fn finalize_soac_frame(
    tstate: *mut ffi::PyThreadState,
    frame: *mut ffi::_PyInterpreterFrame,
) {
    _PyEval_FrameClearAndPop(tstate, frame);
}

extern "C" fn soac_eval_frame(
    tstate: *mut ffi::PyThreadState,
    frame: *mut ffi::_PyInterpreterFrame,
    throwflag: c_int,
) -> *mut ffi::PyObject {
    unsafe {
        let code = PyUnstable_InterpreterFrame_GetCode(frame);
        if code.is_null() {
            return _PyEval_EvalFrameDefault(tstate, frame, throwflag);
        }
        let data = get_frame_data_for_code(code);
        ffi::Py_DECREF(code);
        let Some(data_ptr) = data else {
            return _PyEval_EvalFrameDefault(tstate, frame, throwflag);
        };
        if throwflag != 0 || (*data_ptr).has_yield || (*data_ptr).def.is_async {
            return _PyEval_EvalFrameDefault(tstate, frame, throwflag);
        }
        if (*data_ptr).params.is_empty() {
            let result = eval_frame_with_data_no_frame(&*data_ptr);
            finalize_soac_frame(tstate, frame);
            return result;
        }
        let mut frame_obj = find_matching_soac_frame(tstate, data_ptr);
        if frame_obj.is_null() {
            frame_obj = _PyFrame_MakeAndSetFrameObject(frame);
            if frame_obj.is_null() {
                return ptr::null_mut();
            }
            ffi::Py_INCREF(frame_obj as *mut ffi::PyObject);
        }
        let result = eval_frame_with_data(&*data_ptr, frame_obj as *mut ffi::PyObject);
        ffi::Py_DECREF(frame_obj as *mut ffi::PyObject);
        finalize_soac_frame(tstate, frame);
        result
    }
}

pub unsafe fn install_eval_frame_hook() -> Result<(), ()> {
    let mut ok = true;
    INIT_EVAL_FRAME_HOOK.call_once(|| {
        if soac_code_extra_index().is_err() {
            ok = false;
            return;
        }
        let interp = ffi::PyInterpreterState_Get();
        if interp.is_null() {
            ok = false;
            return;
        }
        _PyInterpreterState_SetEvalFrameFunc(interp, soac_eval_frame);
    });
    if ok { Ok(()) } else { Err(()) }
}

fn context_with_type_params<'a>(
    ctx: &ExecContext<'a>,
    type_params: &'a TypeParamScope,
) -> ExecContext<'a> {
    ExecContext {
        globals_scope: ctx.globals_scope,
        globals_dict: ctx.globals_dict,
        params: ctx.params,
        locals: ctx.locals,
        builtins: ctx.builtins,
        function_codes: ctx.function_codes,
        closure: ctx.closure,
        runtime_fns: ctx.runtime_fns,
        type_params: Some(type_params),
    }
}

pub(crate) fn exec_context_for_scopes<'a>(
    data: &'a FunctionData,
    params: *mut ScopeInstance,
    locals: *mut ScopeInstance,
) -> ExecContext<'a> {
    ExecContext {
        globals_scope: data.globals_scope,
        globals_dict: data.globals_dict,
        params,
        locals,
        builtins: data.builtins,
        function_codes: data.function_codes,
        closure: data.closure.as_ref().map(|closure| &closure.scope),
        runtime_fns: &data.runtime_fns,
        type_params: None,
    }
}

unsafe fn build_type_params(
    def: &min_ast::FunctionDef,
    ctx: &ExecContext<'_>,
) -> Result<Option<TypeParamState>, ()> {
    if def.type_params.is_empty() {
        return Ok(None);
    }

    let typing = ffi::PyImport_ImportModule(b"typing\0".as_ptr() as *const c_char);
    if typing.is_null() {
        return Err(());
    }
    let type_var = ffi::PyObject_GetAttrString(typing, b"TypeVar\0".as_ptr() as *const c_char);
    let type_var_tuple =
        ffi::PyObject_GetAttrString(typing, b"TypeVarTuple\0".as_ptr() as *const c_char);
    let param_spec = ffi::PyObject_GetAttrString(typing, b"ParamSpec\0".as_ptr() as *const c_char);

    let mut state = TypeParamState {
        map: HashMap::new(),
        ordered: Vec::new(),
    };

    let mut ok = true;
    for param in &def.type_params {
        if !ok {
            break;
        }
        let (name, bound, default, factory) = match param {
            min_ast::TypeParam::TypeVar {
                name,
                bound,
                default,
            } => {
                if type_var.is_null() {
                    ok = false;
                    break;
                }
                (name.as_str(), bound.as_ref(), default.as_ref(), type_var)
            }
            min_ast::TypeParam::TypeVarTuple { name, default } => {
                if type_var_tuple.is_null() {
                    ok = false;
                    break;
                }
                (name.as_str(), None, default.as_ref(), type_var_tuple)
            }
            min_ast::TypeParam::ParamSpec { name, default } => {
                if param_spec.is_null() {
                    ok = false;
                    break;
                }
                (name.as_str(), None, default.as_ref(), param_spec)
            }
        };

        let name_obj =
            ffi::PyUnicode_FromString(CString::new(name).unwrap().as_ptr() as *const c_char);
        if name_obj.is_null() {
            ok = false;
            break;
        }

        let args = ffi::PyTuple_New(1);
        if args.is_null() {
            ffi::Py_DECREF(name_obj);
            ok = false;
            break;
        }
        ffi::PyTuple_SetItem(args, 0, name_obj);

        let kwargs_needed = bound.is_some() || default.is_some();
        let kwargs = if kwargs_needed {
            let dict = ffi::PyDict_New();
            if dict.is_null() {
                ffi::Py_DECREF(args);
                ok = false;
                break;
            }
            dict
        } else {
            ptr::null_mut()
        };

        if let Some(bound_expr) = bound {
            let ann_ctx = context_with_type_params(ctx, &state.map);
            let bound_value = match eval_expr(bound_expr, &ann_ctx) {
                Ok(value) => value,
                Err(()) => {
                    ffi::Py_DECREF(args);
                    if !kwargs.is_null() {
                        ffi::Py_DECREF(kwargs);
                    }
                    ok = false;
                    break;
                }
            };
            if ffi::PyDict_SetItemString(kwargs, b"bound\0".as_ptr() as *const c_char, bound_value)
                != 0
            {
                ffi::Py_DECREF(bound_value);
                ffi::Py_DECREF(args);
                if !kwargs.is_null() {
                    ffi::Py_DECREF(kwargs);
                }
                ok = false;
                break;
            }
            ffi::Py_DECREF(bound_value);
        }

        if let Some(default_expr) = default {
            let ann_ctx = context_with_type_params(ctx, &state.map);
            let default_value = match eval_expr(default_expr, &ann_ctx) {
                Ok(value) => value,
                Err(()) => {
                    ffi::Py_DECREF(args);
                    if !kwargs.is_null() {
                        ffi::Py_DECREF(kwargs);
                    }
                    ok = false;
                    break;
                }
            };
            if ffi::PyDict_SetItemString(
                kwargs,
                b"default\0".as_ptr() as *const c_char,
                default_value,
            ) != 0
            {
                ffi::Py_DECREF(default_value);
                ffi::Py_DECREF(args);
                if !kwargs.is_null() {
                    ffi::Py_DECREF(kwargs);
                }
                ok = false;
                break;
            }
            ffi::Py_DECREF(default_value);
        }

        let obj = ffi::PyObject_Call(factory, args, kwargs);
        ffi::Py_DECREF(args);
        if !kwargs.is_null() {
            ffi::Py_DECREF(kwargs);
        }
        if obj.is_null() {
            ok = false;
            break;
        }
        state.map.insert(name.to_string(), obj);
        state.ordered.push(obj);
    }

    ffi::Py_XDECREF(type_var);
    ffi::Py_XDECREF(type_var_tuple);
    ffi::Py_XDECREF(param_spec);
    ffi::Py_DECREF(typing);

    if !ok {
        return Err(());
    }

    Ok(Some(state))
}

fn collect_bound_names(stmts: &[min_ast::StmtNode], names: &mut HashSet<String>) {
    for stmt in stmts {
        match stmt {
            min_ast::StmtNode::Assign { target, .. } | min_ast::StmtNode::Delete { target, .. } => {
                names.insert(target.clone());
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

pub fn build_module_layout(module: &min_ast::Module) -> ScopeLayout {
    let mut names = HashSet::new();
    collect_bound_names(&module.body, &mut names);
    ScopeLayout::new(names)
}

fn capture_closure(freevars: &[String], ctx: &ExecContext<'_>) -> Result<Option<ClosureScope>, ()> {
    unsafe fn cleanup_captured(captured: &mut Vec<(String, *mut ffi::PyObject)>) {
        for (_, value) in captured.drain(..) {
            ffi::Py_DECREF(value);
        }
    }

    unsafe fn capture_if_cell(
        captured: &mut Vec<(String, *mut ffi::PyObject)>,
        name: &str,
        value: *mut ffi::PyObject,
    ) -> Result<bool, ()> {
        if value.is_null() {
            return Ok(false);
        }
        if ffi::PyObject_TypeCheck(value, std::ptr::addr_of_mut!(PyCell_Type)) == 0 {
            if closure_name_requires_cell(name) {
                ffi::Py_DECREF(value);
                cleanup_captured(captured);
                set_runtime_error("closure values must be cells")?;
            }
            // `_dp_cell_*` / `_dp_classcell` are explicit transformed cell bindings and must
            // already be cells. Other captured names can be implicit CPython cellvars
            // (for example nested def/class binding names), so eval mode promotes them.
            let promoted = PyCell_New(value);
            ffi::Py_DECREF(value);
            if promoted.is_null() {
                cleanup_captured(captured);
                return Err(());
            }
            captured.push((name.to_string(), promoted));
            return Ok(true);
        }
        captured.push((name.to_string(), value));
        Ok(true)
    }

    let mut captured = Vec::new();
    for name in freevars {
        unsafe {
            if ctx.locals != ctx.globals_scope {
                let locals_scope = &*ctx.locals;
                let value = scope_lookup_name(locals_scope, name);
                if capture_if_cell(&mut captured, name.as_str(), value)? {
                    continue;
                }
            }
            if !ctx.params.is_null() {
                let params_scope = &*ctx.params;
                let value = scope_lookup_name(params_scope, name);
                if capture_if_cell(&mut captured, name.as_str(), value)? {
                    continue;
                }
            }
            if let Some(closure) = ctx.closure {
                let value = scope_lookup_name(closure, name);
                if capture_if_cell(&mut captured, name.as_str(), value)? {
                    continue;
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

    Ok(Some(ClosureScope {
        _layout: layout,
        scope,
    }))
}

impl FunctionData {
    pub(crate) unsafe fn call_from_python(
        &self,
        args: *mut ffi::PyObject,
        kwargs: *mut ffi::PyObject,
    ) -> *mut ffi::PyObject {
        if self.has_yield {
            ffi::PyErr_SetString(
                ffi::PyExc_NotImplementedError,
                b"yield not supported\0".as_ptr() as *const c_char,
            );
            return ptr::null_mut();
        }
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
            if std::env::var_os("DIET_PYTHON_DEBUG_BIND").is_some() {
                eprintln!("bind_args failed for {}", self.def.name);
            }
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
            globals_scope: self.globals_scope,
            globals_dict: self.globals_dict,
            params: params_scope,
            locals: locals_ptr,
            builtins: self.builtins,
            closure: self.closure.as_ref().map(|closure| &closure.scope),
        runtime_fns: &self.runtime_fns,
        function_codes: self.function_codes,
        type_params: None,
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
    globals_scope: *mut ScopeInstance,
    globals_dict: *mut ffi::PyObject,
    builtins: *mut ffi::PyObject,
    function_codes: *mut ffi::PyObject,
    runtime_fns: &RuntimeFns,
) -> Result<(), ()> {
    let ctx = ExecContext {
        globals_scope,
        globals_dict,
        params: ptr::null_mut(),
        locals: globals_scope,
        builtins,
        function_codes,
        closure: None,
        runtime_fns,
        type_params: None,
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

fn normalize_freevar_name(name: &str) -> &str {
    name.strip_prefix("_dp_cell_").unwrap_or(name)
}

fn closure_name_requires_cell(name: &str) -> bool {
    name.starts_with("_dp_cell_") || name == "_dp_classcell"
}

unsafe fn function_defaults_object(params: &[ParamSpec]) -> Result<*mut ffi::PyObject, ()> {
    let mut defaults = Vec::new();
    let mut seen_default = false;
    for param in params {
        if param.kind == ParamKind::Positional {
            if let Some(value) = param.default {
                seen_default = true;
                defaults.push(value);
            } else if seen_default {
                break;
            }
        }
    }
    if defaults.is_empty() {
        ffi::Py_INCREF(ffi::Py_None());
        return Ok(ffi::Py_None());
    }
    let tuple = ffi::PyTuple_New(defaults.len() as ffi::Py_ssize_t);
    if tuple.is_null() {
        return Err(());
    }
    for (idx, value) in defaults.iter().enumerate() {
        ffi::Py_INCREF(*value);
        if ffi::PyTuple_SetItem(tuple, idx as ffi::Py_ssize_t, *value) != 0 {
            ffi::Py_DECREF(tuple);
            return Err(());
        }
    }
    Ok(tuple)
}

unsafe fn function_kwdefaults_object(params: &[ParamSpec]) -> Result<*mut ffi::PyObject, ()> {
    let mut dict: *mut ffi::PyObject = ptr::null_mut();
    for param in params {
        if param.kind != ParamKind::KwOnly {
            continue;
        }
        let Some(value) = param.default else {
            continue;
        };
        if dict.is_null() {
            dict = ffi::PyDict_New();
            if dict.is_null() {
                return Err(());
            }
        }
        let key = CString::new(param.name.as_str()).unwrap();
        if ffi::PyDict_SetItemString(dict, key.as_ptr(), value) != 0 {
            ffi::Py_DECREF(dict);
            return Err(());
        }
    }
    if dict.is_null() {
        ffi::Py_INCREF(ffi::Py_None());
        return Ok(ffi::Py_None());
    }
    Ok(dict)
}

unsafe fn function_closure_object(
    data: &FunctionData,
    code: *mut ffi::PyObject,
) -> Result<*mut ffi::PyObject, ()> {
    let Some(closure) = data.closure.as_ref() else {
        ffi::Py_INCREF(ffi::Py_None());
        return Ok(ffi::Py_None());
    };
    if ffi::PyObject_TypeCheck(code, std::ptr::addr_of_mut!(ffi::PyCode_Type)) != 0 {
        let freevars = ffi::PyObject_GetAttrString(code, b"co_freevars\0".as_ptr() as *const c_char);
        if freevars.is_null() {
            return Err(());
        }
        let freevars_len = ffi::PyTuple_Size(freevars);
        if freevars_len < 0 {
            ffi::Py_DECREF(freevars);
            return Err(());
        }
        let tuple = ffi::PyTuple_New(freevars_len);
        if tuple.is_null() {
            ffi::Py_DECREF(freevars);
            return Err(());
        }
        for idx in 0..freevars_len {
            let name_obj = ffi::PyTuple_GetItem(freevars, idx);
            if name_obj.is_null() || ffi::PyUnicode_Check(name_obj) == 0 {
                ffi::Py_DECREF(tuple);
                ffi::Py_DECREF(freevars);
                ffi::PyErr_SetString(
                    ffi::PyExc_TypeError,
                    b"co_freevars must contain strings\0".as_ptr() as *const c_char,
                );
                return Err(());
            }
            let name_ptr = ffi::PyUnicode_AsUTF8(name_obj);
            if name_ptr.is_null() {
                ffi::Py_DECREF(tuple);
                ffi::Py_DECREF(freevars);
                return Err(());
            }
            let name = std::ffi::CStr::from_ptr(name_ptr).to_string_lossy();
            let mut value = scope_lookup_name(&closure.scope, name.as_ref());
            if value.is_null() && !name.starts_with("_dp_cell_") {
                let cell_name = format!("_dp_cell_{name}");
                value = scope_lookup_name(&closure.scope, cell_name.as_str());
            }
            if value.is_null() {
                ffi::Py_DECREF(tuple);
                ffi::Py_DECREF(freevars);
                ffi::PyErr_SetString(
                    ffi::PyExc_RuntimeError,
                    b"closure value missing for compiled freevar\0".as_ptr() as *const c_char,
                );
                return Err(());
            }
            if ffi::PyObject_TypeCheck(value, std::ptr::addr_of_mut!(PyCell_Type)) == 0 {
                ffi::Py_DECREF(value);
                ffi::Py_DECREF(tuple);
                ffi::Py_DECREF(freevars);
                ffi::PyErr_SetString(
                    ffi::PyExc_TypeError,
                    b"closure values must be cells\0".as_ptr() as *const c_char,
                );
                return Err(());
            }
            let tuple_value = if closure_name_requires_cell(name.as_ref()) {
                // Compiled CPython bytecode loads freevars as cell contents. For transformed
                // `_dp_cell_*` bindings we need the value itself to remain a cell object, so
                // wrap the captured cell in an outer closure cell.
                // TODO: eliminate this double-cell representation by aligning transformed
                // closure conventions with CPython freevar expectations directly.
                let wrapped = PyCell_New(value);
                ffi::Py_DECREF(value);
                if wrapped.is_null() {
                    ffi::Py_DECREF(tuple);
                    ffi::Py_DECREF(freevars);
                    return Err(());
                }
                wrapped
            } else {
                value
            };
            if ffi::PyTuple_SetItem(tuple, idx, tuple_value) != 0 {
                ffi::Py_DECREF(tuple_value);
                ffi::Py_DECREF(tuple);
                ffi::Py_DECREF(freevars);
                return Err(());
            }
        }
        ffi::Py_DECREF(freevars);
        return Ok(tuple);
    }

    let tuple = ffi::PyTuple_New(closure._layout.names.len() as ffi::Py_ssize_t);
    if tuple.is_null() {
        return Err(());
    }
    for (idx, name) in closure._layout.names.iter().enumerate() {
        let value = scope_lookup_name(&closure.scope, name.as_str());
        if value.is_null() {
            ffi::Py_DECREF(tuple);
            return Err(());
        }
        if ffi::PyObject_TypeCheck(value, std::ptr::addr_of_mut!(PyCell_Type)) == 0 {
            ffi::Py_DECREF(value);
            ffi::Py_DECREF(tuple);
            ffi::PyErr_SetString(
                ffi::PyExc_TypeError,
                b"closure values must be cells\0".as_ptr() as *const c_char,
            );
            return Err(());
        }
        if ffi::PyTuple_SetItem(tuple, idx as ffi::Py_ssize_t, value) != 0 {
            ffi::Py_DECREF(value);
            ffi::Py_DECREF(tuple);
            return Err(());
        }
    }
    Ok(tuple)
}

unsafe fn function_code_object(data: &FunctionData) -> Result<*mut ffi::PyObject, ()> {
    if !data.function_codes.is_null() {
        let code = ffi::PyDict_GetItemString(
            data.function_codes,
            CString::new(data.def.name.as_str()).unwrap().as_ptr(),
        );
        if !code.is_null() {
            if ffi::PyObject_TypeCheck(code, std::ptr::addr_of_mut!(ffi::PyCode_Type)) == 0 {
                ffi::PyErr_SetString(
                    ffi::PyExc_TypeError,
                    b"function code map value is not a code object\0".as_ptr() as *const c_char,
                );
                return Err(());
            }
            ffi::Py_INCREF(code);
            let kwargs = ffi::PyDict_New();
            let co_name_obj = ffi::PyUnicode_FromString(
                CString::new(data.def.display_name.as_str())
                    .unwrap()
                    .as_ptr(),
            );
            let qualname_obj = ffi::PyUnicode_FromString(
                CString::new(data.def.qualname.as_str()).unwrap().as_ptr(),
            );
            if kwargs.is_null() || co_name_obj.is_null() || qualname_obj.is_null() {
                ffi::Py_XDECREF(kwargs);
                ffi::Py_XDECREF(co_name_obj);
                ffi::Py_XDECREF(qualname_obj);
                ffi::Py_DECREF(code);
                return Err(());
            }
            let ok =
                ffi::PyDict_SetItemString(kwargs, b"co_name\0".as_ptr() as *const c_char, co_name_obj)
                    == 0
                    && ffi::PyDict_SetItemString(
                        kwargs,
                        b"co_qualname\0".as_ptr() as *const c_char,
                        qualname_obj,
                    ) == 0;
            ffi::Py_DECREF(co_name_obj);
            ffi::Py_DECREF(qualname_obj);
            if !ok {
                ffi::Py_DECREF(kwargs);
                ffi::Py_DECREF(code);
                return Err(());
            }
            let replace = ffi::PyObject_GetAttrString(code, b"replace\0".as_ptr() as *const c_char);
            let args = ffi::PyTuple_New(0);
            if replace.is_null() || args.is_null() {
                ffi::Py_XDECREF(replace);
                ffi::Py_XDECREF(args);
                ffi::Py_DECREF(kwargs);
                ffi::Py_DECREF(code);
                return Err(());
            }
            let replaced = ffi::PyObject_Call(replace, args, kwargs);
            ffi::Py_DECREF(replace);
            ffi::Py_DECREF(args);
            ffi::Py_DECREF(kwargs);
            if replaced.is_null() {
                // Keep compiled code if replace fails.
                ffi::PyErr_Clear();
                return Ok(code);
            }
            ffi::Py_DECREF(code);
            return Ok(replaced);
        }
    }

    let mut positional = Vec::new();
    let mut kwonly = Vec::new();
    let mut vararg = None;
    let mut kwarg = None;
    for param in &data.params {
        match param.kind {
            ParamKind::Positional => positional.push(param.name.clone()),
            ParamKind::VarArg => vararg = Some(param.name.clone()),
            ParamKind::KwOnly => kwonly.push(param.name.clone()),
            ParamKind::KwArg => kwarg = Some(param.name.clone()),
        }
    }
    let mut varnames = Vec::new();
    varnames.extend(positional.iter().cloned());
    if let Some(name) = vararg.as_ref() {
        varnames.push(name.clone());
    }
    varnames.extend(kwonly.iter().cloned());
    if let Some(name) = kwarg.as_ref() {
        varnames.push(name.clone());
    }

    let filename_obj = {
        let file =
            ffi::PyDict_GetItemString(data.globals_dict, b"__file__\0".as_ptr() as *const c_char);
        if !file.is_null() {
            ffi::Py_INCREF(file);
        }
        if file.is_null() || ffi::PyUnicode_Check(file) == 0 {
            ffi::Py_XDECREF(file);
            ffi::PyUnicode_FromString(b"<eval>\0".as_ptr() as *const c_char)
        } else {
            file
        }
    };
    if filename_obj.is_null() {
        return Err(());
    }
    let co_name_obj = ffi::PyUnicode_FromString(
        CString::new(data.def.display_name.as_str())
            .unwrap()
            .as_ptr(),
    );
    let qualname_obj =
        ffi::PyUnicode_FromString(CString::new(data.def.qualname.as_str()).unwrap().as_ptr());
    let firstlineno_obj = ffi::PyLong_FromLong(1);
    if co_name_obj.is_null() || qualname_obj.is_null() || firstlineno_obj.is_null() {
        ffi::Py_XDECREF(co_name_obj);
        ffi::Py_XDECREF(qualname_obj);
        ffi::Py_XDECREF(firstlineno_obj);
        ffi::Py_DECREF(filename_obj);
        return Err(());
    }

    let argcount_obj = ffi::PyLong_FromLong(positional.len() as c_long);
    let posonly_obj = ffi::PyLong_FromLong(0);
    let kwonly_obj = ffi::PyLong_FromLong(kwonly.len() as c_long);
    let nlocals_obj = ffi::PyLong_FromLong(varnames.len() as c_long);
    const CO_VARARGS: c_long = 0x04;
    const CO_VARKEYWORDS: c_long = 0x08;
    const CO_GENERATOR: c_long = 0x20;
    const CO_COROUTINE: c_long = 0x80;
    const CO_ASYNC_GENERATOR: c_long = 0x200;
    let mut flags = 0 as c_long;
    if vararg.is_some() {
        flags |= CO_VARARGS;
    }
    if kwarg.is_some() {
        flags |= CO_VARKEYWORDS;
    }
    if data.def.is_async {
        if data.has_yield {
            flags |= CO_ASYNC_GENERATOR;
        } else {
            flags |= CO_COROUTINE;
        }
    } else if data.has_yield {
        flags |= CO_GENERATOR;
    }
    let flags_obj = ffi::PyLong_FromLong(flags);
    if argcount_obj.is_null()
        || posonly_obj.is_null()
        || kwonly_obj.is_null()
        || nlocals_obj.is_null()
        || flags_obj.is_null()
    {
        ffi::Py_XDECREF(argcount_obj);
        ffi::Py_XDECREF(posonly_obj);
        ffi::Py_XDECREF(kwonly_obj);
        ffi::Py_XDECREF(nlocals_obj);
        ffi::Py_XDECREF(flags_obj);
        ffi::Py_DECREF(co_name_obj);
        ffi::Py_DECREF(qualname_obj);
        ffi::Py_DECREF(firstlineno_obj);
        ffi::Py_DECREF(filename_obj);
        return Err(());
    }

    let varnames_tuple = ffi::PyTuple_New(varnames.len() as ffi::Py_ssize_t);
    if varnames_tuple.is_null() {
        ffi::Py_DECREF(argcount_obj);
        ffi::Py_DECREF(posonly_obj);
        ffi::Py_DECREF(kwonly_obj);
        ffi::Py_DECREF(nlocals_obj);
        ffi::Py_DECREF(flags_obj);
        ffi::Py_DECREF(co_name_obj);
        ffi::Py_DECREF(qualname_obj);
        ffi::Py_DECREF(firstlineno_obj);
        ffi::Py_DECREF(filename_obj);
        return Err(());
    }
    for (idx, name) in varnames.iter().enumerate() {
        let name_obj = ffi::PyUnicode_FromString(CString::new(name.as_str()).unwrap().as_ptr());
        if name_obj.is_null()
            || ffi::PyTuple_SetItem(varnames_tuple, idx as ffi::Py_ssize_t, name_obj) != 0
        {
            ffi::Py_XDECREF(name_obj);
            ffi::Py_DECREF(varnames_tuple);
            ffi::Py_DECREF(argcount_obj);
            ffi::Py_DECREF(posonly_obj);
            ffi::Py_DECREF(kwonly_obj);
            ffi::Py_DECREF(nlocals_obj);
            ffi::Py_DECREF(flags_obj);
            ffi::Py_DECREF(co_name_obj);
            ffi::Py_DECREF(qualname_obj);
            ffi::Py_DECREF(firstlineno_obj);
            ffi::Py_DECREF(filename_obj);
            return Err(());
        }
    }

    let freevars_len = data
        .closure
        .as_ref()
        .map(|closure| closure._layout.names.len())
        .unwrap_or(0);
    let freevars_tuple = ffi::PyTuple_New(freevars_len as ffi::Py_ssize_t);
    let cellvars_tuple = ffi::PyTuple_New(0);
    if freevars_tuple.is_null() || cellvars_tuple.is_null() {
        ffi::Py_XDECREF(freevars_tuple);
        ffi::Py_XDECREF(cellvars_tuple);
        ffi::Py_DECREF(varnames_tuple);
        ffi::Py_DECREF(argcount_obj);
        ffi::Py_DECREF(posonly_obj);
        ffi::Py_DECREF(kwonly_obj);
        ffi::Py_DECREF(nlocals_obj);
        ffi::Py_DECREF(flags_obj);
        ffi::Py_DECREF(co_name_obj);
        ffi::Py_DECREF(qualname_obj);
        ffi::Py_DECREF(firstlineno_obj);
        ffi::Py_DECREF(filename_obj);
        return Err(());
    }
    if let Some(closure) = data.closure.as_ref() {
        for (idx, name) in closure._layout.names.iter().enumerate() {
            let public_name = normalize_freevar_name(name.as_str());
            let name_obj = ffi::PyUnicode_FromString(CString::new(public_name).unwrap().as_ptr());
            if name_obj.is_null()
                || ffi::PyTuple_SetItem(freevars_tuple, idx as ffi::Py_ssize_t, name_obj) != 0
            {
                ffi::Py_XDECREF(name_obj);
                ffi::Py_DECREF(freevars_tuple);
                ffi::Py_DECREF(cellvars_tuple);
                ffi::Py_DECREF(varnames_tuple);
                ffi::Py_DECREF(argcount_obj);
                ffi::Py_DECREF(posonly_obj);
                ffi::Py_DECREF(kwonly_obj);
                ffi::Py_DECREF(nlocals_obj);
                ffi::Py_DECREF(flags_obj);
                ffi::Py_DECREF(co_name_obj);
                ffi::Py_DECREF(qualname_obj);
                ffi::Py_DECREF(firstlineno_obj);
                ffi::Py_DECREF(filename_obj);
                return Err(());
            }
        }
    }
    let code = ffi::PyCode_NewEmpty(
        b"<eval>\0".as_ptr() as *const c_char,
        CString::new(data.def.display_name.as_str())
            .unwrap()
            .as_ptr(),
        1,
    ) as *mut ffi::PyObject;
    if code.is_null() {
        ffi::Py_DECREF(freevars_tuple);
        ffi::Py_DECREF(cellvars_tuple);
        ffi::Py_DECREF(varnames_tuple);
        ffi::Py_DECREF(argcount_obj);
        ffi::Py_DECREF(posonly_obj);
        ffi::Py_DECREF(kwonly_obj);
        ffi::Py_DECREF(nlocals_obj);
        ffi::Py_DECREF(flags_obj);
        ffi::Py_DECREF(co_name_obj);
        ffi::Py_DECREF(qualname_obj);
        ffi::Py_DECREF(firstlineno_obj);
        ffi::Py_DECREF(filename_obj);
        return Err(());
    }

    let kwargs = ffi::PyDict_New();
    if kwargs.is_null() {
        ffi::Py_DECREF(code);
        ffi::Py_DECREF(freevars_tuple);
        ffi::Py_DECREF(cellvars_tuple);
        ffi::Py_DECREF(varnames_tuple);
        ffi::Py_DECREF(argcount_obj);
        ffi::Py_DECREF(posonly_obj);
        ffi::Py_DECREF(kwonly_obj);
        ffi::Py_DECREF(nlocals_obj);
        ffi::Py_DECREF(flags_obj);
        ffi::Py_DECREF(co_name_obj);
        ffi::Py_DECREF(qualname_obj);
        ffi::Py_DECREF(firstlineno_obj);
        ffi::Py_DECREF(filename_obj);
        return Err(());
    }
    let set_item = |key: &[u8], obj: *mut ffi::PyObject| {
        ffi::PyDict_SetItemString(kwargs, key.as_ptr() as *const c_char, obj)
    };
    if set_item(b"co_argcount\0", argcount_obj) != 0
        || set_item(b"co_posonlyargcount\0", posonly_obj) != 0
        || set_item(b"co_kwonlyargcount\0", kwonly_obj) != 0
        || set_item(b"co_nlocals\0", nlocals_obj) != 0
        || set_item(b"co_flags\0", flags_obj) != 0
        || set_item(b"co_varnames\0", varnames_tuple) != 0
        || set_item(b"co_freevars\0", freevars_tuple) != 0
        || set_item(b"co_cellvars\0", cellvars_tuple) != 0
        || set_item(b"co_filename\0", filename_obj) != 0
        || set_item(b"co_name\0", co_name_obj) != 0
        || set_item(b"co_qualname\0", qualname_obj) != 0
        || set_item(b"co_firstlineno\0", firstlineno_obj) != 0
    {
        ffi::Py_DECREF(kwargs);
        ffi::Py_DECREF(code);
        ffi::Py_DECREF(freevars_tuple);
        ffi::Py_DECREF(cellvars_tuple);
        ffi::Py_DECREF(varnames_tuple);
        ffi::Py_DECREF(argcount_obj);
        ffi::Py_DECREF(posonly_obj);
        ffi::Py_DECREF(kwonly_obj);
        ffi::Py_DECREF(nlocals_obj);
        ffi::Py_DECREF(flags_obj);
        ffi::Py_DECREF(co_name_obj);
        ffi::Py_DECREF(qualname_obj);
        ffi::Py_DECREF(firstlineno_obj);
        ffi::Py_DECREF(filename_obj);
        return Err(());
    }

    let replace = ffi::PyObject_GetAttrString(code, b"replace\0".as_ptr() as *const c_char);
    let args = ffi::PyTuple_New(0);
    if replace.is_null() || args.is_null() {
        ffi::Py_XDECREF(replace);
        ffi::Py_XDECREF(args);
        ffi::Py_DECREF(kwargs);
        ffi::Py_DECREF(code);
        ffi::Py_DECREF(freevars_tuple);
        ffi::Py_DECREF(cellvars_tuple);
        ffi::Py_DECREF(varnames_tuple);
        ffi::Py_DECREF(argcount_obj);
        ffi::Py_DECREF(posonly_obj);
        ffi::Py_DECREF(kwonly_obj);
        ffi::Py_DECREF(nlocals_obj);
        ffi::Py_DECREF(flags_obj);
        ffi::Py_DECREF(co_name_obj);
        ffi::Py_DECREF(qualname_obj);
        ffi::Py_DECREF(firstlineno_obj);
        ffi::Py_DECREF(filename_obj);
        return Err(());
    }

    let replaced = ffi::PyObject_Call(replace, args, kwargs);
    ffi::Py_DECREF(replace);
    ffi::Py_DECREF(args);
    ffi::Py_DECREF(kwargs);
    ffi::Py_DECREF(code);
    ffi::Py_DECREF(freevars_tuple);
    ffi::Py_DECREF(cellvars_tuple);
    ffi::Py_DECREF(varnames_tuple);
    ffi::Py_DECREF(argcount_obj);
    ffi::Py_DECREF(posonly_obj);
    ffi::Py_DECREF(kwonly_obj);
    ffi::Py_DECREF(nlocals_obj);
    ffi::Py_DECREF(flags_obj);
    ffi::Py_DECREF(co_name_obj);
    ffi::Py_DECREF(qualname_obj);
    ffi::Py_DECREF(firstlineno_obj);
    ffi::Py_DECREF(filename_obj);
    if replaced.is_null() {
        return Err(());
    }
    Ok(replaced)
}

static mut SOAC_FUNCTION_ANNOTATE_PYFUNC_DEF: ffi::PyMethodDef = ffi::PyMethodDef {
    ml_name: b"__annotate__\0".as_ptr() as *const c_char,
    ml_meth: ffi::PyMethodDefPointer {
        PyCFunction: soac_function_annotate_pyfunc,
    },
    ml_flags: ffi::METH_O,
    ml_doc: ptr::null(),
};

unsafe extern "C" fn soac_function_annotate_pyfunc(
    slf: *mut ffi::PyObject,
    arg: *mut ffi::PyObject,
) -> *mut ffi::PyObject {
    if arg.is_null() {
        ffi::PyErr_SetString(
            ffi::PyExc_TypeError,
            b"__annotate__ expects a format\0".as_ptr() as *const c_char,
        );
        return ptr::null_mut();
    }
    let format = ffi::PyLong_AsLong(arg);
    if format == -1 && !ffi::PyErr_Occurred().is_null() {
        return ptr::null_mut();
    }
    if format > 2 {
        ffi::PyErr_SetString(
            ffi::PyExc_NotImplementedError,
            b"format not supported\0".as_ptr() as *const c_char,
        );
        return ptr::null_mut();
    }
    let code = ffi::PyObject_GetAttrString(slf, b"__code__\0".as_ptr() as *const c_char);
    if code.is_null() {
        return ptr::null_mut();
    }
    let data = get_frame_data_for_code(code);
    ffi::Py_DECREF(code);
    let Some(data) = data else {
        ffi::PyErr_SetString(
            ffi::PyExc_RuntimeError,
            b"soac function missing data\0".as_ptr() as *const c_char,
        );
        return ptr::null_mut();
    };
    match eval_function_annotations(&*data, format as i32) {
        Ok(dict) => dict,
        Err(()) => ptr::null_mut(),
    }
}

pub unsafe fn build_function(
    def: min_ast::FunctionDef,
    ctx: &ExecContext<'_>,
    module_name: *mut ffi::PyObject,
) -> Result<*mut ffi::PyObject, ()> {
    let has_yield = def.body.iter().any(stmt_has_yield);
    let local_names = collect_local_names(&def);
    let closure = capture_closure(&def.freevars, ctx)?;
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

    let type_params = build_type_params(&def, ctx)?;

    let mut params = Vec::new();
    for param in &def.params {
        match param {
            min_ast::Parameter::Positional {
                name,
                default,
                annotation,
            } => {
                let _ = annotation;
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
                let _ = annotation;
                params.push(ParamSpec {
                    name: name.clone(),
                    kind: ParamKind::VarArg,
                    default: None,
                });
            }
            min_ast::Parameter::KwOnly {
                name,
                default,
                annotation,
            } => {
                let _ = annotation;
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
                let _ = annotation;
                params.push(ParamSpec {
                    name: name.clone(),
                    kind: ParamKind::KwArg,
                    default: None,
                });
            }
        }
    }

    let _ = &def.returns;

    let display_name = def.display_name.clone();
    let qualname = def.qualname.clone();
    let doc = if let Some(min_ast::StmtNode::Expr { value, .. }) = def.body.first() {
        if let min_ast::ExprNode::String { value, .. } = value {
            let doc = ffi::PyUnicode_FromStringAndSize(
                value.as_ptr() as *const c_char,
                value.len() as ffi::Py_ssize_t,
            );
            if doc.is_null() {
                return Err(());
            }
            doc
        } else {
            ffi::Py_INCREF(ffi::Py_None());
            ffi::Py_None()
        }
    } else {
        ffi::Py_INCREF(ffi::Py_None());
        ffi::Py_None()
    };

    let name_obj = ffi::PyUnicode_FromString(CString::new(display_name.as_str()).unwrap().as_ptr());
    if name_obj.is_null() {
        ffi::Py_DECREF(doc);
        return Err(());
    }
    let qualname_obj = ffi::PyUnicode_FromString(CString::new(qualname.as_str()).unwrap().as_ptr());
    if qualname_obj.is_null() {
        ffi::Py_DECREF(name_obj);
        ffi::Py_DECREF(doc);
        return Err(());
    }

    let module_obj = if module_name.is_null() {
        ffi::Py_INCREF(ffi::Py_None());
        ffi::Py_None()
    } else {
        ffi::Py_INCREF(module_name);
        module_name
    };

    if install_eval_frame_hook().is_err() {
        ffi::Py_DECREF(name_obj);
        ffi::Py_DECREF(qualname_obj);
        ffi::Py_DECREF(doc);
        ffi::Py_DECREF(module_obj);
        return Err(());
    }

    let globals_scope = ctx.globals_scope;
    let globals_dict = ctx.globals_dict;
    let builtins = ctx.builtins;
    let function_codes = ctx.function_codes;
    ffi::Py_INCREF(globals_dict);
    ffi::Py_INCREF(builtins);
    if !function_codes.is_null() {
        ffi::Py_INCREF(function_codes);
    }

    let data = Box::new(FunctionData {
        def,
        params,
        param_layout,
        local_layout,
        closure,
        type_params,
        has_yield,
        globals_scope,
        globals_dict,
        builtins,
        function_codes,
        runtime_fns: ctx.runtime_fns.clone(),
    });

    let code = match function_code_object(&data) {
        Ok(code) => code,
        Err(()) => {
            ffi::Py_DECREF(name_obj);
            ffi::Py_DECREF(qualname_obj);
            ffi::Py_DECREF(doc);
            ffi::Py_DECREF(module_obj);
            return Err(());
        }
    };

    let data_ptr = Box::into_raw(data);
    let code_extra = Box::new(SoacCodeExtra {
        magic: SOAC_CODE_EXTRA_MAGIC,
        data: data_ptr,
    });
    let code_extra_ptr = Box::into_raw(code_extra);
    let extra_index = match soac_code_extra_index() {
        Ok(index) => index,
        Err(()) => {
            drop(Box::from_raw(code_extra_ptr));
            ffi::Py_DECREF(code);
            ffi::Py_DECREF(name_obj);
            ffi::Py_DECREF(qualname_obj);
            ffi::Py_DECREF(doc);
            ffi::Py_DECREF(module_obj);
            return Err(());
        }
    };
    if PyUnstable_Code_SetExtra(code, extra_index, code_extra_ptr as *mut c_void) != 0 {
        drop(Box::from_raw(code_extra_ptr));
        ffi::Py_DECREF(code);
        ffi::Py_DECREF(name_obj);
        ffi::Py_DECREF(qualname_obj);
        ffi::Py_DECREF(doc);
        ffi::Py_DECREF(module_obj);
        return Err(());
    }

    let func = ffi::PyFunction_NewWithQualName(code, globals_dict, qualname_obj);
    if func.is_null() {
        ffi::Py_DECREF(code);
        ffi::Py_DECREF(name_obj);
        ffi::Py_DECREF(qualname_obj);
        ffi::Py_DECREF(doc);
        ffi::Py_DECREF(module_obj);
        return Err(());
    }
    if ffi::PyFunction_Check(func) == 0 {
        ffi::PyErr_SetString(
            ffi::PyExc_RuntimeError,
            b"PyFunction_NewWithQualName did not return function\0".as_ptr() as *const c_char,
        );
        ffi::Py_DECREF(code);
        ffi::Py_DECREF(func);
        ffi::Py_DECREF(name_obj);
        ffi::Py_DECREF(qualname_obj);
        ffi::Py_DECREF(doc);
        ffi::Py_DECREF(module_obj);
        return Err(());
    }

    let defaults = function_defaults_object(&(*data_ptr).params)?;
    if ffi::PyFunction_SetDefaults(func, defaults) != 0 {
        ffi::Py_DECREF(defaults);
        ffi::Py_DECREF(code);
        ffi::Py_DECREF(func);
        ffi::Py_DECREF(name_obj);
        ffi::Py_DECREF(qualname_obj);
        ffi::Py_DECREF(doc);
        ffi::Py_DECREF(module_obj);
        return Err(());
    }
    ffi::Py_DECREF(defaults);

    let kwdefaults = function_kwdefaults_object(&(*data_ptr).params)?;
    if ffi::PyFunction_SetKwDefaults(func, kwdefaults) != 0 {
        ffi::Py_DECREF(kwdefaults);
        ffi::Py_DECREF(code);
        ffi::Py_DECREF(func);
        ffi::Py_DECREF(name_obj);
        ffi::Py_DECREF(qualname_obj);
        ffi::Py_DECREF(doc);
        ffi::Py_DECREF(module_obj);
        return Err(());
    }
    ffi::Py_DECREF(kwdefaults);

    let closure = match function_closure_object(&*data_ptr, code) {
        Ok(closure) => closure,
        Err(()) => {
            ffi::Py_DECREF(code);
            ffi::Py_DECREF(func);
            ffi::Py_DECREF(name_obj);
            ffi::Py_DECREF(qualname_obj);
            ffi::Py_DECREF(doc);
            ffi::Py_DECREF(module_obj);
            return Err(());
        }
    };
    if ffi::PyFunction_SetClosure(func, closure) != 0 {
        ffi::Py_DECREF(closure);
        ffi::Py_DECREF(code);
        ffi::Py_DECREF(func);
        ffi::Py_DECREF(name_obj);
        ffi::Py_DECREF(qualname_obj);
        ffi::Py_DECREF(doc);
        ffi::Py_DECREF(module_obj);
        return Err(());
    }
    ffi::Py_DECREF(closure);
    ffi::Py_DECREF(code);

    if ffi::PyObject_SetAttrString(func, b"__module__\0".as_ptr() as *const c_char, module_obj) != 0
    {
        ffi::PyErr_Clear();
    }
    if ffi::PyObject_SetAttrString(func, b"__doc__\0".as_ptr() as *const c_char, doc) != 0 {
        ffi::PyErr_Clear();
    }

    if function_has_annotations(&(*data_ptr).def) {
        let annotate = ffi::PyCFunction_NewEx(
            std::ptr::addr_of_mut!(SOAC_FUNCTION_ANNOTATE_PYFUNC_DEF),
            func,
            ptr::null_mut(),
        );
        if !annotate.is_null() {
            if ffi::PyObject_SetAttrString(
                func,
                b"__annotate__\0".as_ptr() as *const c_char,
                annotate,
            ) != 0
            {
                ffi::PyErr_Clear();
            }
            ffi::Py_DECREF(annotate);
        } else {
            ffi::PyErr_Clear();
        }
    }

    ffi::Py_DECREF(name_obj);
    ffi::Py_DECREF(qualname_obj);
    ffi::Py_DECREF(doc);
    ffi::Py_DECREF(module_obj);
    Ok(func)
}

pub(crate) fn function_has_annotations(def: &min_ast::FunctionDef) -> bool {
    if def.returns.is_some() {
        return true;
    }
    def.params.iter().any(|param| match param {
        min_ast::Parameter::Positional { annotation, .. }
        | min_ast::Parameter::VarArg { annotation, .. }
        | min_ast::Parameter::KwOnly { annotation, .. }
        | min_ast::Parameter::KwArg { annotation, .. } => annotation.is_some(),
    })
}

pub(crate) unsafe fn eval_function_annotations(
    data: &FunctionData,
    _format: i32,
) -> Result<*mut ffi::PyObject, ()> {
    let annotations = ffi::PyDict_New();
    if annotations.is_null() {
        return Err(());
    }

    let ctx = ExecContext {
        globals_scope: data.globals_scope,
        globals_dict: data.globals_dict,
        params: ptr::null_mut(),
        locals: data.globals_scope,
        builtins: data.builtins,
        function_codes: data.function_codes,
        closure: data.closure.as_ref().map(|closure| &closure.scope),
        runtime_fns: &data.runtime_fns,
        type_params: data.type_params.as_ref().map(|state| &state.map),
    };

    for param in &data.def.params {
        let (name, annotation) = match param {
            min_ast::Parameter::Positional {
                name, annotation, ..
            }
            | min_ast::Parameter::VarArg { name, annotation }
            | min_ast::Parameter::KwOnly {
                name, annotation, ..
            }
            | min_ast::Parameter::KwArg { name, annotation } => (name, annotation),
        };
        if let Some(annotation) = annotation {
            let value = eval_expr(annotation, &ctx)?;
            if ffi::PyDict_SetItemString(
                annotations,
                CString::new(name.as_str()).unwrap().as_ptr(),
                value,
            ) != 0
            {
                ffi::Py_DECREF(value);
                ffi::Py_DECREF(annotations);
                return Err(());
            }
            ffi::Py_DECREF(value);
        }
    }

    if let Some(returns) = &data.def.returns {
        let value = eval_expr(returns, &ctx)?;
        if ffi::PyDict_SetItemString(annotations, CString::new("return").unwrap().as_ptr(), value)
            != 0
        {
            ffi::Py_DECREF(value);
            ffi::Py_DECREF(annotations);
            return Err(());
        }
        ffi::Py_DECREF(value);
    }

    Ok(annotations)
}

pub(crate) fn bind_args(
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
                let name = std::str::from_utf8_unchecked(std::slice::from_raw_parts(
                    ptr as *const u8,
                    len as usize,
                ));
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
                        if scope_assign_name(&mut *param_scope, param.name.as_str(), value).is_err()
                        {
                            ffi::Py_DECREF(args_tuple);
                            return Err(());
                        }
                        arg_index += 1;
                    } else if let Some(value) = kw_map.remove(&param.name) {
                        if scope_assign_name(&mut *param_scope, param.name.as_str(), value).is_err()
                        {
                            ffi::Py_DECREF(args_tuple);
                            return Err(());
                        }
                    } else if param.default.is_some() {
                        // Default applied later by apply_param_defaults.
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
                        if scope_assign_name(&mut *param_scope, param.name.as_str(), value).is_err()
                        {
                            ffi::Py_DECREF(args_tuple);
                            return Err(());
                        }
                    } else if param.default.is_some() {
                        // Default applied later by apply_param_defaults.
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

pub(crate) fn apply_param_defaults(
    params: &[ParamSpec],
    param_scope: *mut ScopeInstance,
) -> Result<(), ()> {
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


fn eval_block(stmts: &[min_ast::StmtNode], ctx: &ExecContext<'_>) -> Result<StmtFlow, ()> {
    for stmt in stmts {
        match eval_stmt(stmt, ctx)? {
            StmtFlow::Normal => {}
            flow => return Ok(flow),
        }
    }
    Ok(StmtFlow::Normal)
}

unsafe fn scope_is_module_globals(ctx: &ExecContext<'_>) -> bool {
    ctx.locals == ctx.globals_scope
}

unsafe fn name_key(name: &str) -> Result<*mut ffi::PyObject, ()> {
    let key = ffi::PyUnicode_FromStringAndSize(name.as_ptr() as *const c_char, name.len() as _);
    if key.is_null() {
        Err(())
    } else {
        Ok(key)
    }
}

unsafe fn dict_lookup_name(dict: *mut ffi::PyObject, name: &str) -> Result<*mut ffi::PyObject, ()> {
    let key = name_key(name)?;
    let value = ffi::PyObject_GetItem(dict, key);
    ffi::Py_DECREF(key);
    if value.is_null() {
        if ffi::PyErr_ExceptionMatches(ffi::PyExc_KeyError) != 0 {
            ffi::PyErr_Clear();
            return Ok(ptr::null_mut());
        }
        return Err(());
    }
    Ok(value)
}

unsafe fn assign_name_in_context(
    ctx: &ExecContext<'_>,
    name: &str,
    value: *mut ffi::PyObject,
) -> Result<(), ()> {
    if scope_is_module_globals(ctx) {
        let key = name_key(name)?;
        let set_result = ffi::PyObject_SetItem(ctx.globals_dict, key, value);
        ffi::Py_DECREF(key);
        if set_result != 0 {
            return Err(());
        }
        return Ok(());
    }
    scope_assign_name(&mut *ctx.locals, name, value)
}

unsafe fn delete_name_in_context(ctx: &ExecContext<'_>, name: &str) -> Result<(), ()> {
    if scope_is_module_globals(ctx) {
        let key = name_key(name)?;
        let del_result = ffi::PyObject_DelItem(ctx.globals_dict, key);
        ffi::Py_DECREF(key);
        if del_result != 0 {
            if ffi::PyErr_ExceptionMatches(ffi::PyExc_KeyError) != 0 {
                ffi::PyErr_Clear();
                return set_name_error(name);
            }
            return Err(());
        }
        return Ok(());
    }
    scope_delete_name(&mut *ctx.locals, name)
}

fn eval_stmt(stmt: &min_ast::StmtNode, ctx: &ExecContext<'_>) -> Result<StmtFlow, ()> {
    match stmt {
        min_ast::StmtNode::FunctionDef(func) => unsafe {
            let module_name = dict_lookup_name(ctx.globals_dict, "__name__")?;
            let function = build_function(func.clone(), ctx, module_name)?;
            if !module_name.is_null() {
                ffi::Py_DECREF(module_name);
            }
            if assign_name_in_context(ctx, func.name.as_str(), function).is_err() {
                ffi::Py_DECREF(function);
                return Err(());
            }
            ffi::Py_DECREF(function);
            Ok(StmtFlow::Normal)
        },
        min_ast::StmtNode::While {
            test, body, orelse, ..
        } => {
            loop {
                let condition = eval_expr(test, ctx)?;
                let truthy = unsafe { ffi::PyObject_IsTrue(condition) };
                unsafe {
                    ffi::Py_DECREF(condition);
                }
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
        min_ast::StmtNode::If {
            test, body, orelse, ..
        } => {
            let condition = eval_expr(test, ctx)?;
            let truthy = unsafe { ffi::PyObject_IsTrue(condition) };
            unsafe {
                ffi::Py_DECREF(condition);
            }
            if truthy < 0 {
                return Err(());
            }
            if truthy == 0 {
                eval_block(orelse, ctx)
            } else {
                eval_block(body, ctx)
            }
        }
        min_ast::StmtNode::Try {
            body,
            handler,
            orelse,
            finalbody,
            ..
        } => {
            let mut had_exception = false;
            let mut flow = match eval_block(body, ctx) {
                Ok(flow) => flow,
                Err(()) => {
                    had_exception = true;
                    if let Some(handler) = handler {
                        unsafe {
                            let mut prev_type: *mut ffi::PyObject = ptr::null_mut();
                            let mut prev_value: *mut ffi::PyObject = ptr::null_mut();
                            let mut prev_tb: *mut ffi::PyObject = ptr::null_mut();
                            ffi::PyErr_GetExcInfo(&mut prev_type, &mut prev_value, &mut prev_tb);

                            let raised = ffi::PyErr_GetRaisedException();
                            let mut raised_type: *mut ffi::PyObject = ptr::null_mut();
                            let mut raised_tb: *mut ffi::PyObject = ptr::null_mut();
                            if !raised.is_null() {
                                raised_type = ffi::Py_TYPE(raised) as *mut ffi::PyObject;
                                ffi::Py_INCREF(raised_type);
                                raised_tb = ffi::PyException_GetTraceback(raised);
                            }
                            ffi::PyErr_SetExcInfo(raised_type, raised, raised_tb);
                            // Clear the error indicator before running the handler.
                            ffi::PyErr_Clear();

                            let handler_result = eval_block(handler, ctx);
                            ffi::PyErr_SetExcInfo(prev_type, prev_value, prev_tb);
                            match handler_result {
                                Ok(flow) => flow,
                                Err(()) => return Err(()),
                            }
                        }
                    } else {
                        return Err(());
                    }
                }
            };

            if !had_exception && matches!(flow, StmtFlow::Normal) {
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
                    let mut typ: *mut ffi::PyObject = ptr::null_mut();
                    let mut val: *mut ffi::PyObject = ptr::null_mut();
                    let mut tb: *mut ffi::PyObject = ptr::null_mut();
                    ffi::PyErr_GetExcInfo(&mut typ, &mut val, &mut tb);
                    if !val.is_null() {
                        if typ.is_null() {
                            typ = ffi::Py_TYPE(val) as *mut ffi::PyObject;
                            ffi::Py_INCREF(typ);
                        }
                        if !tb.is_null() {
                            ffi::PyException_SetTraceback(val, tb);
                        }
                        ffi::PyErr_SetObject(typ, val);
                        ffi::Py_XDECREF(typ);
                        ffi::Py_XDECREF(val);
                        ffi::Py_XDECREF(tb);
                    } else {
                        ffi::PyErr_SetString(
                            ffi::PyExc_RuntimeError,
                            b"No active exception to reraise\0".as_ptr() as *const c_char,
                        );
                    }
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
                unsafe {
                    ffi::Py_INCREF(ffi::Py_None());
                }
                unsafe { ffi::Py_None() }
            };
            Ok(StmtFlow::Return(result))
        }
        min_ast::StmtNode::Expr { value, .. } => {
            let result = eval_expr(value, ctx)?;
            unsafe {
                ffi::Py_DECREF(result);
            }
            Ok(StmtFlow::Normal)
        }
        min_ast::StmtNode::Assign { target, value, .. } => {
            let result = eval_expr(value, ctx)?;
            let status = unsafe { assign_name_in_context(ctx, target.as_str(), result) };
            unsafe {
                ffi::Py_DECREF(result);
            }
            if status.is_err() {
                return Err(());
            }
            Ok(StmtFlow::Normal)
        }
        min_ast::StmtNode::Delete { target, .. } => {
            if unsafe { delete_name_in_context(ctx, target.as_str()) }.is_err() {
                return Err(());
            }
            Ok(StmtFlow::Normal)
        }
        min_ast::StmtNode::Pass(_) => Ok(StmtFlow::Normal),
    }
}

pub(crate) fn eval_expr(
    expr: &min_ast::ExprNode,
    ctx: &ExecContext<'_>,
) -> Result<*mut ffi::PyObject, ()> {
    match expr {
        min_ast::ExprNode::Name { id, .. } => lookup_name(id.as_str(), ctx),
        min_ast::ExprNode::Attribute { value, attr, .. } => {
            let base = eval_expr(value, ctx)?;
            let name = CString::new(attr.as_str()).unwrap();
            let result = unsafe { ffi::PyObject_GetAttrString(base, name.as_ptr()) };
            unsafe {
                ffi::Py_DECREF(base);
            }
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
                if result.is_null() {
                    Err(())
                } else {
                    Ok(result)
                }
            },
            min_ast::Number::Float(text) => unsafe {
                let py_str =
                    ffi::PyUnicode_FromString(CString::new(text.as_str()).unwrap().as_ptr());
                if py_str.is_null() {
                    return Err(());
                }
                let result = ffi::PyFloat_FromString(py_str);
                ffi::Py_DECREF(py_str);
                if result.is_null() {
                    Err(())
                } else {
                    Ok(result)
                }
            },
        },
        min_ast::ExprNode::String { value, .. } => unsafe {
            let bytes = value.as_bytes();
            let result =
                ffi::PyUnicode_FromStringAndSize(bytes.as_ptr() as *const c_char, bytes.len() as _);
            if result.is_null() {
                Err(())
            } else {
                Ok(result)
            }
        },
        min_ast::ExprNode::Bytes { value, .. } => unsafe {
            let result =
                ffi::PyBytes_FromStringAndSize(value.as_ptr() as *const c_char, value.len() as _);
            if result.is_null() {
                Err(())
            } else {
                Ok(result)
            }
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
        min_ast::ExprNode::Raw { expr, .. } => unsafe {
            let source = dp_transform::ruff_ast_to_string(expr);
            eval_raw_expr_source(source.trim(), ctx)
        },
        min_ast::ExprNode::Await { .. } => set_not_implemented("await not supported"),
        min_ast::ExprNode::Yield { .. } => set_not_implemented("yield not supported"),
        min_ast::ExprNode::Call { func, args, .. } => eval_call(func, args, ctx),
    }
}

unsafe fn eval_raw_expr_source(
    source: &str,
    ctx: &ExecContext<'_>,
) -> Result<*mut ffi::PyObject, ()> {
    let source_obj =
        ffi::PyUnicode_FromStringAndSize(source.as_ptr() as *const c_char, source.len() as _);
    if source_obj.is_null() {
        return Err(());
    }

    let globals_obj = {
        ffi::Py_INCREF(ctx.globals_dict);
        ctx.globals_dict
    };

    let locals_obj = if ctx.locals == ctx.globals_scope {
        ffi::Py_INCREF(globals_obj);
        globals_obj
    } else {
        match locals_snapshot(ctx) {
            Ok(locals) => locals,
            Err(()) => {
                ffi::Py_DECREF(source_obj);
                ffi::Py_DECREF(globals_obj);
                return Err(());
            }
        }
    };

    let eval_fn = dict_lookup_name(ctx.builtins, "eval")?;
    if eval_fn.is_null() {
        ffi::Py_DECREF(source_obj);
        ffi::Py_DECREF(globals_obj);
        ffi::Py_DECREF(locals_obj);
        ffi::PyErr_SetString(
            ffi::PyExc_RuntimeError,
            b"missing builtins.eval\0".as_ptr() as *const c_char,
        );
        return Err(());
    }
    let result = ffi::PyObject_CallFunctionObjArgs(
        eval_fn,
        source_obj,
        globals_obj,
        locals_obj,
        ptr::null_mut::<ffi::PyObject>(),
    );
    ffi::Py_DECREF(eval_fn);
    ffi::Py_DECREF(source_obj);
    ffi::Py_DECREF(globals_obj);
    ffi::Py_DECREF(locals_obj);
    if result.is_null() { Err(()) } else { Ok(result) }
}

fn lookup_name(name: &str, ctx: &ExecContext<'_>) -> Result<*mut ffi::PyObject, ()> {
    unsafe {
        if let Some(type_params) = ctx.type_params {
            if let Some(value) = type_params.get(name) {
                ffi::Py_INCREF(*value);
                return Ok(*value);
            }
        }
        let locals = &*ctx.locals;
        if ctx.locals != ctx.globals_scope {
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
            let value = dict_lookup_name(ctx.globals_dict, name)?;
            if !value.is_null() {
                return Ok(value);
            }
        }

        if let Some(closure) = ctx.closure {
            let value = scope_lookup_name(closure, name);
            if !value.is_null() {
                if ffi::PyObject_TypeCheck(value, std::ptr::addr_of_mut!(PyCell_Type)) != 0
                    && !closure_name_requires_cell(name)
                {
                    let loaded = ffi::PyObject_GetAttrString(
                        value,
                        b"cell_contents\0".as_ptr() as *const c_char,
                    );
                    ffi::Py_DECREF(value);
                    if loaded.is_null() {
                        if ffi::PyErr_ExceptionMatches(ffi::PyExc_ValueError) != 0 {
                            ffi::PyErr_Clear();
                            return set_name_error(name);
                        }
                        return Err(());
                    }
                    return Ok(loaded);
                }
                return Ok(value);
            }
        }

        if ctx.locals != ctx.globals_scope {
            let value = dict_lookup_name(ctx.globals_dict, name)?;
            if !value.is_null() {
                return Ok(value);
            }
        }

        let value = dict_lookup_name(ctx.builtins, name)?;
        if !value.is_null() {
            return Ok(value);
        }
    }

    set_name_error(name)
}

unsafe fn locals_snapshot(ctx: &ExecContext<'_>) -> Result<*mut ffi::PyObject, ()> {
    unsafe fn merge_scope_dict(
        target: *mut ffi::PyObject,
        source: *mut ffi::PyObject,
        allow_overwrite: bool,
    ) -> Result<(), ()> {
        let mut pos: ffi::Py_ssize_t = 0;
        let mut key: *mut ffi::PyObject = ptr::null_mut();
        let mut value: *mut ffi::PyObject = ptr::null_mut();
        while ffi::PyDict_Next(source, &mut pos, &mut key, &mut value) != 0 {
            let mut alias_key: *mut ffi::PyObject = ptr::null_mut();
            if ffi::PyUnicode_Check(key) != 0 {
                let mut len: ffi::Py_ssize_t = 0;
                let ptr = ffi::PyUnicode_AsUTF8AndSize(key, &mut len);
                if ptr.is_null() {
                    return Err(());
                }
                let name = std::str::from_utf8_unchecked(std::slice::from_raw_parts(
                    ptr as *const u8,
                    len as usize,
                ));
                if let Some(stripped) = name.strip_prefix("_dp_cell_") {
                    alias_key = ffi::PyUnicode_FromStringAndSize(
                        stripped.as_ptr() as *const c_char,
                        stripped.len() as _,
                    );
                    if alias_key.is_null() {
                        return Err(());
                    }
                }
            }
            let mut value_for_dict = value;
            let mut value_needs_decref = false;
            if ffi::PyObject_TypeCheck(value, std::ptr::addr_of_mut!(PyCell_Type)) != 0 {
                let cell_contents = ffi::PyObject_GetAttrString(
                    value,
                    b"cell_contents\0".as_ptr() as *const c_char,
                );
                if !cell_contents.is_null() {
                    value_for_dict = cell_contents;
                    value_needs_decref = true;
                } else {
                    ffi::PyErr_Clear();
                    ffi::Py_XDECREF(alias_key);
                    continue;
                }
            }
            let target_key = if !alias_key.is_null() { alias_key } else { key };
            if !allow_overwrite {
                let contains = ffi::PyDict_Contains(target, target_key);
                if contains < 0 {
                    ffi::Py_XDECREF(alias_key);
                    if value_needs_decref {
                        ffi::Py_DECREF(value_for_dict);
                    }
                    return Err(());
                }
                if contains != 0 {
                    ffi::Py_XDECREF(alias_key);
                    if value_needs_decref {
                        ffi::Py_DECREF(value_for_dict);
                    }
                    continue;
                }
            }
            if ffi::PyDict_SetItem(target, target_key, value_for_dict) != 0 {
                ffi::Py_XDECREF(alias_key);
                if value_needs_decref {
                    ffi::Py_DECREF(value_for_dict);
                }
                return Err(());
            }
            ffi::Py_XDECREF(alias_key);
            if value_needs_decref {
                ffi::Py_DECREF(value_for_dict);
            }
        }
        Ok(())
    }

    let dict = ffi::PyDict_New();
    if dict.is_null() {
        return Err(());
    }
    if !ctx.params.is_null() {
        let params = scope_to_dict(&*ctx.params)?;
        if merge_scope_dict(dict, params, true).is_err() {
            ffi::Py_DECREF(params);
            ffi::Py_DECREF(dict);
            return Err(());
        }
        ffi::Py_DECREF(params);
    }
    let locals = scope_to_dict(&*ctx.locals)?;
    if merge_scope_dict(dict, locals, true).is_err() {
        ffi::Py_DECREF(locals);
        ffi::Py_DECREF(dict);
        return Err(());
    }
    ffi::Py_DECREF(locals);
    if let Some(closure) = ctx.closure {
        let closure_dict = scope_to_dict(closure)?;
        if merge_scope_dict(dict, closure_dict, false).is_err() {
            ffi::Py_DECREF(closure_dict);
            ffi::Py_DECREF(dict);
            return Err(());
        }
        ffi::Py_DECREF(closure_dict);
    }
    Ok(dict)
}

fn eval_call(
    func: &min_ast::ExprNode,
    args: &[min_ast::Arg],
    ctx: &ExecContext<'_>,
) -> Result<*mut ffi::PyObject, ()> {
    if let Some(type_param) = type_param_lookup_target(func, args, ctx) {
        let func_obj = eval_expr(func, ctx)?;
        let call_args = collect_call_args(args, ctx)?;
        unsafe {
            call_args.cleanup();
            ffi::Py_DECREF(func_obj);
            ffi::Py_INCREF(type_param);
        }
        return Ok(type_param);
    }

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
                if ctx.locals == ctx.globals_scope {
                    ffi::Py_INCREF(ctx.globals_dict);
                    ctx.globals_dict
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
                ffi::Py_INCREF(ctx.globals_dict);
                ctx.globals_dict
            };
            ffi::Py_DECREF(func_obj);
            if result.is_null() {
                return Err(());
            }
            return Ok(result);
        }
    }

    let mut positional: Vec<*mut ffi::PyObject> = Vec::new();
    let kwargs = unsafe { ffi::PyDict_New() };
    if kwargs.is_null() {
        unsafe {
            ffi::Py_DECREF(func_obj);
        }
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

    let call_kwargs = unsafe {
        if ffi::PyDict_Size(kwargs) == 0 {
            ffi::Py_DECREF(kwargs);
            ptr::null_mut()
        } else {
            kwargs
        }
    };

    let args_ptr = if positional.is_empty() {
        ptr::null()
    } else {
        positional.as_ptr()
    };

    let result = unsafe {
        ffi::PyObject_VectorcallDict(func_obj, args_ptr, positional.len() as _, call_kwargs)
    };
    unsafe {
        ffi::Py_DECREF(func_obj);
        if !call_kwargs.is_null() {
            ffi::Py_DECREF(call_kwargs);
        }
        for value in positional {
            ffi::Py_DECREF(value);
        }
    }

    if result.is_null() {
        return Err(());
    }
    unsafe {
        if !ffi::PyErr_Occurred().is_null() {
            ffi::Py_DECREF(result);
            return Err(());
        }
    }
    Ok(result)
}

fn type_param_lookup_target(
    func: &min_ast::ExprNode,
    args: &[min_ast::Arg],
    ctx: &ExecContext<'_>,
) -> Option<*mut ffi::PyObject> {
    let type_params = ctx.type_params?;
    let (module, attr) = match func {
        min_ast::ExprNode::Attribute { value, attr, .. } => (value.as_ref(), attr.as_str()),
        _ => return None,
    };
    if !matches!(module, min_ast::ExprNode::Name { id, .. } if id == "__dp__") {
        return None;
    }
    let name_arg_index = match attr {
        "class_lookup_global" | "load_global" => 1,
        _ => return None,
    };
    let name = match args.get(name_arg_index) {
        Some(min_ast::Arg::Positional(min_ast::ExprNode::String { value, .. })) => value,
        _ => return None,
    };
    type_params.get(name).copied()
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

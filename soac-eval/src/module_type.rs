use crate::counter_dump::{CounterDumpRecord, CounterDumpRow};
use crate::module_constants::ModuleCodegenConstants;
use crate::module_globals::ModuleGlobalCache;
use pyo3::exceptions::{PyRuntimeError, PyTypeError};
use pyo3::ffi;
use pyo3::prelude::*;
use pyo3::types::PyAnyMethods;
use soac_blockpy::block_py::{
    BlockPyFunction, BlockPyModule, CounterDef, CounterId, CounterScope, CounterSite, FunctionId,
};
use soac_blockpy::passes::CodegenBlockPyPass;
use std::collections::HashMap;
use std::env;
use std::ffi::{c_int, c_void};
use std::fs::OpenOptions;
use std::io::Write;
use std::mem::MaybeUninit;
use std::path::Path;
use std::ptr;
use std::sync::Arc;

pub struct SoacExtModuleDataRef<'a> {
    pub shared_state: &'a SharedModuleState,
}

pub struct SharedModuleState {
    pub lowered_module: BlockPyModule<CodegenBlockPyPass>,
    pub module_name: String,
    pub package_name: String,
    pub codegen_constants: ModuleCodegenConstants,
    function_index_by_id: HashMap<FunctionId, usize>,
    module_constant_objs: Vec<Py<PyAny>>,
    counter_slots_by_id: Box<[usize]>,
    counter_values: Box<[u64]>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum CounterStorageKey {
    This(CounterId),
    Shared {
        scope: CounterScope,
        site: CounterSite,
        kind: String,
    },
}

impl SharedModuleState {
    pub fn lookup_function(
        &self,
        function_id: FunctionId,
    ) -> Option<&BlockPyFunction<CodegenBlockPyPass>> {
        let function_index = self.function_index_by_id.get(&function_id).copied()?;
        let function = self.lowered_module.callable_defs.get(function_index)?;
        assert_eq!(function.function_id, function_id);
        Some(function)
    }

    pub(crate) fn module_constant_ptrs(&self) -> Vec<*mut ffi::PyObject> {
        self.module_constant_objs
            .iter()
            .map(|obj| obj.as_ptr())
            .collect()
    }

    pub(crate) fn counter_ptrs(&self) -> Vec<*mut u64> {
        self.counter_slots_by_id
            .iter()
            .map(|slot| &self.counter_values[*slot] as *const u64 as *mut u64)
            .collect()
    }

    pub fn counter_values(&self) -> &[u64] {
        &self.counter_values
    }

    pub fn counter_value(&self, counter_id: CounterId) -> u64 {
        let Some(slot) = self.counter_slots_by_id.get(counter_id.0).copied() else {
            return 0;
        };
        self.counter_values.get(slot).copied().unwrap_or_default()
    }

    pub fn counter_dump_record(&self) -> Option<CounterDumpRecord> {
        if self.lowered_module.counter_defs.is_empty() {
            return None;
        }

        let rows = self
            .lowered_module
            .counter_defs
            .iter()
            .map(|counter| {
                let value = self.counter_value(counter.id);
                match &counter.site {
                    CounterSite::BlockEntry {
                        function_id,
                        block_label,
                    } => {
                        let function = self.lookup_function(*function_id);
                        CounterDumpRow {
                            counter_id: u32::try_from(counter.id.0)
                                .expect("counter ids should fit in u32"),
                            scope: counter_scope_name(counter.scope).to_string(),
                            kind: counter.kind.clone(),
                            site_kind: "block_entry".to_string(),
                            function_id: Some(
                                u32::try_from(function_id.0)
                                    .expect("function ids should fit in u32"),
                            ),
                            function_qualname: function
                                .map(|function| function.names.qualname.clone())
                                .or_else(|| Some("<missing-function>".to_string())),
                            block_label: Some(block_label.to_string()),
                            value,
                        }
                    }
                    CounterSite::Runtime { function_id } => CounterDumpRow {
                        counter_id: u32::try_from(counter.id.0)
                            .expect("counter ids should fit in u32"),
                        scope: counter_scope_name(counter.scope).to_string(),
                        kind: counter.kind.clone(),
                        site_kind: "runtime".to_string(),
                        function_id: function_id.map(|function_id| {
                            u32::try_from(function_id.0).expect("function ids should fit in u32")
                        }),
                        function_qualname: function_id.and_then(|function_id| {
                            self.lookup_function(function_id)
                                .map(|function| function.names.qualname.clone())
                        }),
                        block_label: None,
                        value,
                    },
                }
            })
            .collect();

        Some(CounterDumpRecord {
            module_name: self.module_name.clone(),
            package_name: (!self.package_name.is_empty()).then(|| self.package_name.clone()),
            rows,
        })
    }

    pub fn append_counter_dump_file(&self, path: &Path) -> Result<(), String> {
        let Some(record) = self.counter_dump_record() else {
            return Ok(());
        };
        let bytes = record.encode()?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|err| format!("failed to open {}: {err}", path.display()))?;
        file.write_all(bytes.as_slice())
            .map_err(|err| format!("failed to write {}: {err}", path.display()))
    }
}

fn counter_scope_name(scope: CounterScope) -> &'static str {
    match scope {
        CounterScope::This => "this",
        CounterScope::Function => "function",
        CounterScope::Global => "global",
    }
}

fn counter_storage_key(counter: &CounterDef) -> PyResult<CounterStorageKey> {
    match counter.scope {
        CounterScope::This => Ok(CounterStorageKey::This(counter.id)),
        CounterScope::Function | CounterScope::Global => Ok(CounterStorageKey::Shared {
            scope: counter.scope,
            site: counter.site.clone(),
            kind: counter.kind.clone(),
        }),
    }
}

fn build_counter_storage(counter_defs: &[CounterDef]) -> PyResult<(Box<[usize]>, Box<[u64]>)> {
    let mut slots_by_id = vec![usize::MAX; counter_defs.len()];
    let mut slot_by_key = HashMap::new();
    let mut counter_values = Vec::new();
    for counter in counter_defs {
        if counter.id.0 >= slots_by_id.len() {
            return Err(PyRuntimeError::new_err(format!(
                "counter id {} is out of range for {} counter defs",
                counter.id.0,
                counter_defs.len()
            )));
        }
        let key = counter_storage_key(counter)?;
        let slot = if let Some(slot) = slot_by_key.get(&key).copied() {
            slot
        } else {
            let slot = counter_values.len();
            counter_values.push(0);
            slot_by_key.insert(key, slot);
            slot
        };
        slots_by_id[counter.id.0] = slot;
    }
    Ok((
        slots_by_id.into_boxed_slice(),
        counter_values.into_boxed_slice(),
    ))
}

#[cfg(test)]
pub(crate) fn build_shared_state_for_testing(
    py: Python<'_>,
    lowered_module: BlockPyModule<CodegenBlockPyPass>,
    module_name: &str,
    package_name: &str,
) -> PyResult<Arc<SharedModuleState>> {
    let function_index_by_id = build_function_index_by_id(&lowered_module)?;
    let (counter_slots_by_id, counter_values) =
        build_counter_storage(&lowered_module.counter_defs)?;
    let codegen_constants = ModuleCodegenConstants::collect_from_module(&lowered_module);
    let module_constant_objs = codegen_constants.build_python_constants(py)?;
    Ok(Arc::new(SharedModuleState {
        lowered_module,
        module_name: module_name.to_string(),
        package_name: package_name.to_string(),
        codegen_constants,
        function_index_by_id,
        module_constant_objs,
        counter_slots_by_id,
        counter_values,
    }))
}

fn build_function_index_by_id(
    module: &BlockPyModule<CodegenBlockPyPass>,
) -> PyResult<HashMap<FunctionId, usize>> {
    let mut function_index_by_id = HashMap::with_capacity(module.callable_defs.len());
    for (function_index, function) in module.callable_defs.iter().enumerate() {
        if function_index_by_id
            .insert(function.function_id, function_index)
            .is_some()
        {
            return Err(PyRuntimeError::new_err(format!(
                "duplicate function id {} in shared module state ({})",
                function.function_id.0, function.names.qualname
            )));
        }
    }
    Ok(function_index_by_id)
}

#[repr(C)]
struct SoacExtModuleState {
    initialized: bool,
    shared_state: MaybeUninit<Arc<SharedModuleState>>,
    global_cache: MaybeUninit<Arc<ModuleGlobalCache>>,
    global_cache_initialized: bool,
}

impl SoacExtModuleState {
    unsafe fn init(
        &mut self,
        py: Python<'_>,
        lowered_module: BlockPyModule<CodegenBlockPyPass>,
        module_name: String,
        package_name: String,
    ) -> PyResult<()> {
        if self.initialized {
            return Err(PyRuntimeError::new_err(
                "transformed module state was unexpectedly initialized twice",
            ));
        }
        let function_index_by_id = build_function_index_by_id(&lowered_module)?;
        let (counter_slots_by_id, counter_values) =
            build_counter_storage(&lowered_module.counter_defs)?;
        let codegen_constants = ModuleCodegenConstants::collect_from_module(&lowered_module);
        let module_constant_objs = codegen_constants.build_python_constants(py)?;
        self.shared_state.write(Arc::new(SharedModuleState {
            lowered_module,
            module_name,
            package_name,
            codegen_constants,
            function_index_by_id,
            module_constant_objs,
            counter_slots_by_id,
            counter_values,
        }));
        self.initialized = true;
        self.global_cache_initialized = false;
        Ok(())
    }

    unsafe fn clear(&mut self) {
        if !self.initialized {
            return;
        }
        let shared_state = unsafe { self.shared_state.assume_init_ref().as_ref() };
        if let Some(path) = counter_dump_file_from_env() {
            if let Err(err) = shared_state.append_counter_dump_file(path.as_path()) {
                eprintln!(
                    "[soac counters] failed to append counter dump to {}: {err}",
                    path.display()
                );
            }
        }
        if self.global_cache_initialized {
            unsafe { ptr::drop_in_place(self.global_cache.as_mut_ptr()) };
            self.global_cache_initialized = false;
        }
        unsafe { ptr::drop_in_place(self.shared_state.as_mut_ptr()) };
        self.initialized = false;
    }

    unsafe fn data(&self) -> PyResult<SoacExtModuleDataRef<'_>> {
        if !self.initialized {
            return Err(PyRuntimeError::new_err(
                "missing transformed-module lowering data in module state",
            ));
        }
        Ok(SoacExtModuleDataRef {
            shared_state: unsafe { self.shared_state.assume_init_ref().as_ref() },
        })
    }

    unsafe fn clone_shared_state(&self) -> PyResult<Arc<SharedModuleState>> {
        if !self.initialized {
            return Err(PyRuntimeError::new_err(
                "missing transformed-module lowering data in module state",
            ));
        }
        Ok(unsafe { self.shared_state.assume_init_ref().clone() })
    }

    unsafe fn clone_or_init_global_cache(
        &mut self,
        globals_obj: *mut ffi::PyObject,
    ) -> PyResult<Arc<ModuleGlobalCache>> {
        if !self.initialized {
            return Err(PyRuntimeError::new_err(
                "missing transformed-module lowering data in module state",
            ));
        }
        if self.global_cache_initialized {
            return Ok(unsafe { self.global_cache.assume_init_ref().clone() });
        }
        let global_names = unsafe {
            self.shared_state
                .assume_init_ref()
                .lowered_module
                .global_names
                .clone()
        };
        let cache = unsafe { ModuleGlobalCache::new(globals_obj, global_names.as_slice()) }
            .map_err(|_| {
                if unsafe { ffi::PyErr_Occurred() }.is_null() {
                    PyRuntimeError::new_err("failed to create module global cache")
                } else {
                    PyErr::fetch(Python::assume_attached())
                }
            })?;
        self.global_cache.write(cache.clone());
        self.global_cache_initialized = true;
        Ok(cache)
    }
}

fn counter_dump_file_from_env() -> Option<std::path::PathBuf> {
    let raw = env::var("DIET_PYTHON_COUNTERS_FILE").ok()?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.into())
    }
}

unsafe extern "C" fn soac_ext_module_clear(module: *mut ffi::PyObject) -> c_int {
    let state = unsafe { ffi::PyModule_GetState(module) }.cast::<SoacExtModuleState>();
    if state.is_null() {
        return 0;
    }
    unsafe { (*state).clear() };
    0
}

unsafe extern "C" fn soac_ext_module_traverse(
    module: *mut ffi::PyObject,
    visit: ffi::visitproc,
    arg: *mut c_void,
) -> c_int {
    let state = unsafe { ffi::PyModule_GetState(module) }.cast::<SoacExtModuleState>();
    if state.is_null() || unsafe { !(*state).initialized } {
        return 0;
    }
    let shared_state = unsafe { (*state).shared_state.assume_init_ref().as_ref() };
    for obj in &shared_state.module_constant_objs {
        let rc = unsafe { visit(obj.as_ptr(), arg) };
        if rc != 0 {
            return rc;
        }
    }
    0
}

unsafe extern "C" fn soac_ext_module_free(module: *mut c_void) {
    unsafe {
        soac_ext_module_clear(module.cast());
    }
}

static mut SOAC_EXT_MODULE_DEF: ffi::PyModuleDef = ffi::PyModuleDef {
    m_base: ffi::PyModuleDef_HEAD_INIT,
    m_name: c"_soac_ext.module_state".as_ptr(),
    m_doc: ptr::null(),
    m_size: std::mem::size_of::<SoacExtModuleState>() as ffi::Py_ssize_t,
    m_methods: ptr::null_mut(),
    m_slots: ptr::null_mut(),
    m_traverse: Some(soac_ext_module_traverse),
    m_clear: Some(soac_ext_module_clear),
    m_free: Some(soac_ext_module_free),
};

fn soac_ext_module_def() -> *mut ffi::PyModuleDef {
    ptr::addr_of_mut!(SOAC_EXT_MODULE_DEF)
}

fn soac_ext_module_state(module: &Bound<'_, PyAny>) -> PyResult<*mut SoacExtModuleState> {
    unsafe {
        let module_def = ffi::PyModule_GetDef(module.as_ptr());
        if module_def != soac_ext_module_def() {
            return Err(PyTypeError::new_err(
                "expected a module created via _soac_ext.create_module",
            ));
        }
        let state = ffi::PyModule_GetState(module.as_ptr()).cast::<SoacExtModuleState>();
        if state.is_null() {
            if ffi::PyErr_Occurred().is_null() {
                Err(PyRuntimeError::new_err(
                    "missing _soac_ext module state for transformed module",
                ))
            } else {
                Err(PyErr::fetch(module.py()))
            }
        } else {
            Ok(state)
        }
    }
}

pub struct SoacExtModule;

impl SoacExtModule {
    pub fn new(
        py: Python<'_>,
        spec: &Bound<'_, PyAny>,
        lowered_module: BlockPyModule<CodegenBlockPyPass>,
    ) -> PyResult<Py<PyAny>> {
        let module_name = spec
            .getattr("name")?
            .extract::<String>()
            .map_err(|_| PyTypeError::new_err("expected a module spec with a string 'name'"))?;
        let package_name = spec
            .getattr("parent")?
            .extract::<String>()
            .map_err(|_| PyTypeError::new_err("expected a module spec with a string 'parent'"))?;
        let module = unsafe {
            Bound::from_owned_ptr_or_err(
                py,
                ffi::PyModule_FromDefAndSpec(soac_ext_module_def(), spec.as_ptr()),
            )?
        };
        if unsafe { ffi::PyModule_ExecDef(module.as_ptr(), soac_ext_module_def()) } != 0 {
            return Err(PyErr::fetch(py));
        }
        let state = soac_ext_module_state(&module)?;
        unsafe {
            (*state).init(py, lowered_module, module_name, package_name)?;
        }
        Ok(module.unbind())
    }

    pub fn with_data<R>(
        module: &Bound<'_, PyAny>,
        f: impl FnOnce(SoacExtModuleDataRef<'_>) -> PyResult<R>,
    ) -> PyResult<R> {
        let state = soac_ext_module_state(module)?;
        unsafe { f((*state).data()?) }
    }

    pub fn clone_shared_state(module: &Bound<'_, PyAny>) -> PyResult<Arc<SharedModuleState>> {
        let state = soac_ext_module_state(module)?;
        unsafe { (*state).clone_shared_state() }
    }

    pub fn clone_or_init_global_cache(
        module: &Bound<'_, PyAny>,
        globals_obj: *mut ffi::PyObject,
    ) -> PyResult<Arc<ModuleGlobalCache>> {
        let state = soac_ext_module_state(module)?;
        unsafe { (*state).clone_or_init_global_cache(globals_obj) }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::counter_dump::COUNTER_DUMP_MAGIC;
    use soac_blockpy::lower_python_to_blockpy_for_testing;
    use soac_blockpy::passes::instrument_bb_module_with_block_entry_counters;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn counter_dump_record_includes_block_entry_metadata_and_value() {
        let mut lowered = lower_python_to_blockpy_for_testing(
            r#"
def f():
    return None
"#,
        )
        .expect("transform should succeed")
        .codegen_module;
        instrument_bb_module_with_block_entry_counters(&mut lowered);

        let function = lowered
            .callable_defs
            .iter()
            .find(|function| function.names.bind_name == "f")
            .expect("missing lowered function f");
        let function_id = function.function_id.0 as u32;
        let entry_label = function.entry_block().label;
        let entry_label_text = entry_label.to_string();

        let shared_state = SharedModuleState {
            function_index_by_id: build_function_index_by_id(&lowered)
                .expect("function index should build"),
            codegen_constants: ModuleCodegenConstants::collect_from_module(&lowered),
            module_constant_objs: Vec::new(),
            counter_slots_by_id: vec![0].into_boxed_slice(),
            counter_values: vec![3].into_boxed_slice(),
            lowered_module: lowered,
            module_name: "counter_test".to_string(),
            package_name: String::new(),
        };

        let record = shared_state
            .counter_dump_record()
            .expect("counter dump record should be present");
        assert_eq!(record.module_name, "counter_test");
        let row = record
            .rows
            .iter()
            .find(|row| {
                row.kind == "block_entry"
                    && row.block_label.as_deref() == Some(entry_label_text.as_str())
            })
            .expect("entry block counter row should be present");
        assert_eq!(row.scope, "this");
        assert_eq!(row.kind, "block_entry");
        assert_eq!(row.site_kind, "block_entry");
        assert_eq!(row.function_id, Some(function_id));
        assert_eq!(row.function_qualname.as_deref(), Some("f"));
        assert_eq!(row.block_label, Some(entry_label_text));
        assert_eq!(row.value, 3);
    }

    #[test]
    fn counter_scope_controls_storage_sharing() {
        let counter_defs = vec![
            CounterDef {
                id: CounterId(0),
                scope: CounterScope::Function,
                kind: "runtime_incref".to_string(),
                site: CounterSite::Runtime {
                    function_id: Some(FunctionId(7)),
                },
            },
            CounterDef {
                id: CounterId(1),
                scope: CounterScope::Function,
                kind: "runtime_incref".to_string(),
                site: CounterSite::Runtime {
                    function_id: Some(FunctionId(7)),
                },
            },
            CounterDef {
                id: CounterId(2),
                scope: CounterScope::Global,
                kind: "runtime_decref".to_string(),
                site: CounterSite::Runtime { function_id: None },
            },
            CounterDef {
                id: CounterId(3),
                scope: CounterScope::Global,
                kind: "runtime_decref".to_string(),
                site: CounterSite::Runtime { function_id: None },
            },
            CounterDef {
                id: CounterId(4),
                scope: CounterScope::This,
                kind: "block_entry".to_string(),
                site: CounterSite::BlockEntry {
                    function_id: FunctionId(7),
                    block_label: soac_blockpy::block_py::BlockLabel::from_index(0),
                },
            },
        ];

        let (slots_by_id, counter_values) =
            build_counter_storage(&counter_defs).expect("counter storage should build");
        assert_eq!(counter_values.len(), 3);
        assert_eq!(slots_by_id[0], slots_by_id[1]);
        assert_eq!(slots_by_id[2], slots_by_id[3]);
        assert_ne!(slots_by_id[0], slots_by_id[2]);
        assert_ne!(slots_by_id[0], slots_by_id[4]);
        assert_ne!(slots_by_id[2], slots_by_id[4]);
    }

    #[test]
    fn append_counter_dump_file_writes_binary_record() {
        let mut lowered = lower_python_to_blockpy_for_testing(
            r#"
VALUE = 1

def f():
    return VALUE
"#,
        )
        .expect("transform should succeed")
        .codegen_module;
        instrument_bb_module_with_block_entry_counters(&mut lowered);

        let shared_state = SharedModuleState {
            function_index_by_id: build_function_index_by_id(&lowered)
                .expect("function index should build"),
            codegen_constants: ModuleCodegenConstants::collect_from_module(&lowered),
            module_constant_objs: Vec::new(),
            counter_slots_by_id: vec![0, 1].into_boxed_slice(),
            counter_values: vec![5, 8].into_boxed_slice(),
            lowered_module: lowered,
            module_name: "counter_test".to_string(),
            package_name: "pkg".to_string(),
        };

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "soac_counter_dump_module_type_{unique}_{}.bin",
            std::process::id()
        ));
        if path.exists() {
            fs::remove_file(&path).expect("stale temp file should be removable");
        }

        shared_state
            .append_counter_dump_file(path.as_path())
            .expect("counter dump file should be written");

        let bytes = fs::read(&path).expect("counter dump file should be readable");
        assert!(bytes.starts_with(COUNTER_DUMP_MAGIC.as_slice()));
        assert!(!bytes.is_empty());

        fs::remove_file(&path).expect("temp file should be removable");
    }
}

use cranelift::prelude::*;
use cranelift::codegen::ir::{FuncRef, Function};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{Linkage, Module, default_libcall_names};

use pyo3::ffi;

/// Create a Cranelift JIT module with Python FFI symbols registered.
pub fn cranelift_module() -> JITModule {
    let mut builder = JITBuilder::new(default_libcall_names()).unwrap();
    builder.symbol("PyObject_Print", ffi::PyObject_Print as *const u8);
    builder.symbol("Py_IncRef", ffi::Py_IncRef as *const u8);
    JITModule::new(builder)
}

/// Declare the `PyObject_Print` function for use within `func`.
pub fn declare_pyobject_print(module: &mut JITModule, func: &mut Function) -> FuncRef {
    let ptr_ty = module.target_config().pointer_type();
    let mut print_sig = module.make_signature();
    print_sig.params.push(AbiParam::new(ptr_ty));
    print_sig.params.push(AbiParam::new(ptr_ty));
    print_sig.params.push(AbiParam::new(types::I32));
    print_sig.returns.push(AbiParam::new(types::I32));
    let print = module
        .declare_function("PyObject_Print", Linkage::Import, &print_sig)
        .unwrap();
    module.declare_func_in_func(print, func)
}

/// Declare the `Py_IncRef` function for use within `func`.
pub fn declare_py_incref(module: &mut JITModule, func: &mut Function) -> FuncRef {
    let ptr_ty = module.target_config().pointer_type();
    let mut inc_sig = module.make_signature();
    inc_sig.params.push(AbiParam::new(ptr_ty));
    let inc = module
        .declare_function("Py_IncRef", Linkage::Import, &inc_sig)
        .unwrap();
    module.declare_func_in_func(inc, func)
}


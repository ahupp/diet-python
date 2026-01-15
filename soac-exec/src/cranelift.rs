use cranelift::prelude::*;
use cranelift_module::{Linkage, Module};
use pyo3::ffi;

use crate::oython_ffi::{cranelift_module, declare_pyobject_print, declare_py_incref};

/// Build a zero-argument Python function that prints "hello, world".
pub unsafe fn build_jit(
    msg: *mut ffi::PyObject,
    out: *mut libc::FILE,
) -> unsafe extern "C" fn(*mut ffi::PyObject, *mut ffi::PyObject) -> *mut ffi::PyObject {
    let mut module = cranelift_module();

    let mut ctx = module.make_context();
    let mut func_ctx = FunctionBuilderContext::new();

    let ptr_ty = module.target_config().pointer_type();
    ctx.func.signature.params.push(AbiParam::new(ptr_ty));
    ctx.func.signature.params.push(AbiParam::new(ptr_ty));
    ctx.func.signature.returns.push(AbiParam::new(ptr_ty));

    let mut fb = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);
    let block = fb.create_block();
    fb.append_block_params_for_function_params(block);
    fb.switch_to_block(block);
    fb.seal_block(block);

    // Call PyObject_Print(msg, out, Py_PRINT_RAW)
    let print = declare_pyobject_print(&mut module, fb.func);

    let msg_val = fb.ins().iconst(ptr_ty, msg as i64);
    let out_val = fb.ins().iconst(ptr_ty, out as i64);
    let flag_val = fb.ins().iconst(types::I32, 1); // Py_PRINT_RAW
    fb.ins().call(print, &[msg_val, out_val, flag_val]);

    // Py_IncRef(Py_None)
    let inc = declare_py_incref(&mut module, fb.func);
    let none_ptr = ffi::Py_None() as i64;
    let none_val = fb.ins().iconst(ptr_ty, none_ptr);
    fb.ins().call(inc, &[none_val]);

    fb.ins().return_(&[none_val]);
    fb.finalize();

    let func_id = module
        .declare_function("run", Linkage::Export, &ctx.func.signature)
        .unwrap();
    module.define_function(func_id, &mut ctx).unwrap();
    module.clear_context(&mut ctx);
    module.finalize_definitions().unwrap();

    let code = module.get_finalized_function(func_id);
    std::mem::transmute(code)
}

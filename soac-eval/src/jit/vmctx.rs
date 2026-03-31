use super::ObjPtr;
use std::ffi::c_void;
use std::mem::offset_of;

#[repr(C)]
pub struct JitModuleVmCtx {
    pub module_obj: ObjPtr,
    pub module_state: *mut c_void,
    pub globals_obj: ObjPtr,
    pub true_obj: ObjPtr,
    pub false_obj: ObjPtr,
    pub none_obj: ObjPtr,
    pub deleted_obj: ObjPtr,
    pub empty_tuple_obj: ObjPtr,
}

pub const MODULE_OBJ_OFFSET: i32 = offset_of!(JitModuleVmCtx, module_obj) as i32;
pub const MODULE_STATE_OFFSET: i32 = offset_of!(JitModuleVmCtx, module_state) as i32;
pub const GLOBALS_OBJ_OFFSET: i32 = offset_of!(JitModuleVmCtx, globals_obj) as i32;
pub const TRUE_OBJ_OFFSET: i32 = offset_of!(JitModuleVmCtx, true_obj) as i32;
pub const FALSE_OBJ_OFFSET: i32 = offset_of!(JitModuleVmCtx, false_obj) as i32;
pub const NONE_OBJ_OFFSET: i32 = offset_of!(JitModuleVmCtx, none_obj) as i32;
pub const DELETED_OBJ_OFFSET: i32 = offset_of!(JitModuleVmCtx, deleted_obj) as i32;
pub const EMPTY_TUPLE_OBJ_OFFSET: i32 = offset_of!(JitModuleVmCtx, empty_tuple_obj) as i32;

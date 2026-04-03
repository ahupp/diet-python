use pyo3::ffi;
use pyo3::prelude::*;
use soac_blockpy::block_py::{
    AbruptKind, BlockArg, BlockPyFunction, BlockPyLiteral, BlockPyModule, BlockPyNameLike,
    BlockPyTerm, CodegenBlockPyExpr, CoreBlockPyExpr, CoreBlockPyKeywordArg,
    CoreNumberLiteralValue, InstrExprNode, LocatedCodegenBlockPyExpr, LocatedCoreBlockPyExpr,
    ParamDefaultSource, operation as blockpy_intrinsics,
};
use soac_blockpy::passes::CodegenBlockPyPass;
use std::collections::HashMap;

const ALWAYS_REQUIRED_UNICODE_CONSTANTS: &[&str] = &[
    "dict",
    "list",
    "raise_from",
    "tuple_from_iter",
    "append",
    "extend",
    "update",
];

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct ModuleConstantId(pub usize);

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
enum ModuleConstantValue {
    Unicode(Vec<u8>),
    Bytes(Vec<u8>),
    Int(i64),
    BigInt(String),
    FloatBits(u64),
}

#[derive(Debug, Clone, Default)]
pub struct ModuleCodegenConstants {
    values: Vec<ModuleConstantValue>,
    ids: HashMap<ModuleConstantValue, ModuleConstantId>,
}

impl ModuleCodegenConstants {
    pub fn collect_from_module(module: &BlockPyModule<CodegenBlockPyPass>) -> Self {
        let mut collector = ModuleConstantCollector::default();
        for expr in &module.module_constants {
            collector.constants.push_explicit_literal(expr);
        }
        for name in ALWAYS_REQUIRED_UNICODE_CONSTANTS {
            collector.constants.intern_unicode_bytes(name.as_bytes());
        }
        for function in &module.callable_defs {
            collector.collect_function(function);
        }
        collector.constants
    }

    pub fn collect_from_functions<'a>(
        functions: impl IntoIterator<Item = &'a BlockPyFunction<CodegenBlockPyPass>>,
    ) -> Self {
        let mut collector = ModuleConstantCollector::default();
        for name in ALWAYS_REQUIRED_UNICODE_CONSTANTS {
            collector.constants.intern_unicode_bytes(name.as_bytes());
        }
        for function in functions {
            collector.collect_function(function);
        }
        collector.constants
    }

    pub fn build_python_constants(&self, py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        let mut out = Vec::with_capacity(self.values.len());
        for value in &self.values {
            out.push(match value {
                ModuleConstantValue::Unicode(bytes) => {
                    let ptr = unsafe {
                        ffi::PyUnicode_DecodeUTF8(
                            bytes.as_ptr() as *const i8,
                            bytes.len() as ffi::Py_ssize_t,
                            c"surrogatepass".as_ptr(),
                        )
                    };
                    let bound: Bound<'_, PyAny> = unsafe { Bound::from_owned_ptr_or_err(py, ptr)? };
                    bound.unbind()
                }
                ModuleConstantValue::Bytes(bytes) => {
                    let ptr = unsafe {
                        ffi::PyBytes_FromStringAndSize(
                            bytes.as_ptr() as *const i8,
                            bytes.len() as ffi::Py_ssize_t,
                        )
                    };
                    let bound: Bound<'_, PyAny> = unsafe { Bound::from_owned_ptr_or_err(py, ptr)? };
                    bound.unbind()
                }
                ModuleConstantValue::Int(value) => {
                    let ptr = unsafe { ffi::PyLong_FromLongLong(*value as std::ffi::c_longlong) };
                    let bound: Bound<'_, PyAny> = unsafe { Bound::from_owned_ptr_or_err(py, ptr)? };
                    bound.unbind()
                }
                ModuleConstantValue::BigInt(value) => {
                    let value = std::ffi::CString::new(value.as_str())
                        .expect("big int literal should not contain NUL");
                    let mut end_ptr = std::ptr::null_mut();
                    let ptr = unsafe { ffi::PyLong_FromString(value.as_ptr(), &mut end_ptr, 0) };
                    let bound: Bound<'_, PyAny> = unsafe { Bound::from_owned_ptr_or_err(py, ptr)? };
                    bound.unbind()
                }
                ModuleConstantValue::FloatBits(bits) => {
                    let ptr = unsafe { ffi::PyFloat_FromDouble(f64::from_bits(*bits)) };
                    let bound: Bound<'_, PyAny> = unsafe { Bound::from_owned_ptr_or_err(py, ptr)? };
                    bound.unbind()
                }
            });
        }
        Ok(out)
    }

    pub fn require_unicode_constant_id(&self, value: &str) -> ModuleConstantId {
        self.require_unicode_constant_id_for_bytes(value.as_bytes())
    }

    pub fn require_unicode_constant_id_for_bytes(&self, value: &[u8]) -> ModuleConstantId {
        self.lookup_id(&ModuleConstantValue::Unicode(value.to_vec()))
            .unwrap_or_else(|| {
                panic!(
                    "missing module unicode constant in codegen pool: {:?}",
                    String::from_utf8_lossy(value)
                )
            })
    }

    pub fn require_bytes_constant_id(&self, value: &[u8]) -> ModuleConstantId {
        self.lookup_id(&ModuleConstantValue::Bytes(value.to_vec()))
            .unwrap_or_else(|| panic!("missing module bytes constant in codegen pool"))
    }

    pub fn require_int_constant_id(&self, value: i64) -> ModuleConstantId {
        self.lookup_id(&ModuleConstantValue::Int(value))
            .unwrap_or_else(|| panic!("missing module int constant in codegen pool: {value}"))
    }

    pub fn require_big_int_constant_id(&self, value: &str) -> ModuleConstantId {
        self.lookup_id(&ModuleConstantValue::BigInt(value.to_string()))
            .unwrap_or_else(|| panic!("missing module big-int constant in codegen pool: {value}"))
    }

    pub fn require_float_constant_id(&self, value: f64) -> ModuleConstantId {
        self.lookup_id(&ModuleConstantValue::FloatBits(value.to_bits()))
            .unwrap_or_else(|| panic!("missing module float constant in codegen pool: {value}"))
    }

    pub fn constant_bytes_value(&self, constant_id: ModuleConstantId) -> Option<&[u8]> {
        match self.values.get(constant_id.0)? {
            ModuleConstantValue::Bytes(bytes) => Some(bytes.as_slice()),
            ModuleConstantValue::Unicode(_)
            | ModuleConstantValue::Int(_)
            | ModuleConstantValue::BigInt(_)
            | ModuleConstantValue::FloatBits(_) => None,
        }
    }

    pub fn constant_string_bytes_value(&self, constant_id: ModuleConstantId) -> Option<&[u8]> {
        match self.values.get(constant_id.0)? {
            ModuleConstantValue::Unicode(bytes) | ModuleConstantValue::Bytes(bytes) => {
                Some(bytes.as_slice())
            }
            ModuleConstantValue::Int(_)
            | ModuleConstantValue::BigInt(_)
            | ModuleConstantValue::FloatBits(_) => None,
        }
    }

    pub fn constant_string_value(&self, constant_id: ModuleConstantId) -> Option<String> {
        match self.values.get(constant_id.0)? {
            ModuleConstantValue::Unicode(bytes) | ModuleConstantValue::Bytes(bytes) => {
                String::from_utf8(bytes.clone()).ok()
            }
            ModuleConstantValue::Int(_)
            | ModuleConstantValue::BigInt(_)
            | ModuleConstantValue::FloatBits(_) => None,
        }
    }

    fn lookup_id(&self, value: &ModuleConstantValue) -> Option<ModuleConstantId> {
        self.ids.get(value).copied()
    }

    fn push_explicit_literal(&mut self, expr: &LocatedCoreBlockPyExpr) -> ModuleConstantId {
        let CoreBlockPyExpr::Literal(literal) = expr else {
            panic!("unsupported explicit module constant expr after codegen lowering: {expr:?}");
        };

        let value = match literal.as_literal() {
            BlockPyLiteral::StringLiteral(string) => {
                ModuleConstantValue::Unicode(string.value.as_bytes().to_vec())
            }
            BlockPyLiteral::BytesLiteral(bytes) => ModuleConstantValue::Bytes(bytes.value.clone()),
            BlockPyLiteral::NumberLiteral(number) => match &number.value {
                CoreNumberLiteralValue::Int(value) => {
                    if let Some(value) = value.as_i64() {
                        ModuleConstantValue::Int(value)
                    } else {
                        ModuleConstantValue::BigInt(value.to_string())
                    }
                }
                CoreNumberLiteralValue::Float(value) => {
                    ModuleConstantValue::FloatBits(value.to_bits())
                }
            },
        };
        let id = ModuleConstantId(self.values.len());
        self.values.push(value.clone());
        self.ids.entry(value).or_insert(id);
        id
    }

    fn intern(&mut self, value: ModuleConstantValue) -> ModuleConstantId {
        if let Some(existing) = self.ids.get(&value).copied() {
            return existing;
        }
        let id = ModuleConstantId(self.values.len());
        self.values.push(value.clone());
        self.ids.insert(value, id);
        id
    }

    fn intern_unicode_bytes(&mut self, value: &[u8]) -> ModuleConstantId {
        self.intern(ModuleConstantValue::Unicode(value.to_vec()))
    }

    fn intern_bytes(&mut self, value: &[u8]) -> ModuleConstantId {
        self.intern(ModuleConstantValue::Bytes(value.to_vec()))
    }

    fn intern_int(&mut self, value: i64) -> ModuleConstantId {
        self.intern(ModuleConstantValue::Int(value))
    }

    fn intern_big_int(&mut self, value: &str) -> ModuleConstantId {
        self.intern(ModuleConstantValue::BigInt(value.to_string()))
    }

    fn intern_float(&mut self, value: f64) -> ModuleConstantId {
        self.intern(ModuleConstantValue::FloatBits(value.to_bits()))
    }
}

#[derive(Default)]
struct ModuleConstantCollector {
    constants: ModuleCodegenConstants,
}

impl ModuleConstantCollector {
    fn collect_function(&mut self, function: &BlockPyFunction<CodegenBlockPyPass>) {
        for (param, default_source) in function.params.iter_with_default_sources() {
            match default_source {
                Some(ParamDefaultSource::Positional(_)) => {
                    self.constants.intern_unicode_bytes(param.name.as_bytes());
                }
                Some(ParamDefaultSource::KeywordOnly(name)) => {
                    self.constants.intern_unicode_bytes(name.as_bytes());
                }
                None => {}
            }
        }
        for block in &function.blocks {
            for stmt in &block.body {
                self.collect_stmt(stmt);
            }
            self.collect_term(&block.term);
        }
    }

    fn collect_stmt(&mut self, stmt: &LocatedCodegenBlockPyExpr) {
        self.collect_expr(stmt);
    }

    fn collect_term(&mut self, term: &BlockPyTerm<LocatedCodegenBlockPyExpr>) {
        match term {
            BlockPyTerm::Jump(edge) => self.collect_block_args(&edge.args),
            BlockPyTerm::IfTerm(if_term) => self.collect_expr(&if_term.test),
            BlockPyTerm::BranchTable(branch_table) => self.collect_expr(&branch_table.index),
            BlockPyTerm::Raise(raise_stmt) => {
                if let Some(exc) = &raise_stmt.exc {
                    self.collect_expr(exc);
                }
            }
            BlockPyTerm::Return(value) => self.collect_expr(value),
        }
    }

    fn collect_block_args(&mut self, args: &[BlockArg]) {
        for arg in args {
            if let BlockArg::AbruptKind(kind) = arg {
                self.constants.intern_int(abrupt_kind_tag(*kind));
            }
        }
    }

    fn collect_expr(&mut self, expr: &LocatedCodegenBlockPyExpr) {
        match expr {
            CodegenBlockPyExpr::Literal(literal) => match literal.as_literal() {
                BlockPyLiteral::StringLiteral(string) => {
                    self.constants.intern_unicode_bytes(string.value.as_bytes());
                }
                BlockPyLiteral::NumberLiteral(number) => match &number.value {
                    CoreNumberLiteralValue::Int(value) => {
                        if let Some(value) = value.as_i64() {
                            self.constants.intern_int(value);
                        } else {
                            let value_text = value.to_string();
                            self.constants.intern_big_int(value_text.as_str());
                        }
                    }
                    CoreNumberLiteralValue::Float(value) => {
                        self.constants.intern_float(*value);
                    }
                },
                BlockPyLiteral::BytesLiteral(bytes) => {
                    self.constants.intern_bytes(bytes.value.as_slice());
                }
            },
            CodegenBlockPyExpr::Call(call) => {
                if let Some(const_bytes) = self.string_constant_bytes_for_specialized_codegen(expr)
                {
                    self.constants.intern_unicode_bytes(const_bytes.as_slice());
                }
                if let Some(delete_name_bytes) = self.deleted_name_arg_bytes(call) {
                    self.constants
                        .intern_unicode_bytes(delete_name_bytes.as_slice());
                }
                self.collect_expr(call.func.as_ref());
                for arg in &call.args {
                    self.collect_expr(arg.expr());
                }
                for keyword in &call.keywords {
                    if let CoreBlockPyKeywordArg::Named { arg, .. } = keyword {
                        self.constants.intern_unicode_bytes(arg.as_str().as_bytes());
                    }
                    self.collect_expr(keyword.expr());
                }
            }
            CodegenBlockPyExpr::GetAttr(op) => {
                if let Some(attr_bytes) =
                    self.string_constant_bytes_for_specialized_codegen(op.attr.as_ref())
                {
                    self.constants.intern_unicode_bytes(attr_bytes.as_slice());
                }
                op.visit_exprs(&mut |child| self.collect_expr(child));
            }
            CodegenBlockPyExpr::SetAttr(op) => {
                if let Some(attr_bytes) =
                    self.string_constant_bytes_for_specialized_codegen(op.attr.as_ref())
                {
                    self.constants.intern_unicode_bytes(attr_bytes.as_slice());
                }
                op.visit_exprs(&mut |child| self.collect_expr(child));
            }
            CodegenBlockPyExpr::Load(op)
                if op.name.location.is_global() || op.name.location.is_runtime_name() =>
            {
                self.constants
                    .intern_unicode_bytes(op.name.id_str().as_bytes());
            }
            CodegenBlockPyExpr::Load(_) => {}
            CodegenBlockPyExpr::Store(op) if op.name.location.is_global() => {
                self.constants
                    .intern_unicode_bytes(op.name.id_str().as_bytes());
                op.visit_exprs(&mut |child| self.collect_expr(child));
            }
            CodegenBlockPyExpr::Store(op) => {
                op.visit_exprs(&mut |child| self.collect_expr(child));
            }
            CodegenBlockPyExpr::Del(op) if op.name.location.is_global() => {
                self.constants
                    .intern_unicode_bytes(op.name.id_str().as_bytes());
            }
            CodegenBlockPyExpr::BinOp(op) => op.visit_exprs(&mut |child| self.collect_expr(child)),
            CodegenBlockPyExpr::UnaryOp(op) => {
                op.visit_exprs(&mut |child| self.collect_expr(child));
            }
            CodegenBlockPyExpr::GetItem(op) => {
                op.visit_exprs(&mut |child| self.collect_expr(child));
            }
            CodegenBlockPyExpr::SetItem(op) => {
                op.visit_exprs(&mut |child| self.collect_expr(child));
            }
            CodegenBlockPyExpr::DelItem(op) => {
                op.visit_exprs(&mut |child| self.collect_expr(child));
            }
            CodegenBlockPyExpr::MakeCell(op) => {
                op.visit_exprs(&mut |child| self.collect_expr(child));
            }
            CodegenBlockPyExpr::MakeFunction(op) => {
                op.visit_exprs(&mut |child| self.collect_expr(child));
            }
            CodegenBlockPyExpr::Del(_)
            | CodegenBlockPyExpr::CellRefForName(_)
            | CodegenBlockPyExpr::CellRef(_) => {}
        }
    }

    fn deleted_name_arg_bytes(
        &self,
        call: &blockpy_intrinsics::Call<LocatedCodegenBlockPyExpr>,
    ) -> Option<Vec<u8>> {
        if helper_name_for_codegen_expr(call.func.as_ref()) != Some("load_deleted_name")
            || call.args.len() != 2
        {
            return None;
        }
        self.string_constant_bytes_for_specialized_codegen(call.args[0].expr())
    }

    fn string_constant_bytes_for_specialized_codegen(
        &self,
        expr: &LocatedCodegenBlockPyExpr,
    ) -> Option<Vec<u8>> {
        match expr {
            CodegenBlockPyExpr::Literal(literal) => match literal.as_literal() {
                BlockPyLiteral::StringLiteral(string) => Some(string.value.as_bytes().to_vec()),
                BlockPyLiteral::NumberLiteral(_) => None,
                BlockPyLiteral::BytesLiteral(bytes) => Some(bytes.value.clone()),
            },
            CodegenBlockPyExpr::Load(op) => op.name.location.as_constant().and_then(|index| {
                self.constants
                    .constant_string_bytes_value(ModuleConstantId(index as usize))
                    .map(ToOwned::to_owned)
            }),
            CodegenBlockPyExpr::Call(call) => {
                if helper_name_for_codegen_expr(call.func.as_ref()) != Some("str")
                    || call.args.len() != 1
                    || !call.keywords.is_empty()
                {
                    return None;
                }
                self.string_constant_bytes_for_specialized_codegen(call.args[0].expr())
            }
            _ => None,
        }
    }
}

fn helper_name_for_codegen_expr(expr: &LocatedCodegenBlockPyExpr) -> Option<&str> {
    match expr {
        CodegenBlockPyExpr::Load(op)
            if op.name.location.is_global() || op.name.location.is_runtime_name() =>
        {
            Some(op.name.id.as_str())
        }
        _ => None,
    }
}

fn abrupt_kind_tag(kind: AbruptKind) -> i64 {
    match kind {
        AbruptKind::Fallthrough => 0,
        AbruptKind::Return => 1,
        AbruptKind::Exception => 2,
        AbruptKind::Break => 3,
        AbruptKind::Continue => 4,
    }
}

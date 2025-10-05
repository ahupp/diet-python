use std::mem::take;

use ruff_python_ast::{self as ast, Stmt};

use crate::{
    body_transform::{walk_stmt, Transformer},
    py_expr, py_stmt,
    template::py_stmt_single,
    transform::class_def::{class_call_arguments, class_ident_from_qualname},
};

pub struct NestedClassCollector {
    class_qualname: String,
    nested: Vec<(String, ast::StmtClassDef)>,
}

impl NestedClassCollector {
    pub fn new(class_qualname: String) -> Self {
        Self {
            class_qualname,
            nested: Vec::new(),
        }
    }

    pub fn into_nested(self) -> Vec<(String, ast::StmtClassDef)> {
        self.nested
    }
}

impl Transformer for NestedClassCollector {
    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        if let Stmt::ClassDef(class_def) = stmt {
            let nested_name = class_def.name.id.to_string();
            let nested_qualname = format!("{}.{}", self.class_qualname, nested_name);
            let dp_name = class_ident_from_qualname(&nested_qualname);
            let class_ident = dp_name
                .strip_prefix("_dp_class_")
                .expect("dp class names are prefixed")
                .to_string();

            let mut nested_def = class_def.clone();
            let decorators = take(&mut nested_def.decorator_list);

            let (bases_tuple, prepare_dict) = class_call_arguments(nested_def.arguments.clone());

            let mut value = py_expr!(
                "__dp__.create_class({class_name:literal}, _dp_ns_{class_ident:id}, {bases:expr}, {prepare_dict:expr})",
                class_name = nested_name.as_str(),
                class_ident = class_ident.as_str(),
                bases = bases_tuple,
                prepare_dict = prepare_dict,
            );
            for decorator in decorators.into_iter().rev() {
                value = py_expr!(
                    "({decorator:expr})({value:expr})",
                    decorator = decorator.expression,
                    value = value,
                );
            }

            self.nested.push((dp_name.clone(), nested_def));

            *stmt = py_stmt_single(py_stmt!(
                "{name:id} = {value:expr}",
                name = nested_name.as_str(),
                value = value,
            ));

            return;
        }

        if matches!(stmt, Stmt::FunctionDef(_)) {
            return;
        }

        walk_stmt(self, stmt);
    }
}

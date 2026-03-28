use super::{rewrite, validate_future_imports};
use crate::passes::ast_to_ast::context::Context;
use ruff_python_parser::parse_module;
use std::collections::HashSet;

fn rewrite_module(source: &str) -> (HashSet<String>, String) {
    let mut module = parse_module(source)
        .expect("parse should succeed")
        .into_syntax();
    validate_future_imports(&module.body).expect("future imports should be valid");
    let context = Context::new(source);
    let future_features = rewrite(&context, &mut module.body);
    (future_features, crate::ruff_ast_to_string(&module.body))
}

#[test]
fn strips_all_future_imports_and_returns_feature_names() {
    let source = concat!(
        "from __future__ import annotations, division\n",
        "from __future__ import generator_stop\n",
        "x: Foo = 1\n",
    );

    let (future_features, rendered) = rewrite_module(source);

    assert_eq!(
        future_features,
        HashSet::from([
            "annotations".to_string(),
            "division".to_string(),
            "generator_stop".to_string(),
        ])
    );
    assert!(!rendered.contains("__future__"), "{rendered}");
    assert!(rendered.contains("x: \"Foo\" = 1"), "{rendered}");
}

#[test]
fn non_annotations_future_does_not_stringize_annotations() {
    let source = concat!("from __future__ import division\n", "x: Foo = 1\n",);

    let (future_features, rendered) = rewrite_module(source);

    assert_eq!(future_features, HashSet::from(["division".to_string()]));
    assert!(!rendered.contains("__future__"), "{rendered}");
    assert!(rendered.contains("x: Foo = 1"), "{rendered}");
}

#[test]
fn invalid_future_import_reports_parse_error() {
    let source = "from __future__ import not_a_feature\nx = 1\n";
    let module = parse_module(source)
        .expect("parse should succeed")
        .into_syntax();

    let err = validate_future_imports(&module.body).expect_err("future import should be invalid");

    assert!(err.to_string().contains("not_a_feature"), "{err}");
}

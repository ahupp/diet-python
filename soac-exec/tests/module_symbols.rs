use std::collections::HashMap;

use diet_python::transform_min_ast;
use soac_exec::module_symbols::{module_symbols, SymbolMetadata};

#[test]
fn finds_written_globals() {
    let src = r#"x = 1
y = 2
def a():
    global x
    x = 3
def b():
    z = 5
def c():
    global w
    return
"#;
    let module = transform_min_ast(src, None).unwrap();
    let symbols = module_symbols(&module);

    let mut expected = HashMap::new();
    expected.insert(
        "a".to_string(),
        SymbolMetadata {
            index: 0,
            written_after_init: false,
        },
    );
    expected.insert(
        "b".to_string(),
        SymbolMetadata {
            index: 1,
            written_after_init: false,
        },
    );
    expected.insert(
        "c".to_string(),
        SymbolMetadata {
            index: 2,
            written_after_init: false,
        },
    );
    expected.insert(
        "w".to_string(),
        SymbolMetadata {
            index: 3,
            written_after_init: true,
        },
    );
    expected.insert(
        "x".to_string(),
        SymbolMetadata {
            index: 4,
            written_after_init: true,
        },
    );
    expected.insert(
        "y".to_string(),
        SymbolMetadata {
            index: 5,
            written_after_init: false,
        },
    );

    assert_eq!(symbols.globals, expected);
}


#[cfg(test)]
#[macro_export]
macro_rules! assert_flatten_eq {
    ($actual:expr, $expected:expr $(,)?) => {{
        use ruff_python_ast::comparable::ComparableStmt;
        use ruff_python_parser::parse_module;
        let mut actual = $actual;
        crate::template::flatten(&mut actual);
        let mut expected = parse_module($expected).unwrap().into_syntax().body;
        crate::template::flatten(&mut expected);
        let actual: Vec<_> = actual.iter().map(ComparableStmt::from).collect();
        let expected: Vec<_> = expected.iter().map(ComparableStmt::from).collect();
        assert_eq!(actual, expected);
    }};
}

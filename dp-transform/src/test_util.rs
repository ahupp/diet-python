use ruff_python_ast::{comparable::ComparableStmt, Stmt};
use ruff_python_parser::parse_module;

use crate::basic_block::ast_to_ast::Options;
use crate::fixture::parse_fixture;
use crate::{ruff_ast_to_string, transform_str_to_ruff_with_options};
use similar::TextDiff;

fn expected_output_for_mode(expected: &str) -> &str {
    const BB_MARKER: &str = "# -- bb --";
    if let Some(index) = expected.find(BB_MARKER) {
        return &expected[index + BB_MARKER.len()..];
    }
    expected
}

pub(crate) fn assert_transform_eq_ex(actual: &str, expected: &str) {
    let expected_for_mode = expected_output_for_mode(expected);
    let mut expected_normalized = expected_for_mode.trim_matches('\n').to_string();
    expected_normalized.push('\n');
    let options = Options::for_test();
    let module = transform_str_to_ruff_with_options(actual, options).unwrap();
    let actual_str = ruff_ast_to_string(&module.module.body);
    let actual_body = &module.module.body.body;
    let actual_stmt_internal: Vec<_> = actual_body
        .iter()
        .map(|stmt| ComparableStmt::from(stmt.as_ref()))
        .collect();

    if std::env::var("DP_ENFORCE_IDEMPOTENCE").is_ok() {
        let rerun_module = transform_str_to_ruff_with_options(&actual_str, options).unwrap();
        let rerun_body = &rerun_module.module.body.body;
        let rerun_stmt: Vec<_> = rerun_body
            .iter()
            .map(|stmt| ComparableStmt::from(stmt.as_ref()))
            .collect();
        if actual_stmt_internal != rerun_stmt {
            let difference = format_first_difference(&module.module.body.body, rerun_body);
            panic!("transform is not idempotent: {difference}");
        }
    }

    let actual_parsed = parse_module(actual_str.as_str())
        .unwrap()
        .into_syntax()
        .body;
    let actual_stmt: Vec<_> = actual_parsed
        .body
        .iter()
        .map(|stmt| ComparableStmt::from(stmt.as_ref()))
        .collect();

    let expected_ast = parse_module(expected_normalized.as_str())
        .unwrap()
        .into_syntax()
        .body;
    let expected_body = &expected_ast.body;
    let expected_stmt: Vec<_> = expected_body
        .iter()
        .map(|stmt| ComparableStmt::from(stmt.as_ref()))
        .collect();

    if actual_stmt != expected_stmt {
        let diff = TextDiff::from_lines(expected_normalized.as_str(), &actual_str)
            .unified_diff()
            .header("expected", "actual")
            .to_string();
        panic!("expected desugaring to match fixture:\n{diff}");
    }
}

fn format_first_difference(actual: &[Box<Stmt>], rerun: &[Box<Stmt>]) -> String {
    let min_len = actual.len().min(rerun.len());
    for (index, (actual_stmt, rerun_stmt)) in actual.iter().zip(rerun).enumerate().take(min_len) {
        if ComparableStmt::from(actual_stmt.as_ref()) != ComparableStmt::from(rerun_stmt.as_ref()) {
            let actual_str = ruff_ast_to_string(actual_stmt.as_ref());
            let rerun_str = ruff_ast_to_string(rerun_stmt.as_ref());
            return format!(
                "first difference at stmt index {index}:\nactual: {actual_str}\nrerun: {rerun_str}"
            );
        }
    }

    if actual.len() != rerun.len() {
        if actual.len() > rerun.len() {
            let remainder = &actual[rerun.len()..];
            let remainder_str = ruff_ast_to_string(remainder);
            format!(
                "rerun dropped {} trailing statement(s): {remainder_str}",
                actual.len() - rerun.len()
            )
        } else {
            let remainder = &rerun[actual.len()..];
            let remainder_str = ruff_ast_to_string(remainder);
            format!(
                "rerun added {} trailing statement(s): {remainder_str}",
                rerun.len() - actual.len()
            )
        }
    } else {
        "unable to determine difference".to_string()
    }
}

pub(crate) fn assert_transform_eq_basic_blocks(actual: &str, expected: &str) {
    assert_transform_eq_ex(actual, expected);
}

pub(crate) fn run_transform_fixture_tests(fixture: &str) {
    let blocks = match parse_fixture(fixture) {
        Ok(blocks) => blocks,
        Err(err) => panic!("{err}"),
    };

    for block in blocks {
        eprintln!("transform_fixture: {}", block.name);
        assert_transform_eq_basic_blocks(block.input.as_str(), block.output.as_str());
    }
}

fn blockpy_output_for_snapshot(actual: &str) -> String {
    let lowered = transform_str_to_ruff_with_options(actual, Options::for_test()).unwrap();
    let mut output = lowered
        .get_pass::<crate::basic_block::LoweredBlockPyModuleBundle>("semantic_blockpy")
        .map(|bundle| {
            crate::basic_block::blockpy_module_to_string(
                &crate::basic_block::project_lowered_module_callable_defs(
                    bundle,
                    |lowered| -> &crate::basic_block::block_py::SemanticBlockPyCallableDef {
                        lowered
                    },
                ),
            )
        })
        .unwrap_or_else(|| "; no BlockPy module emitted".to_string())
        .trim_matches('\n')
        .to_string();
    output.push('\n');
    output
}

pub(crate) fn assert_blockpy_snapshot_eq(actual: &str, expected: &str) {
    let actual_output = blockpy_output_for_snapshot(actual);
    let mut expected_output = expected.trim_matches('\n').to_string();
    expected_output.push('\n');
    if actual_output != expected_output {
        let diff = TextDiff::from_lines(expected_output.as_str(), &actual_output)
            .unified_diff()
            .header("expected", "actual")
            .to_string();
        panic!("expected BlockPy snapshot to match fixture:\n{diff}");
    }
}

pub(crate) fn run_blockpy_snapshot_fixture_tests(fixture: &str, snapshot: &str) {
    let fixture_blocks = match parse_fixture(fixture) {
        Ok(blocks) => blocks,
        Err(err) => panic!("{err}"),
    };
    let snapshot_blocks = match parse_fixture(snapshot) {
        Ok(blocks) => blocks,
        Err(err) => panic!("{err}"),
    };

    assert_eq!(
        fixture_blocks.len(),
        snapshot_blocks.len(),
        "fixture block count does not match snapshot block count"
    );

    for (fixture_block, snapshot_block) in fixture_blocks.iter().zip(snapshot_blocks.iter()) {
        assert_eq!(
            fixture_block.name, snapshot_block.name,
            "fixture block names do not match snapshot block names"
        );
        eprintln!("blockpy_snapshot_fixture: {}", fixture_block.name);
        assert_blockpy_snapshot_eq(fixture_block.input.as_str(), snapshot_block.output.as_str());
    }
}

pub(crate) fn assert_ast_eq(actual: Stmt, expected: Stmt) {
    let actual_stmt: ComparableStmt = ComparableStmt::from(&actual);
    let expected_stmt: ComparableStmt = ComparableStmt::from(&expected);
    assert_eq!(actual_stmt, expected_stmt);
}

#[macro_export]
macro_rules! transform_fixture_test {
    ($path:literal) => {
        #[test]
        fn transform_fixture() {
            $crate::test_util::run_transform_fixture_tests(include_str!($path));
        }
    };
    ($name:ident, $path:literal) => {
        #[test]
        fn $name() {
            $crate::test_util::run_transform_fixture_tests(include_str!($path));
        }
    };
    ($name:ident, $fixture_path:literal, $snapshot_path:literal) => {
        #[test]
        fn $name() {
            $crate::test_util::run_blockpy_snapshot_fixture_tests(
                include_str!($fixture_path),
                include_str!($snapshot_path),
            );
        }
    };
}

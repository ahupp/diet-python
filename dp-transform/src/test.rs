use super::{PassTracker, RecordingPassTracker};
use crate::passes::ast_to_ast::body::Suite;
use crate::py_stmt;
use serde_json::Value;

#[test]
#[should_panic(expected = "PassTracker already contains a pass named one")]
fn pass_tracker_rejects_duplicate_names() {
    let mut tracker = RecordingPassTracker::new();
    let _suite: Suite = tracker.run_pass("one", || vec![py_stmt!("x = 1")]);
    let _suite: Suite = tracker.run_pass("one", || vec![py_stmt!("x = 2")]);
}

#[test]
fn pass_tracker_records_timing_without_storing_pass_value() {
    let mut tracker = RecordingPassTracker::new();
    let value: i32 = tracker.record_timing("timed-only", || 7);

    assert_eq!(value, 7);
    assert_eq!(
        tracker
            .pass_timings()
            .map(|timing| timing.name)
            .collect::<Vec<_>>(),
        vec!["timed-only".to_string()]
    );
    assert_eq!(tracker.render_pass_text("timed-only"), None);
}

#[test]
fn pass_tracker_renders_tracked_pass_text_for_renderable_passes() {
    let mut tracker = RecordingPassTracker::new();
    let _suite: Suite = tracker.run_pass("one", || vec![py_stmt!("x = 1")]);

    assert_eq!(tracker.render_pass_text("one").as_deref(), Some("x = 1\n"));
    assert_eq!(
        tracker
            .pass_timings()
            .map(|timing| timing.name)
            .collect::<Vec<_>>(),
        vec!["one".to_string()]
    );
}

#[test]
fn render_inspector_payload_includes_lowered_callable_metadata() {
    let source = "def f(x):\n    return x\n";
    let lowered = crate::transform_str_to_ruff_with_options(source, crate::Options::default())
        .expect("source should lower");
    let payload: Value = serde_json::from_str(&crate::render_inspector_payload(source, &lowered))
        .expect("inspector payload should be valid JSON");

    let steps = payload["steps"]
        .as_array()
        .expect("inspector payload should include step array");
    assert_eq!(steps[0]["key"], "input_source");

    let functions = payload["functions"]
        .as_array()
        .expect("inspector payload should include function array");
    let function = functions
        .iter()
        .find(|function| function["qualname"] == "f")
        .expect("inspector payload should include lowered function metadata");
    assert_eq!(function["displayName"], "f");
    assert!(function["functionId"].as_u64().is_some());
    assert!(function["entryLabel"]
        .as_str()
        .is_some_and(|entry_label| !entry_label.is_empty()));
}

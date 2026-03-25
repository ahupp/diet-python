use super::PassTracker;
use crate::passes::ast_to_ast::body::Suite;
use crate::py_stmt;

#[test]
#[should_panic(expected = "PassTracker already contains a pass named one")]
fn pass_tracker_rejects_duplicate_names() {
    let mut tracker = PassTracker::new();
    let _ = tracker.run_pass("one", || 1_i32);
    let _ = tracker.run_pass("one", || 2_i32);
}

#[test]
fn pass_tracker_renders_tracked_pass_text_for_renderable_passes() {
    let mut tracker = PassTracker::new();
    let _suite: Suite = tracker.run_renderable_pass("one", || vec![py_stmt!("x = 1")]);

    assert_eq!(tracker.render_text("one").as_deref(), Some("x = 1\n"));
}

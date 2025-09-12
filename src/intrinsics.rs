/// Embedded Python intrinsics module.
///
/// The contents of `__dp__.py` are included at compile time and exposed as a
/// constant so consumers can embed the intrinsic helpers.
pub const DP_SOURCE: &str = include_str!("../__dp__.py");

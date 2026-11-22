//! Minimal assertions that name what failed.

pub fn that(cond: bool, what: &str) {
    assert!(cond, "FAILED: {what}");
}

pub fn equal<T: PartialEq + std::fmt::Debug>(want: T, got: T, what: &str) {
    assert!(want == got, "FAILED: {what}: expected {want:?}, got {got:?}");
}

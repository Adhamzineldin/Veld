//! The formatter must report deeply nested input as a diagnostic, not overflow the stack.
#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

use veld_tools::format;

#[test]
fn formatting_deeply_nested_types_returns_a_diagnostic() {
    let ty = format!("{}int{}", "Map<string, ".repeat(10_000), ">".repeat(10_000));
    let src = format!("model A {{ a: {ty} }}");
    let result = std::thread::Builder::new()
        .stack_size(256 * 1024)
        .spawn(move || format(&src))
        .unwrap()
        .join()
        .expect("formatting deeply nested input must not overflow the stack");
    let diags = result.unwrap_err();
    assert!(diags.iter().any(|d| d.code == "E0005"));
}

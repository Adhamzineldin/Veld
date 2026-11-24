//! Runs the shared vector files `tests/rules/core/*.json` against hand-built IR.

use std::path::PathBuf;

use veld_ir::Ir;

use super::models;
use crate::vectors::{load_cases, run_case};

fn vector_file(name: &str) -> String {
    let path: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "tests",
        "rules",
        "core",
        name,
    ]
    .iter()
    .collect();
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// Runs every case of `file` and fails with all mismatches; returns the number of cases.
fn run_file(ir: &Ir, file: &str) -> usize {
    let cases = load_cases(&vector_file(file)).unwrap();
    let failures: Vec<String> = cases
        .iter()
        .filter_map(|c| run_case(ir, c).err().map(|m| m.to_string()))
        .collect();
    assert!(failures.is_empty(), "{file}:\n{}", failures.join("\n"));
    cases.len()
}

#[test]
fn decimal_vectors() {
    assert_eq!(run_file(&models::decimal(), "decimal.json"), 14);
}

#[test]
fn numbers_vectors() {
    assert_eq!(run_file(&models::numbers(), "numbers.json"), 22);
}

#[test]
fn logic_vectors() {
    assert_eq!(run_file(&models::logic(), "logic.json"), 6);
}

#[test]
fn strings_vectors() {
    assert_eq!(run_file(&models::strings(), "strings.json"), 13);
}

#[test]
fn time_vectors() {
    assert_eq!(run_file(&models::time_ir(), "time.json"), 18);
}

#[test]
fn lists_vectors() {
    assert_eq!(run_file(&models::lists(), "lists.json"), 11);
}

#[test]
fn nested_vectors() {
    assert_eq!(run_file(&models::nested(), "nested.json"), 12);
}

#[test]
fn optional_vectors() {
    assert_eq!(run_file(&models::optional(), "optional.json"), 8);
}

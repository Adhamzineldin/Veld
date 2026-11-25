//! Running examples offline, against the reference evaluator.

use veld_ir::{Example, Ir, Origin};
use veld_rules::{ExampleError, ExampleReport, run_example};

/// An example and what came of running it.
#[derive(Debug, Clone)]
pub struct ExampleRun {
    pub name: String,
    pub origin: Origin,
    /// `Err` when the example cannot be filled in or does not fit the IR.
    pub result: Result<ExampleReport, ExampleError>,
}

/// The examples of `ir` whose name, or `Service.endpoint`, contains `filter` (ignoring case),
/// in a stable order: by file, line, column and name.
#[must_use]
pub fn select<'a>(ir: &'a Ir, filter: Option<&str>) -> Vec<&'a Example> {
    let needle = filter.map(str::to_lowercase);
    let mut out: Vec<&Example> = ir
        .examples
        .iter()
        .filter(|ex| {
            needle
                .as_deref()
                .is_none_or(|n| haystack(ir, ex).contains(n))
        })
        .collect();
    out.sort_by(|a, b| key(a).cmp(&key(b)));
    out
}

fn haystack(ir: &Ir, ex: &Example) -> String {
    let service = ex
        .service
        .and_then(|s| ir.services.get(s.index()))
        .map_or("", |s| s.name.as_str());
    let event = ex
        .event
        .as_ref()
        .map_or("", |e| ir.decl(e.decl).name.as_str());
    format!("{}\n{service}.{}\n{event}", ex.name, ex.endpoint).to_lowercase()
}

fn key(ex: &Example) -> (&str, u32, u32, &str) {
    (&ex.origin.file, ex.origin.line, ex.origin.col, &ex.name)
}

/// Runs the examples of `ir` that match `filter` offline (`spec/rules.md` section 12.3).
#[must_use]
pub fn run_offline(ir: &Ir, filter: Option<&str>) -> Vec<ExampleRun> {
    run_examples(ir, &select(ir, filter))
}

/// Runs these examples offline, in the order given.
#[must_use]
pub fn run_examples(ir: &Ir, examples: &[&Example]) -> Vec<ExampleRun> {
    examples
        .iter()
        .map(|ex| ExampleRun {
            name: ex.name.clone(),
            origin: ex.origin.clone(),
            result: run_example(ir, ex),
        })
        .collect()
}

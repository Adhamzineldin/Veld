//! `veld test --live <url> --fuzz <N>`: generated requests against a live test handler
//! (`spec/rules.md` section 12.3).

use anyhow::Result;
use veld_ir::Ir;
use veld_testing::{EndpointReport, FuzzMismatch, FuzzOptions, Live, fuzz};

use crate::error::{Exit, user};
use crate::ui::{Style, Ui};

/// A seed from the clock, for a run that was not given one.
fn clock_seed() -> u64 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    u64::try_from(nanos & 0xFFFF_FFFF_FFFF).unwrap_or(0)
}

/// Fuzzes every endpoint and prints what was sent and what disagreed.
pub fn run(ui: &Ui, ir: &Ir, live: &Live, count: usize, seed: Option<u64>) -> Result<Exit> {
    let seed = seed.unwrap_or_else(clock_seed);
    ui.info(&format!(
        "fuzz: {count} requests per endpoint, seed {seed} (repeat with --seed {seed})"
    ));
    let report = fuzz(ir, live, &FuzzOptions { count, seed }).map_err(|e| user(e.to_string()))?;
    let mut skipped = 0;
    for e in &report.endpoints {
        match &e.skipped {
            Some(why) => {
                skipped += 1;
                let line = format!("skip {}.{} ({why})", e.service, e.endpoint);
                ui.info(&ui.out_styled(Style::Dim, &line));
            }
            None => ui.info(&sent_line(e)),
        }
    }
    let failed = report
        .endpoints
        .iter()
        .filter(|e| e.mismatch.is_some())
        .count();
    for e in &report.endpoints {
        if let Some(m) = &e.mismatch {
            print_mismatch(ui, e, m, seed);
        }
    }
    let fuzzed = report.endpoints.len() - skipped;
    let mut parts = vec![format!("{fuzzed} endpoints fuzzed")];
    if skipped > 0 {
        parts.push(format!("{skipped} skipped"));
    }
    if failed > 0 {
        parts.insert(0, format!("{failed} disagree with the reference evaluator"));
        eprintln!(
            "{}",
            crate::ui::render::fail(&ui.theme_err(), &parts.join(", "))
        );
        return Ok(Exit::User);
    }
    ui.info(&crate::ui::render::ok(&ui.theme_out(), &parts.join(", ")));
    Ok(Exit::Ok)
}

/// `Service.endpoint  200 sent (36 boundary)  ok 70, ValidationError 80, ...`
fn sent_line(e: &EndpointReport) -> String {
    let kinds: Vec<String> = e.outcomes.iter().map(|(k, n)| format!("{k} {n}")).collect();
    format!(
        "fuzz {}.{}: {} sent ({} boundary): {}",
        e.service,
        e.endpoint,
        e.sent,
        e.boundaries,
        kinds.join(", ")
    )
}

fn print_mismatch(ui: &Ui, e: &EndpointReport, m: &FuzzMismatch, seed: u64) {
    let kind = if m.boundary {
        "boundary request"
    } else {
        "request"
    };
    ui.data(&format!(
        "{} {}.{} (seed {seed}, {kind} {})",
        ui.out_styled(Style::Red, "FAIL"),
        e.service,
        e.endpoint,
        m.index
    ));
    ui.data(&format!("     expected: {}", m.expected));
    ui.data(&format!("     actual:   {}", m.actual));
    for d in &m.differences {
        ui.data(&format!("     {d}"));
    }
    match &m.example {
        Some(block) => {
            ui.data(
                "     the request as an example (paste it into a contract; offline it passes):",
            );
            for line in block.lines() {
                ui.data(&format!("       {line}"));
            }
        }
        None => ui.data("     (this request cannot be written as an example block)"),
    }
}

//! `veld test --coverage` (`spec/rules.md` section 19.1): which rules the examples of a contract
//! exercise.
//!
//! The units are counted from the IR ([`units`]); what each example evaluated comes from the tape
//! of the reference pipeline ([`veld_rules::coverage`]), so the numbers are the same with and
//! without `--live`.

use std::collections::{BTreeMap, BTreeSet};

use veld_ir::{Decl, DeclId, DeclKind, Example, Ir, Origin};
use veld_rules::coverage::{UnitEvent, allow_key, collect};
use veld_rules::run_example;

/// What kind of rule a unit is. It decides how a missing half is worded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnitKind {
    /// An invariant of a declaration.
    Invariant,
    Require,
    Ensure,
    /// An `allow` line: held is permitted, violated is denied.
    Allow,
    /// A `transition` line: held is a legal move, violated is an illegal one.
    Transition,
}

/// One enforceable unit of the workspace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unit {
    /// The rule id, or the key of an `allow` line.
    pub key: String,
    pub kind: UnitKind,
    pub package: String,
    /// The name the report gives it, such as `bookings/Bookings.payBooking allow ManageBooking`.
    pub label: String,
    pub origin: Origin,
}

/// What the examples saw of one unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Seen {
    pub held: bool,
    pub violated: bool,
}

impl Seen {
    /// Held and violated.
    #[must_use]
    pub fn covered(self) -> bool {
        self.held && self.violated
    }
}

/// The units of the packages `include` accepts, once each (by key), in report order: package
/// name, then position in the source.
#[must_use]
pub fn units(ir: &Ir, include: &dyn Fn(&str) -> bool) -> Vec<Unit> {
    let mut by_key: BTreeMap<String, Unit> = BTreeMap::new();
    for unit in invariant_units(ir).chain(endpoint_units(ir)) {
        if include(&unit.package) {
            by_key.entry(unit.key.clone()).or_insert(unit);
        }
    }
    let mut out: Vec<Unit> = by_key.into_values().collect();
    out.sort_by(|a, b| order(a).cmp(&order(b)));
    out
}

fn order(u: &Unit) -> (&str, &str, u32, u32, &str) {
    (
        &u.package,
        &u.origin.file,
        u.origin.line,
        u.origin.col,
        &u.key,
    )
}

/// The declarations whose invariants count: what a service's endpoints reach, and the events the
/// event examples deliver with what they reach. No endpoint reaches an event (`spec/rules.md`
/// section 17.1), so an event without an example adds no unit.
fn counted_decls(ir: &Ir) -> BTreeSet<DeclId> {
    let services: Vec<_> = ir.services.iter().map(|s| s.id).collect();
    let mut ids: BTreeSet<DeclId> = ir.reachable_decls(&services).into_iter().collect();
    let mut stack: Vec<DeclId> = ir
        .examples
        .iter()
        .filter_map(|e| e.event.as_ref().map(|v| v.decl))
        .collect();
    while let Some(id) = stack.pop() {
        if ids.insert(id) {
            references(ir.decl(id), &mut stack);
        }
    }
    ids
}

/// The declarations `decl` mentions: field types, the parent of a model, the arms of a union.
fn references(decl: &Decl, out: &mut Vec<DeclId>) {
    for f in decl.fields() {
        f.ty.collect_decls(out);
    }
    match &decl.kind {
        DeclKind::Model(m) => out.extend(m.parent),
        DeclKind::Union(u) => out.extend(u.arms.iter().map(|a| a.decl)),
        DeclKind::Error(_) | DeclKind::Event(_) | DeclKind::Enum(_) | DeclKind::Const(_) => {}
    }
}

fn invariant_units(ir: &Ir) -> impl Iterator<Item = Unit> + '_ {
    counted_decls(ir).into_iter().flat_map(move |id| {
        let decl = ir.decl(id);
        let package = ir.package(decl.package).name.clone();
        // An inherited rule keeps its id, so the parent's entry is the unit.
        decl.rules()
            .iter()
            .filter(|r| r.inherited_from.is_none())
            .map(|r| Unit {
                key: r.id.clone(),
                kind: UnitKind::Invariant,
                label: format!("{package}/{}.{}", decl.name, r.name),
                package: package.clone(),
                origin: r.origin.clone(),
            })
            .collect::<Vec<_>>()
    })
}

fn endpoint_units(ir: &Ir) -> impl Iterator<Item = Unit> + '_ {
    ir.services.iter().flat_map(move |s| {
        let package = ir.package(s.package).name.clone();
        s.endpoints
            .iter()
            .flat_map(|e| {
                let at = format!("{package}/{}.{}", s.name, e.name);
                statements(ir, &package, &at, e)
            })
            .collect::<Vec<_>>()
    })
}

fn statements(ir: &Ir, package: &str, at: &str, e: &veld_ir::Endpoint) -> Vec<Unit> {
    let rules = &e.rules;
    let unit = |key: &str, kind, text: String, origin: &Origin| Unit {
        key: key.to_owned(),
        kind,
        package: package.to_owned(),
        label: format!("{at} {text}"),
        origin: origin.clone(),
    };
    let mut out = Vec::new();
    for a in &rules.allows {
        let name = ir.policy(a.policy).name.clone();
        out.push(unit(
            &allow_key(&a.origin),
            UnitKind::Allow,
            format!("allow {name}"),
            &a.origin,
        ));
    }
    if let Some(t) = &rules.transition {
        let text = format!("transition {} -> {}", t.from.join(", "), t.to);
        out.push(unit(&t.id, UnitKind::Transition, text, &t.origin));
    }
    for (i, r) in rules.requires.iter().enumerate() {
        let text = format!(
            "require {}",
            r.name.clone().unwrap_or_else(|| format!("#{}", i + 1))
        );
        out.push(unit(&r.id, UnitKind::Require, text, &r.origin));
    }
    for (i, r) in rules.ensures.iter().enumerate() {
        let text = format!(
            "ensure {}",
            r.name.clone().unwrap_or_else(|| format!("#{}", i + 1))
        );
        out.push(unit(&r.id, UnitKind::Ensure, text, &r.origin));
    }
    out
}

/// The result of running the examples against the units.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Coverage {
    pub units: Vec<(Unit, Seen)>,
}

/// Runs `examples` against the reference evaluator and counts what they exercise, for the units
/// of the packages `include` accepts. An example that cannot be run counts nothing. Fuzzed
/// requests are not examples and do not count (`spec/rules.md` section 19.1).
#[must_use]
pub fn measure(ir: &Ir, examples: &[&Example], include: &dyn Fn(&str) -> bool) -> Coverage {
    let mut seen: BTreeMap<String, Seen> = BTreeMap::new();
    for ex in examples {
        let (_, events) = collect(|| run_example(ir, ex));
        for UnitEvent { key, held } in events {
            let s = seen.entry(key).or_default();
            if held {
                s.held = true;
            } else {
                s.violated = true;
            }
        }
    }
    let units = units(ir, include)
        .into_iter()
        .map(|u| {
            let s = seen.get(&u.key).copied().unwrap_or_default();
            (u, s)
        })
        .collect();
    Coverage { units }
}

impl Coverage {
    /// The number of units that are held and violated.
    #[must_use]
    pub fn covered(&self) -> usize {
        self.units.iter().filter(|(_, s)| s.covered()).count()
    }

    /// The percentage covered, rounded down; 100 when there is no unit.
    #[must_use]
    pub fn percent(&self) -> u32 {
        let total = self.units.len();
        if total == 0 {
            return 100;
        }
        u32::try_from(self.covered() * 100 / total).unwrap_or(100)
    }

    /// The report: the totals line, then every unit that is not covered, with what is missing.
    #[must_use]
    pub fn report(&self) -> Vec<String> {
        if self.units.is_empty() {
            return vec!["coverage: no rules".to_owned()];
        }
        let head = format!(
            "coverage: {} of {} rules held and violated ({}%)",
            self.covered(),
            self.units.len(),
            self.percent()
        );
        let missing: Vec<(&Unit, &'static str)> = self
            .units
            .iter()
            .filter(|(_, s)| !s.covered())
            .map(|(u, s)| (u, what_is_missing(u.kind, *s)))
            .collect();
        let width = missing
            .iter()
            .map(|(u, _)| u.label.chars().count())
            .max()
            .unwrap_or(0);
        let mut lines = vec![head];
        for (u, text) in missing {
            lines.push(format!("  {:<width$}   {text}", u.label));
        }
        lines
    }
}

/// The words for what an uncovered unit lacks (`spec/rules.md` section 19.1).
fn what_is_missing(kind: UnitKind, seen: Seen) -> &'static str {
    match (kind, seen.held, seen.violated) {
        (_, false, false) => "never evaluated",
        (UnitKind::Allow, _, false) => "never denied",
        (UnitKind::Allow, false, _) => "never permitted",
        (UnitKind::Transition, _, false) => "never illegal",
        (UnitKind::Transition, false, _) => "never legal",
        (_, _, false) => "never violated",
        (_, false, _) => "never held",
        (_, true, true) => "covered",
    }
}

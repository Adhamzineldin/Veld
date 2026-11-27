//! Flags in `veld diff` (`spec/platform.md` section 11).
//!
//! A statement behind `when flag("f")` exists only while `f` is on, so what it does to a caller
//! depends on a switch the contract does not own. The statements that involve a flag, on either
//! side, are therefore taken out of both contracts before the ordinary comparison and judged here:
//!
//! | Change | Restricts (invariant on a request, `require`, `forbid`) | Grants (`permit`, `flow`) | Guarantee (`ensure`, invariant on a response) |
//! |---|---|---|---|
//! | a flagged statement is added | possibly breaking: "breaking once the flag `f` is on" | compatible | compatible |
//! | a flag is added to a statement | compatible | breaking (`flow`), possibly breaking (`permit`): it may be gone | compatible |
//! | the flag is taken off a statement | judged as adding it (breaking for a `require`) | compatible | compatible |
//! | a flagged statement is removed | compatible | possibly breaking | compatible |
//! | the flag changes from `f` to `g` | possibly breaking | possibly breaking | compatible |

use std::collections::{BTreeMap, BTreeSet};

use veld_ir::{DeclKind, Ir, PolicyEffect};

use super::expr_text::expr_text;
use super::rules::Position;
use super::usage::Usage;
use super::{Change, ChangeKind, Severity};

/// What a statement does to the caller of the contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Effect {
    Restricts,
    Grants,
    Guarantees,
}

#[derive(Debug, Clone)]
struct Stmt {
    flag: Option<String>,
    effect: Effect,
    /// What a change is reported about.
    subject: String,
    /// "rule `x`", "`require ..`" and so on.
    what: String,
    /// The severity of adding the statement without a flag.
    added: Severity,
    /// Flows are removed and so are not simply "compatible" when they gain a flag.
    flow: bool,
}

type Stmts = BTreeMap<String, Stmt>;

/// True when either contract has a flag on any statement.
pub(super) fn involved(old: &Ir, new: &Ir) -> bool {
    [old, new].iter().any(|ir| {
        collect(ir, &Usage::of(old, new))
            .values()
            .any(|s| s.flag.is_some())
    })
}

/// The two contracts without the statements that involve a flag on either side.
pub(super) fn strip(old: &Ir, new: &Ir) -> (Ir, Ir) {
    let usage = Usage::of(old, new);
    let (a, b) = (collect(old, &usage), collect(new, &usage));
    let keys: BTreeSet<String> = a
        .iter()
        .chain(&b)
        .filter(|(_, s)| s.flag.is_some())
        .map(|(k, _)| k.clone())
        .collect();
    let (mut o, mut n) = (old.clone(), new.clone());
    for ir in [&mut o, &mut n] {
        drop_keys(ir, &keys);
    }
    (o, n)
}

fn drop_keys(ir: &mut Ir, keys: &BTreeSet<String>) {
    let quals: Vec<String> = ir.decls.iter().map(|d| d.qualified.clone()).collect();
    for (d, q) in ir.decls.iter_mut().zip(&quals) {
        match &mut d.kind {
            DeclKind::Model(m) => m.rules.retain(|r| !keys.contains(&rule_key(q, &r.name))),
            DeclKind::Error(e) => e
                .fields
                .rules
                .retain(|r| !keys.contains(&rule_key(q, &r.name))),
            DeclKind::Enum(e) => e
                .flows
                .retain(|f| !keys.contains(&flow_key(q, &f.from, &f.to))),
            DeclKind::Event(_) | DeclKind::Union(_) | DeclKind::Const(_) => {}
        }
    }
    for ep in ir.services.iter_mut().flat_map(|s| &mut s.endpoints) {
        ep.rules
            .requires
            .retain(|r| !keys.contains(&format!("require:{}", r.id)));
        ep.rules
            .ensures
            .retain(|e| !keys.contains(&format!("ensure:{}", e.id)));
    }
}

fn rule_key(decl: &str, name: &str) -> String {
    format!("rule:{decl}.{name}")
}

fn flow_key(decl: &str, from: &str, to: &str) -> String {
    format!("flow:{decl}:{from}->{to}")
}

fn collect(ir: &Ir, usage: &Usage) -> Stmts {
    let mut out = Stmts::new();
    for d in &ir.decls {
        let q = &d.qualified;
        let request = Position::of(usage.role(q)).request;
        for r in d.rules().iter().filter(|r| r.inherited_from.is_none()) {
            let (effect, added) = if request {
                (Effect::Restricts, Severity::Breaking)
            } else {
                (Effect::Guarantees, Severity::NonBreaking)
            };
            out.insert(
                rule_key(q, &r.name),
                Stmt {
                    flag: r.flag.clone(),
                    effect,
                    subject: q.clone(),
                    what: format!("rule `{}`", r.name),
                    added,
                    flow: false,
                },
            );
        }
        if let DeclKind::Enum(e) = &d.kind {
            for f in &e.flows {
                out.insert(
                    flow_key(q, &f.from, &f.to),
                    Stmt {
                        flag: f.flag.clone(),
                        effect: Effect::Grants,
                        subject: q.clone(),
                        what: format!("flow `{} -> {}`", f.from, f.to),
                        added: Severity::NonBreaking,
                        flow: true,
                    },
                );
            }
        }
    }
    endpoint_statements(ir, &mut out);
    for p in &ir.policies {
        for c in &p.clauses {
            let effect = match c.effect {
                PolicyEffect::Permit => Effect::Grants,
                PolicyEffect::Forbid => Effect::Restricts,
            };
            let word = if effect == Effect::Grants {
                "permit"
            } else {
                "forbid"
            };
            let text = expr_text(&c.expr);
            out.insert(
                format!("policy:{}:{word}:{text}", p.qualified),
                Stmt {
                    flag: c.flag.clone(),
                    effect,
                    subject: p.qualified.clone(),
                    what: format!("`{word} when {text}`"),
                    added: if effect == Effect::Grants {
                        Severity::NonBreaking
                    } else {
                        Severity::PossiblyBreaking
                    },
                    flow: false,
                },
            );
        }
    }
    out
}

fn endpoint_statements(ir: &Ir, out: &mut Stmts) {
    for ep in ir.services.iter().flat_map(|s| &s.endpoints) {
        let subject = ep.sig.split('@').next().unwrap_or(&ep.sig).to_string();
        for r in &ep.rules.requires {
            out.insert(
                format!("require:{}", r.id),
                Stmt {
                    flag: r.flag.clone(),
                    effect: Effect::Restricts,
                    subject: subject.clone(),
                    what: format!("`require {}`", expr_text(&r.expr)),
                    added: Severity::Breaking,
                    flow: false,
                },
            );
        }
        for e in &ep.rules.ensures {
            out.insert(
                format!("ensure:{}", e.id),
                Stmt {
                    flag: e.flag.clone(),
                    effect: Effect::Guarantees,
                    subject: subject.clone(),
                    what: format!("`ensure {}`", expr_text(&e.expr)),
                    added: Severity::NonBreaking,
                    flow: false,
                },
            );
        }
    }
}

/// Judges every statement that has a flag on either side.
pub(super) fn diff_flagged(old: &Ir, new: &Ir, out: &mut Vec<Change>) {
    let usage = Usage::of(old, new);
    let (a, b) = (collect(old, &usage), collect(new, &usage));
    let keys: BTreeSet<&String> = a.keys().chain(b.keys()).collect();
    for key in keys {
        let (o, n) = (a.get(key), b.get(key));
        let flagged = o.is_some_and(|s| s.flag.is_some()) || n.is_some_and(|s| s.flag.is_some());
        if !flagged {
            continue;
        }
        out.extend(judge(o, n));
    }
}

fn change(s: &Stmt, severity: Severity, message: String) -> Change {
    Change {
        kind: ChangeKind::FlagChanged,
        severity,
        subject: s.subject.clone(),
        message,
    }
}

fn judge(old: Option<&Stmt>, new: Option<&Stmt>) -> Option<Change> {
    match (old, new) {
        (None, Some(n)) => added_flagged(n),
        (Some(o), None) => removed_flagged(o),
        (Some(o), Some(n)) => moved(o, n),
        (None, None) => None,
    }
}

fn added_flagged(n: &Stmt) -> Option<Change> {
    let f = n.flag.as_deref()?;
    Some(match n.effect {
        Effect::Restricts => change(
            n,
            Severity::PossiblyBreaking,
            format!(
                "{} was added behind the flag `{f}`: breaking once the flag `{f}` is on",
                n.what
            ),
        ),
        Effect::Grants | Effect::Guarantees => change(
            n,
            Severity::NonBreaking,
            format!("{} was added behind the flag `{f}`", n.what),
        ),
    })
}

fn removed_flagged(o: &Stmt) -> Option<Change> {
    let f = o.flag.as_deref()?;
    let severity = match o.effect {
        Effect::Grants => Severity::PossiblyBreaking,
        Effect::Restricts | Effect::Guarantees => Severity::NonBreaking,
    };
    Some(change(
        o,
        severity,
        format!("{}, behind the flag `{f}`, was removed", o.what),
    ))
}

fn moved(o: &Stmt, n: &Stmt) -> Option<Change> {
    match (o.flag.as_deref(), n.flag.as_deref()) {
        (None, Some(f)) => Some(flag_added(n, f)),
        (Some(f), None) => Some(flag_removed(n, f)),
        (Some(a), Some(b)) if a != b => {
            let severity = if n.effect == Effect::Guarantees {
                Severity::NonBreaking
            } else {
                Severity::PossiblyBreaking
            };
            Some(change(
                n,
                severity,
                format!("{} moved from the flag `{a}` to the flag `{b}`", n.what),
            ))
        }
        (Some(_) | None, Some(_) | None) => None,
    }
}

fn flag_added(n: &Stmt, f: &str) -> Change {
    let (severity, tail) = match n.effect {
        Effect::Restricts | Effect::Guarantees => (
            Severity::NonBreaking,
            "it applies only while the flag is on",
        ),
        Effect::Grants if n.flow => (Severity::Breaking, "the edge is gone while the flag is off"),
        Effect::Grants => (
            Severity::PossiblyBreaking,
            "it permits nothing while the flag is off",
        ),
    };
    change(
        n,
        severity,
        format!("{} is now behind the flag `{f}`: {tail}", n.what),
    )
}

fn flag_removed(n: &Stmt, f: &str) -> Change {
    // Taking the flag off a statement that restricts is judged as adding it.
    let (severity, tail) = match n.effect {
        Effect::Restricts => (n.added, "it now always applies"),
        Effect::Grants | Effect::Guarantees => (Severity::NonBreaking, "it now always applies"),
    };
    change(
        n,
        severity,
        format!("{} is no longer behind the flag `{f}`: {tail}", n.what),
    )
}

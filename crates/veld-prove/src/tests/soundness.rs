//! The encoder against the reference evaluator (`spec/rules.md` section 18.3): for the values of
//! the shared vectors, the encoding with the value substituted and the evaluator must agree on
//! every rule.
//!
//! Where nothing is left open and every part of the value could be fixed, the two agree exactly:
//! the encoding is satisfiable if and only if the evaluator accepts. Where a part is open, the
//! encoding may accept more, never less: if the evaluator accepts, the encoding is satisfiable.
//! All the checks of a test go to the solver in one batch.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::Duration;

use serde_json::Value as Json;
use veld_ir::{Ir, Type};
use veld_rules::vectors::load_cases;
use veld_rules::{Options, Request, check_value, eval_policy, fill_example, with_defaults};

use super::pin::pin_root;
use super::{compile_text, solver};
use crate::confirm::{Verdict, env_of, judge};
use crate::problem::Problem;
use crate::question::{endpoint_roots, permitted_term};
use crate::solver::{Backend, SolverInfo, Verdict as Answer, parse_answers, preamble};
use crate::types::{OpenPart, RootKey, RootSpec};
use crate::values::Concrete;

/// One rule on one value: what the evaluator says, and what the encoding must say.
struct Probe<'a> {
    label: String,
    problem: Problem<'a>,
    /// The evaluator accepts the value.
    accepted: bool,
    /// Nothing is open and everything was fixed: the two must agree exactly.
    exact: bool,
}

struct Outcome {
    solver: Option<SolverInfo>,
    probes: usize,
    exact: usize,
    one_sided: usize,
    failures: Vec<String>,
}

fn run(test: &str, probes: &[Probe<'_>]) -> Option<Outcome> {
    let s = solver(test)?;
    let timeout = Duration::from_secs(10);
    let mut script = preamble(s.flavour(), timeout);
    for (i, p) in probes.iter().enumerate() {
        script.push_str(&p.problem.script(i));
    }
    let out = s.run(&script, probes.len(), timeout);
    assert!(out.failed.is_none(), "{:?}", out.failed);
    let answers = parse_answers(&out.stdout, probes.len());
    let mut failures = Vec::new();
    for (p, v) in probes.iter().zip(&answers.verdicts) {
        let encoded = match v {
            Answer::Sat(_) => true,
            Answer::Unsat(_) => false,
            Answer::Unknown(r) => {
                failures.push(format!("{}: the solver gave no verdict ({r})", p.label));
                continue;
            }
        };
        let bad = if p.exact {
            encoded != p.accepted
        } else {
            p.accepted && !encoded
        };
        if bad {
            failures.push(format!(
                "{}: the evaluator {} the value, the encoding is {}",
                p.label,
                if p.accepted { "accepts" } else { "rejects" },
                if encoded {
                    "satisfiable"
                } else {
                    "unsatisfiable"
                }
            ));
        }
    }
    let exact = probes.iter().filter(|p| p.exact).count();
    Some(Outcome {
        solver: answers.info,
        probes: probes.len(),
        exact,
        one_sided: probes.len() - exact,
        failures,
    })
}

fn root_of(path: &str) -> PathBuf {
    [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "tests",
        "rules",
        path,
    ]
    .iter()
    .collect()
}

fn read(path: &str) -> String {
    let p = root_of(path);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

const VECTORS: [&str; 9] = [
    "calendar", "decimal", "lists", "logic", "nested", "numbers", "optional", "strings", "time",
];

/// The probes of one vector file: every rule of every case, on the value of the case.
fn vector_probes<'a>(ir: &'a Ir, json: &str, file: &str) -> Vec<Probe<'a>> {
    let mut out = Vec::new();
    for case in load_cases(json).unwrap() {
        if case.partial {
            continue;
        }
        let decl = ir
            .decls
            .iter()
            .find(|d| d.name == case.ty || d.qualified == case.ty)
            .unwrap();
        let options = Options {
            request: case.request,
            partial: false,
        };
        let value = with_defaults(ir, decl, &case.value, options).unwrap();
        let found = check_value(ir, decl, &value, options).unwrap();
        let violated: BTreeSet<&str> = found
            .iter()
            .filter(|v| v.path == "$")
            .map(|v| v.rule_name.as_str())
            .collect();
        for rule in decl.rules().iter().filter(|r| !r.is_audited()) {
            if case.request && rule.reads_readonly {
                continue;
            }
            let roots = [(RootKey::SelfValue, RootSpec::trusted(Type::Decl(decl.id)))].into();
            let mut p = Problem::new(ir, roots);
            let holds = p.holds(&rule.expr).unwrap();
            let complete = pin_root(&mut p, &RootKey::SelfValue, &value);
            p.assert_plain(&holds);
            out.push(Probe {
                label: format!("{file}: {} / {} / {}", case.name, decl.name, rule.name),
                exact: complete && p.open_parts().is_empty(),
                problem: p,
                accepted: !violated.contains(rule.name.as_str()),
            });
        }
    }
    out
}

#[test]
fn the_vectors_of_the_spec_agree_with_the_evaluator() {
    let sources: Vec<(String, Ir, String)> = VECTORS
        .iter()
        .map(|name| {
            let ir = compile_text(&read(&format!("core/{name}.veld")));
            (name.to_string(), ir, read(&format!("core/{name}.json")))
        })
        .collect();
    let probes: Vec<Probe<'_>> = sources
        .iter()
        .flat_map(|(name, ir, json)| vector_probes(ir, json, name))
        .collect();
    assert!(probes.len() > 150, "{} probes", probes.len());
    let Some(out) = run("vectors", &probes) else {
        return;
    };
    eprintln!(
        "soundness: {} rule checks on the vectors of tests/rules/core in one solver call ({:?}): {} exact, {} one-sided",
        out.probes, out.solver, out.exact, out.one_sided
    );
    assert!(out.failures.is_empty(), "{}", out.failures.join("\n"));
    assert!(out.exact > 100, "only {} exact checks", out.exact);
}

fn concrete_of(r: &Request) -> Concrete {
    let mut roots = std::collections::BTreeMap::new();
    for (n, v) in &r.params {
        roots.insert(RootKey::Param(n.clone()), v.clone());
    }
    for (n, v) in &r.facts {
        if let Some(v) = v {
            roots.insert(RootKey::Fact(n.clone()), v.clone());
        }
    }
    if let Some(p) = &r.principal {
        roots.insert(RootKey::Principal, p.clone());
    }
    Concrete { roots, now: r.now }
}

fn pin_all(p: &mut Problem<'_>, c: &Concrete) -> bool {
    let mut complete = true;
    for (key, json) in &c.roots {
        complete &= pin_root(p, key, json);
    }
    if p.open_parts().contains(&OpenPart::Now) {
        p.assert_plain(&format!("(= |now| {})", c.now));
    }
    complete
}

fn trusted(
    roots: std::collections::BTreeMap<RootKey, RootSpec>,
) -> std::collections::BTreeMap<RootKey, RootSpec> {
    roots
        .into_iter()
        .map(|(k, s)| {
            (
                k,
                RootSpec {
                    validated: false,
                    ..s
                },
            )
        })
        .collect()
}

fn exact_enough(p: &Problem<'_>, complete: bool) -> bool {
    complete && p.open_parts().iter().all(|o| matches!(o, OpenPart::Now))
}

/// The `require` lines and the `allow` lines of every example of the endpoint contracts.
fn endpoint_probes<'a>(ir: &'a Ir, file: &str) -> Vec<Probe<'a>> {
    let mut out = Vec::new();
    for ex in ir.examples.iter().filter(|e| e.event.is_none()) {
        let filled = fill_example(ir, ex).unwrap();
        let service = &ir.services[filled.service.index()];
        let ep = service
            .endpoints
            .iter()
            .find(|e| e.name == filled.endpoint)
            .unwrap();
        let c = concrete_of(&filled.request);
        let env = env_of(ir, &c);
        for (i, r) in ep.rules.requires.iter().enumerate() {
            let Ok(verdict) = judge(&env, &r.expr) else {
                continue;
            };
            let mut p = Problem::new(ir, trusted(endpoint_roots(service, ep)));
            let holds = p.holds(&r.expr).unwrap();
            let complete = pin_all(&mut p, &c);
            p.assert_plain(&holds);
            out.push(Probe {
                label: format!("{file}: {} / require #{i}", ex.name),
                exact: exact_enough(&p, complete),
                problem: p,
                accepted: verdict == Verdict::Holds,
            });
        }
        for (i, a) in ep.rules.allows.iter().enumerate() {
            let args: Option<Vec<Json>> = a
                .args
                .iter()
                .map(|e| {
                    veld_rules::eval(&env, e)
                        .ok()
                        .and_then(|v| v.to_json().ok())
                })
                .collect();
            let Some(args) = args else { continue };
            let policy = ir.policy(a.policy);
            let Ok(verdict) = eval_policy(ir, policy, &args, c.now) else {
                continue;
            };
            let roots = policy
                .params
                .iter()
                .map(|q| {
                    (
                        RootKey::Param(q.name.clone()),
                        RootSpec::trusted(q.ty.clone()),
                    )
                })
                .collect();
            let mut p = Problem::new(ir, roots);
            let permitted = permitted_term(&mut p, policy).unwrap();
            let pinned = Concrete {
                roots: policy
                    .params
                    .iter()
                    .zip(&args)
                    .map(|(q, v)| (RootKey::Param(q.name.clone()), v.clone()))
                    .collect(),
                now: c.now,
            };
            let complete = pin_all(&mut p, &pinned);
            p.assert_plain(&permitted);
            out.push(Probe {
                label: format!("{file}: {} / allow #{i} {}", ex.name, policy.name),
                exact: exact_enough(&p, complete),
                problem: p,
                accepted: verdict.permitted,
            });
        }
    }
    out
}

#[test]
fn the_endpoint_examples_agree_with_the_evaluator() {
    let sources: Vec<(&str, Ir)> = ["bookings", "policies", "transitions"]
        .into_iter()
        .map(|n| (n, compile_text(&read(&format!("endpoints/{n}.veld")))))
        .collect();
    let probes: Vec<Probe<'_>> = sources
        .iter()
        .flat_map(|(n, ir)| endpoint_probes(ir, n))
        .collect();
    assert!(probes.len() > 15, "{} probes", probes.len());
    let Some(out) = run("endpoints", &probes) else {
        return;
    };
    eprintln!(
        "soundness: {} require and allow checks on tests/rules/endpoints in one solver call ({:?}): {} exact, {} one-sided",
        out.probes, out.solver, out.exact, out.one_sided
    );
    assert!(out.failures.is_empty(), "{}", out.failures.join("\n"));
}

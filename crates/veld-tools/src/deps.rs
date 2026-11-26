//! The service dependency graph (`consumes`), as text, Graphviz dot or JSON.
//!
//! Cycles between services are legal in Veld; they are reported (text: a trailing note, dot: red
//! edges, JSON: `cycles`) but never an error. Unknown or self references were already rejected
//! or warned about while loading `veld.toml` (`E0702`, `W0701`).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use serde_json::json;
use veld_ir::{Ir, PackageKind};

/// Output format of [`deps`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DepsFormat {
    Text,
    Dot,
    Json,
}

/// The consume graph of the workspace's service packages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DepGraph {
    /// Service names, sorted.
    pub services: Vec<String>,
    /// `service -> consumed services`, sorted; every service has an entry.
    pub consumes: BTreeMap<String, Vec<String>>,
    /// Groups of services that consume each other (strongly connected components of at least
    /// two services, or a service that consumes itself), each sorted, in sorted order.
    pub cycles: Vec<Vec<String>>,
    /// `service -> owners` for the services that declare owners (`[service] owners`); empty when none do.
    pub owners: BTreeMap<String, Vec<String>>,
    /// The feature flags the workspace uses and what each gates (`spec/platform.md` section 11).
    /// Empty without flags, and then absent from the JSON.
    pub flags: BTreeMap<String, veld_ir::FlagUsage>,
}

impl DepGraph {
    /// Services that consume `name`.
    #[must_use]
    pub fn consumed_by(&self, name: &str) -> Vec<&str> {
        self.consumes
            .iter()
            .filter(|(_, c)| c.iter().any(|x| x == name))
            .map(|(k, _)| k.as_str())
            .collect()
    }

    fn in_cycle(&self, a: &str, b: &str) -> bool {
        self.cycles
            .iter()
            .any(|c| c.iter().any(|x| x == a) && c.iter().any(|x| x == b))
    }
}

/// Builds the graph from the IR.
#[must_use]
pub fn graph(ir: &Ir) -> DepGraph {
    let services: BTreeSet<String> = ir
        .packages
        .iter()
        .filter(|p| p.kind == PackageKind::Service)
        .map(|p| p.name.clone())
        .collect();
    let consumes: BTreeMap<String, Vec<String>> = services
        .iter()
        .map(|s| {
            let mut c: Vec<String> = ir
                .consumes
                .get(s)
                .map(|v| {
                    v.iter()
                        .filter(|x| services.contains(*x))
                        .cloned()
                        .collect()
                })
                .unwrap_or_default();
            c.sort();
            c.dedup();
            (s.clone(), c)
        })
        .collect();
    let cycles = cycles(&consumes);
    DepGraph {
        services: services.into_iter().collect(),
        consumes,
        cycles,
        owners: BTreeMap::new(),
        flags: ir.flag_usage(),
    }
}

/// Renders the graph of `ir`.
#[must_use]
pub fn deps(ir: &Ir, format: DepsFormat) -> String {
    render(&graph(ir), format)
}

/// Renders the graph of `ir` with the owners of each service (`spec/workspace.md`, section
/// 11): the JSON node of a service that has owners carries them as `owners`. Without owners
/// the output is the same bytes as [`deps`].
#[must_use]
pub fn deps_with_owners(
    ir: &Ir,
    format: DepsFormat,
    owners: &BTreeMap<String, Vec<String>>,
) -> String {
    let mut g = graph(ir);
    g.owners = owners
        .iter()
        .filter(|(name, list)| !list.is_empty() && g.consumes.contains_key(*name))
        .map(|(name, list)| (name.clone(), list.clone()))
        .collect();
    render(&g, format)
}

/// Renders a graph.
#[must_use]
pub fn render(g: &DepGraph, format: DepsFormat) -> String {
    match format {
        DepsFormat::Text => text(g),
        DepsFormat::Dot => dot(g),
        DepsFormat::Json => json_text(g),
    }
}

fn text(g: &DepGraph) -> String {
    let mut out = String::new();
    for (s, c) in &g.consumes {
        if c.is_empty() {
            let _ = writeln!(out, "{s} -> (none)");
        } else {
            let _ = writeln!(out, "{s} -> {}", c.join(", "));
        }
    }
    for c in &g.cycles {
        let _ = writeln!(out, "cycle: {} (allowed)", c.join(" <-> "));
    }
    out
}

fn dot(g: &DepGraph) -> String {
    let mut out = String::from("digraph services {\n  rankdir=LR;\n  node [shape=box];\n");
    for s in &g.services {
        let _ = writeln!(out, "  {};", quote(s));
    }
    for (s, c) in &g.consumes {
        for t in c {
            let attr = if g.in_cycle(s, t) { " [color=red]" } else { "" };
            let _ = writeln!(out, "  {} -> {}{attr};", quote(s), quote(t));
        }
    }
    out.push_str("}\n");
    out
}

fn quote(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

fn json_text(g: &DepGraph) -> String {
    let services: Vec<_> = g
        .services
        .iter()
        .map(|s| {
            let mut node = json!({
                "name": s,
                "consumes": g.consumes.get(s).cloned().unwrap_or_default(),
                "consumed_by": g.consumed_by(s),
            });
            if let (Some(owners), Some(map)) = (g.owners.get(s), node.as_object_mut()) {
                map.insert("owners".to_string(), json!(owners));
            }
            node
        })
        .collect();
    let mut value = json!({ "services": services, "cycles": g.cycles });
    if let (false, Some(map)) = (g.flags.is_empty(), value.as_object_mut()) {
        map.insert("flags".to_string(), json!(g.flags));
    }
    let mut text = serde_json::to_string_pretty(&value).unwrap_or_default();
    text.push('\n');
    text
}

/// Strongly connected components with a cycle (Tarjan), each sorted, in sorted order.
fn cycles(consumes: &BTreeMap<String, Vec<String>>) -> Vec<Vec<String>> {
    struct St<'a> {
        g: &'a BTreeMap<String, Vec<String>>,
        index: BTreeMap<&'a str, usize>,
        low: BTreeMap<&'a str, usize>,
        stack: Vec<&'a str>,
        on: BTreeSet<&'a str>,
        out: Vec<Vec<String>>,
        next: usize,
    }
    fn visit<'a>(st: &mut St<'a>, v: &'a str) {
        st.index.insert(v, st.next);
        st.low.insert(v, st.next);
        st.next += 1;
        st.stack.push(v);
        st.on.insert(v);
        for w in st.g.get(v).into_iter().flatten().map(String::as_str) {
            if !st.index.contains_key(w) {
                visit(st, w);
                let low = st.low[v].min(st.low[w]);
                st.low.insert(v, low);
            } else if st.on.contains(w) {
                let low = st.low[v].min(st.index[w]);
                st.low.insert(v, low);
            }
        }
        if st.low[v] == st.index[v] {
            let mut comp = Vec::new();
            while let Some(w) = st.stack.pop() {
                st.on.remove(w);
                comp.push(w.to_string());
                if w == v {
                    break;
                }
            }
            let self_loop =
                comp.len() == 1 && st.g.get(v).is_some_and(|c| c.iter().any(|x| x == v));
            if comp.len() > 1 || self_loop {
                comp.sort();
                st.out.push(comp);
            }
        }
    }
    let mut st = St {
        g: consumes,
        index: BTreeMap::new(),
        low: BTreeMap::new(),
        stack: Vec::new(),
        on: BTreeSet::new(),
        out: Vec::new(),
        next: 0,
    };
    for v in consumes.keys() {
        if !st.index.contains_key(v.as_str()) {
            visit(&mut st, v);
        }
    }
    st.out.sort();
    st.out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn g(edges: &[(&str, &[&str])]) -> DepGraph {
        let consumes: BTreeMap<String, Vec<String>> = edges
            .iter()
            .map(|(k, v)| {
                (
                    (*k).to_string(),
                    v.iter().map(|s| (*s).to_string()).collect(),
                )
            })
            .collect();
        DepGraph {
            services: consumes.keys().cloned().collect(),
            cycles: cycles(&consumes),
            owners: BTreeMap::new(),
            flags: BTreeMap::new(),
            consumes,
        }
    }

    #[test]
    fn text_dot_and_json() {
        let g = g(&[
            ("iam", &[]),
            ("orders", &["iam", "payments"]),
            ("payments", &["orders"]),
        ]);
        assert_eq!(
            g.cycles,
            vec![vec!["orders".to_string(), "payments".to_string()]]
        );
        assert_eq!(
            render(&g, DepsFormat::Text),
            "iam -> (none)\norders -> iam, payments\npayments -> orders\ncycle: orders <-> payments (allowed)\n"
        );
        let dot = render(&g, DepsFormat::Dot);
        assert!(dot.contains("\"orders\" -> \"iam\";"), "{dot}");
        assert!(
            dot.contains("\"orders\" -> \"payments\" [color=red];"),
            "{dot}"
        );
        let json = render(&g, DepsFormat::Json);
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["services"][1]["consumed_by"], json!(["payments"]));
        assert_eq!(v["cycles"], json!([["orders", "payments"]]));
    }

    #[test]
    fn owners_appear_only_on_the_nodes_that_have_them() {
        let ir = crate::testutil::shop();
        let plain = deps(&ir, DepsFormat::Json);
        let none = deps_with_owners(&ir, DepsFormat::Json, &BTreeMap::new());
        assert_eq!(plain, none, "no owners: the same bytes as before");
        let owners = BTreeMap::from([
            ("app".to_string(), vec!["team-a".to_string()]),
            ("other".to_string(), vec!["x".to_string()]),
        ]);
        let json = deps_with_owners(&ir, DepsFormat::Json, &owners);
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["services"][0]["owners"], json!(["team-a"]));
        let text = deps_with_owners(&ir, DepsFormat::Text, &owners);
        assert_eq!(text, deps(&ir, DepsFormat::Text));
    }

    #[test]
    fn self_loop_is_a_cycle_and_acyclic_graphs_have_none() {
        assert_eq!(g(&[("a", &["a"])]).cycles, vec![vec!["a".to_string()]]);
        assert!(g(&[("a", &["b"]), ("b", &[])]).cycles.is_empty());
    }

    #[test]
    fn graph_from_ir_lists_service_packages() {
        let ir = crate::testutil::shop();
        let graph = graph(&ir);
        assert_eq!(graph.services, vec!["app".to_string()]);
        assert_eq!(render(&graph, DepsFormat::Text), "app -> (none)\n");
    }
}

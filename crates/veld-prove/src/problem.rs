//! One question as SMT-LIB 2 text, and what is needed to read a solver's answer back.
//!
//! A [`Problem`] is built by the encoder (`encode_*` methods in sibling modules) and rendered by
//! [`Problem::script`]. The text of a question is the same bytes on every run: names are numbered
//! in the order the encoder meets things, and every collection is ordered.

use std::collections::{BTreeMap, BTreeSet};

use veld_ir::Ir;

use crate::smt;
use crate::sorts;
use crate::types::{OpenPart, RootKey, RootSpec, Sort, VarInfo, VarKind};

/// One value asked of the solver after `sat`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValueKey {
    /// A constant, by name.
    Var(String),
    /// The length of a string constant, by name.
    Len(String),
}

impl ValueKey {
    /// The key a model is indexed by.
    #[must_use]
    pub fn id(&self) -> String {
        match self {
            ValueKey::Var(n) => n.clone(),
            ValueKey::Len(n) => format!("slen:{n}"),
        }
    }
}

/// A string or uuid constant of the question.
#[derive(Debug, Clone)]
pub struct Literal {
    pub sort: Sort,
    pub text: String,
    /// The SMT name, without bars.
    pub name: String,
}

/// The SMT-LIB text of one question and the tables to read an answer with.
#[derive(Debug)]
pub struct Problem<'a> {
    /// The contract the expressions being encoded belong to.
    pub(crate) ir: &'a Ir,
    /// The contract whose enums fix the integer model of an enum (the new one in a `diff`).
    pub(crate) canon: &'a Ir,
    /// What `self` stands for: the value of a declaration, or a parameter of an endpoint.
    pub(crate) self_root: RootKey,
    pub(crate) roots: BTreeMap<RootKey, RootSpec>,
    pub(crate) lines: Vec<String>,
    pub(crate) names: Vec<(String, String)>,
    pub(crate) vars: BTreeMap<String, VarInfo>,
    pub(crate) lits: BTreeMap<(bool, String), Literal>,
    pub(crate) open: BTreeSet<OpenPart>,
    /// Variables whose field has a pattern or a format, which a value of the solver cannot meet.
    pub(crate) restricted: BTreeSet<String>,
    counter: usize,
    now: bool,
}

impl<'a> Problem<'a> {
    /// An empty question over the given roots.
    #[must_use]
    pub fn new(ir: &'a Ir, roots: BTreeMap<RootKey, RootSpec>) -> Problem<'a> {
        Problem {
            ir,
            canon: ir,
            self_root: RootKey::SelfValue,
            roots,
            lines: Vec::new(),
            names: Vec::new(),
            vars: BTreeMap::new(),
            lits: BTreeMap::new(),
            open: BTreeSet::new(),
            restricted: BTreeSet::new(),
            counter: 0,
            now: false,
        }
    }

    /// The parts of the fragment this question leaves open.
    #[must_use]
    pub fn open_parts(&self) -> &BTreeSet<OpenPart> {
        &self.open
    }

    /// The solver variables that stand for parts of input values.
    pub fn vars(&self) -> impl Iterator<Item = &VarInfo> {
        self.vars.values()
    }

    /// The string and uuid constants.
    pub fn literals(&self) -> impl Iterator<Item = &Literal> {
        self.lits.values()
    }

    /// The assertions that carry a name, with what each one is: a rule id or a constraint.
    #[must_use]
    pub fn named(&self) -> &[(String, String)] {
        &self.names
    }

    /// The spec of a root.
    #[must_use]
    pub fn root(&self, key: &RootKey) -> Option<&RootSpec> {
        self.roots.get(key)
    }

    /// Switches to the other contract of a `diff`: its expressions, and the roots they read.
    pub(crate) fn switch(&mut self, ir: &'a Ir, roots: BTreeMap<RootKey, RootSpec>) {
        self.ir = ir;
        self.roots = roots;
    }

    /// The variants of the enum `id` of the current contract, in the order of the contract that
    /// fixes the model; a variant the canonical contract lacks has no index there.
    pub(crate) fn enum_wires(&self, id: veld_ir::DeclId) -> Vec<String> {
        let own = self.ir.decl(id);
        let pick = |d: &veld_ir::Decl| -> Option<Vec<String>> {
            if let veld_ir::DeclKind::Enum(e) = &d.kind {
                Some(e.variants.iter().map(|v| v.wire.clone()).collect())
            } else {
                None
            }
        };
        self.canon
            .decls
            .iter()
            .find(|d| d.qualified == own.qualified)
            .and_then(pick)
            .or_else(|| pick(own))
            .unwrap_or_default()
    }

    /// The wire names of the variants when `ty` is an enum.
    pub(crate) fn variants_of(&self, ty: &veld_ir::Type) -> Vec<String> {
        if let veld_ir::Type::Decl(id) = ty {
            self.enum_wires(*id)
        } else {
            Vec::new()
        }
    }

    /// The integer range of the model of `ty`.
    pub(crate) fn domain_of(&self, ty: &veld_ir::Type) -> Option<(i128, i128)> {
        if matches!(ty, veld_ir::Type::Decl(_)) {
            let n = i128::try_from(self.variants_of(ty).len()).ok()?;
            (n > 0).then_some((0, n - 1))
        } else {
            sorts::domain(self.ir, ty)
        }
    }

    pub(crate) fn next(&mut self) -> usize {
        self.counter += 1;
        self.counter
    }

    pub(crate) fn assert_plain(&mut self, term: &str) {
        if term != "true" {
            self.lines.push(format!("(assert {term})"));
        }
    }

    /// Asserts `term` under a name that shows in an unsatisfiable core.
    pub(crate) fn assert_named(&mut self, what: &str, term: &str) {
        let name = format!("n{}", self.next());
        self.lines
            .push(format!("(assert (! {term} :named {name}))"));
        self.names.push((name, what.to_owned()));
    }

    /// `(define-fun dN () Sort body)`; the name is returned. A constant stays a constant.
    pub(crate) fn define(&mut self, sort: Sort, body: &str) -> String {
        let name = format!("d{}", self.next());
        self.lines
            .push(format!("(define-fun {name} () {} {body})", sort.smt()));
        name
    }

    /// Declares a constant and its domain.
    pub(crate) fn declare(&mut self, name: &str, sort: Sort, range: Option<(i128, i128)>) {
        self.lines
            .push(format!("(declare-const {} {})", smt::sym(name), sort.smt()));
        if let Some((lo, hi)) = range {
            let s = smt::sym(name);
            self.assert_plain(&format!(
                "(and (>= {s} {}) (<= {s} {}))",
                smt::int(lo),
                smt::int(hi)
            ));
        }
        if sort == Sort::Str {
            let s = smt::sym(name);
            self.assert_plain(&format!("(>= (slen {s}) 0)"));
        }
    }

    /// An unconstrained value of `sort` for a part left open.
    pub(crate) fn open_value(
        &mut self,
        part: OpenPart,
        sort: Sort,
        range: Option<(i128, i128)>,
    ) -> String {
        self.open.insert(part);
        let name = format!("o{}", self.next());
        self.declare(&name, sort, range);
        smt::sym(&name)
    }

    /// The one reading of the clock of a question: a datetime, left open.
    pub(crate) fn now_value(&mut self) -> String {
        self.open.insert(OpenPart::Now);
        if !self.now {
            self.now = true;
            let (lo, hi) = self
                .domain_of(&veld_ir::Type::Scalar(veld_ir::Scalar::Datetime))
                .unwrap_or(sorts::I64);
            self.declare("now", Sort::Int, Some((lo, hi)));
        }
        smt::sym("now")
    }

    /// The constant of a string or uuid value, with its length when it is a string.
    pub fn literal(&mut self, sort: Sort, text: &str) -> String {
        let key = (sort == Sort::Uuid, text.to_owned());
        if let Some(l) = self.lits.get(&key) {
            return smt::sym(&l.name);
        }
        let name = format!("lit#{}", self.lits.len());
        self.lines.push(format!(
            "(declare-const {} {})",
            smt::sym(&name),
            sort.smt()
        ));
        if sort == Sort::Str {
            let n = text.chars().count();
            self.lines
                .push(format!("(assert (= (slen {}) {n}))", smt::sym(&name)));
        }
        self.lits.insert(
            key,
            Literal {
                sort,
                text: text.to_owned(),
                name: name.clone(),
            },
        );
        smt::sym(&name)
    }

    pub(crate) fn add_var(&mut self, info: VarInfo, range: Option<(i128, i128)>) {
        self.declare(&info.name, info.sort, range);
        self.vars.insert(info.name.clone(), info);
    }

    /// What the solver is asked for after `sat`, in a fixed order.
    #[must_use]
    pub fn value_keys(&self) -> Vec<ValueKey> {
        let mut out = Vec::new();
        for v in self.vars.values() {
            out.push(ValueKey::Var(v.name.clone()));
            if v.sort == Sort::Str && v.kind == VarKind::Value {
                out.push(ValueKey::Len(v.name.clone()));
            }
        }
        out.extend(self.lits.values().map(|l| ValueKey::Var(l.name.clone())));
        if self.now {
            out.push(ValueKey::Var("now".to_owned()));
        }
        out
    }

    /// The terms of [`Problem::value_keys`], as SMT-LIB text.
    #[must_use]
    pub fn value_terms(&self) -> Vec<String> {
        self.value_keys()
            .iter()
            .map(|k| match k {
                ValueKey::Var(n) => smt::sym(n),
                ValueKey::Len(n) => format!("(slen {})", smt::sym(n)),
            })
            .collect()
    }

    /// Distinct constants really are different values.
    fn distinctness(&self) -> Vec<String> {
        let mut out = Vec::new();
        for uuid in [false, true] {
            let names: Vec<String> = self
                .lits
                .iter()
                .filter(|((is_uuid, _), _)| *is_uuid == uuid)
                .map(|(_, l)| smt::sym(&l.name))
                .collect();
            if names.len() > 1 {
                out.push(format!("(assert (distinct {}))", names.join(" ")));
            }
        }
        out
    }

    /// The text of the question as the `index`-th of a batch: everything between `(push 1)` and
    /// `(pop 1)`, the answer commands included.
    #[must_use]
    pub fn script(&self, index: usize) -> String {
        let mut out = String::from("(push 1)\n");
        for l in &self.lines {
            out.push_str(l);
            out.push('\n');
        }
        for l in self.distinctness() {
            out.push_str(&l);
            out.push('\n');
        }
        out.push_str(&smt::echo(&format!("Q{index}")));
        out.push_str("\n(check-sat)\n(get-unsat-core)\n");
        let terms = self.value_terms();
        if !terms.is_empty() {
            out.push_str(&format!("(get-value ({}))\n", terms.join(" ")));
        }
        out.push_str("(pop 1)\n");
        out
    }
}

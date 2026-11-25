//! Paths: the variables that stand for the parts of an input value, and the constraints that
//! hold for them.

use veld_ir::{
    Constraints, Expr, ExprKind, ExprLit, ExprType, Field, Ir, PathRef, Scalar, Step, Type,
};

use crate::problem::Problem;
use crate::smt;
use crate::sorts;
use crate::types::{EncodeError, OpenPart, RootKey, RootSpec, Sort, VarInfo, VarKind, unsupported};

/// The SMT name of a part: `self.a.b`, `self.a.b#has`, `self.a.b#len`.
#[must_use]
pub fn path_name(root: &RootKey, wires: &[String], kind: VarKind) -> String {
    let mut name = root.prefix();
    for w in wires {
        name.push('.');
        name.push_str(w);
    }
    match kind {
        VarKind::Value => name,
        VarKind::Has => format!("{name}#has"),
        VarKind::Len => format!("{name}#len"),
    }
}

/// The field a path ends at, when every step goes through a model.
fn field_at<'i>(ir: &'i Ir, ty: &Type, steps: &[Step]) -> Option<&'i Field> {
    let mut cur = ty.clone();
    let mut found = None;
    for s in steps {
        let Type::Decl(id) = &cur else { return None };
        let f = ir.decl(*id).fields().iter().find(|f| f.name == s.name)?;
        cur = f.ty.clone();
        found = Some(f);
    }
    found
}

impl Problem<'_> {
    fn spec_of(&self, p: &PathRef) -> Result<(RootKey, RootSpec), EncodeError> {
        let Some(mut key) = RootKey::of(&p.root) else {
            return unsupported("a lambda variable outside a comprehension");
        };
        if key == RootKey::SelfValue {
            key = self.self_root.clone();
        }
        match self.roots.get(&key) {
            Some(spec) => Ok((key, spec.clone())),
            None => unsupported(format!(
                "`{}` is not available in this question",
                key.prefix()
            )),
        }
    }

    fn wires(p: &PathRef) -> Vec<String> {
        p.steps.iter().map(|s| s.wire.clone()).collect()
    }

    fn has_var(&mut self, key: &RootKey, wires: &[String]) -> String {
        let name = path_name(key, wires, VarKind::Has);
        if !self.vars.contains_key(&name) {
            let info = VarInfo {
                name: name.clone(),
                sort: Sort::Bool,
                root: key.clone(),
                path: wires.to_vec(),
                kind: VarKind::Has,
                ty: None,
                variants: Vec::new(),
            };
            self.add_var(info, None);
        }
        smt::sym(&name)
    }

    /// The Boolean that says an optional root (a parameter or a fact) is present.
    pub(crate) fn has_root(&mut self, key: &RootKey) -> String {
        self.has_var(key, &[])
    }

    /// `has(p)`: every optional step of the path, and its root, is present.
    pub(crate) fn has_term(&mut self, p: &PathRef) -> Result<String, EncodeError> {
        let (key, spec) = self.spec_of(p)?;
        let wires = Self::wires(p);
        let mut parts = Vec::new();
        if spec.optional {
            parts.push(self.has_var(&key, &[]));
        }
        for (k, step) in p.steps.iter().enumerate() {
            if step.optional {
                parts.push(self.has_var(&key, &wires[..=k]));
            }
        }
        Ok(smt::and_all(&parts))
    }

    /// The value at a path, as a solver variable made on first use.
    pub(crate) fn read_path(&mut self, p: &PathRef, t: &ExprType) -> Result<String, EncodeError> {
        let (key, spec) = self.spec_of(p)?;
        let ExprType::Type(ty) = t else {
            return unsupported("a path of type duration");
        };
        let Some(sort) = sorts::sort_of_type(self.ir, ty) else {
            return unsupported("a value of this type in an expression");
        };
        let wires = Self::wires(p);
        let name = path_name(&key, &wires, VarKind::Value);
        if !self.vars.contains_key(&name) {
            let range = self.domain_of(ty);
            let variants = self.variants_of(ty);
            let info = VarInfo {
                name: name.clone(),
                sort,
                root: key.clone(),
                path: wires,
                kind: VarKind::Value,
                ty: Some(ty.clone()),
                variants,
            };
            self.add_var(info, range);
            if spec.validated {
                let c = self.constraints_of(&spec, &p.steps);
                let label = Self::label(&key, &p.steps);
                self.apply_constraints(&name, ty, &c, &label);
            }
        }
        Ok(smt::sym(&name))
    }

    fn constraints_of(&self, spec: &RootSpec, steps: &[Step]) -> Constraints {
        if steps.is_empty() {
            return spec.constraints.clone();
        }
        field_at(self.ir, &spec.ty, steps)
            .map(|f| f.constraints.clone())
            .unwrap_or_default()
    }

    /// What a part is called in a report: its field, or the parameter itself.
    fn label(root: &RootKey, steps: &[Step]) -> String {
        match (steps.last(), root) {
            (Some(s), _) => format!("`{}`", s.name),
            (None, RootKey::Param(n) | RootKey::Fact(n)) => format!("`{n}`"),
            (None, other) => format!("`{}`", other.prefix()),
        }
    }

    /// Asserts `@min`, `@max` and `@length` of a part, each under a name of its own.
    fn apply_constraints(&mut self, var: &str, ty: &Type, c: &Constraints, label: &str) {
        let v = smt::sym(var);
        let sort = sorts::sort_of_type(self.ir, ty);
        for (word, bound, op) in [("@min", c.min, ">="), ("@max", c.max, "<=")] {
            let (Some(x), Some(sort)) = (bound, sort) else {
                continue;
            };
            if matches!(
                ty,
                Type::Scalar(Scalar::Int | Scalar::Long | Scalar::Decimal | Scalar::Float)
            ) {
                let term = bound_term(&v, sort, op, x);
                self.assert_named(&format!("constraint {word} of {label}"), &term);
            }
        }
        if matches!(ty, Type::Scalar(Scalar::String)) {
            self.length_constraints(&format!("(slen {v})"), c, label);
        }
        if c.pattern.is_some() || c.format.is_some() {
            self.open.insert(OpenPart::PatternFormat);
            self.restricted.insert(var.to_owned());
        }
    }

    fn length_constraints(&mut self, len: &str, c: &Constraints, label: &str) {
        if let Some(n) = c.min_len {
            self.assert_named(
                &format!("constraint @length min of {label}"),
                &format!("(>= {len} {n})"),
            );
        }
        if let Some(n) = c.max_len {
            self.assert_named(
                &format!("constraint @length max of {label}"),
                &format!("(<= {len} {n})"),
            );
        }
    }

    /// `len(arg)`: the length of a string, list, map or `bytes`.
    pub(crate) fn len_term(&mut self, arg: &Expr) -> Result<String, EncodeError> {
        let ExprKind::Path(p) = &arg.e else {
            if let ExprKind::Lit(ExprLit::String(s)) = &arg.e {
                let lit = self.literal(Sort::Str, s);
                return Ok(format!("(slen {lit})"));
            }
            return unsupported("`len` of something that is not a field");
        };
        let ExprType::Type(ty) = &arg.t else {
            return unsupported("`len` of a duration");
        };
        if matches!(ty, Type::Scalar(Scalar::String)) {
            let v = self.read_path(p, &arg.t)?;
            return Ok(format!("(slen {v})"));
        }
        let (key, spec) = self.spec_of(p)?;
        let name = path_name(&key, &Self::wires(p), VarKind::Len);
        if !self.vars.contains_key(&name) {
            let info = VarInfo {
                name: name.clone(),
                sort: Sort::Int,
                root: key.clone(),
                path: Self::wires(p),
                kind: VarKind::Len,
                ty: Some(ty.clone()),
                variants: Vec::new(),
            };
            self.add_var(info, Some((0, sorts::I32.1)));
            if spec.validated {
                let c = self.constraints_of(&spec, &p.steps);
                let label = Self::label(&key, &p.steps);
                self.length_constraints(&smt::sym(&name), &c, &label);
            }
        }
        Ok(smt::sym(&name))
    }
}

/// `v >= x` or `v <= x` for a number `x` written as `f64` in the contract.
fn bound_term(v: &str, sort: Sort, op: &str, x: f64) -> String {
    let whole = x.fract() == 0.0 && x.abs() < 9.0e18;
    match (sort, whole) {
        (Sort::Int, true) => format!("({op} {v} {})", smt::int(x as i128)),
        (Sort::Int, false) => format!("({op} (to_real {v}) {})", smt::real_f64(x)),
        (_, true) => format!("({op} {v} {})", smt::real_int(x as i128)),
        (_, false) => format!("({op} {v} {})", smt::real_f64(x)),
    }
}

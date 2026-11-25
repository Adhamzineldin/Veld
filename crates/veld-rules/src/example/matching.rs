//! Pattern matching of an example's outcome (`spec/rules.md` section 12.1).
//!
//! Every field the pattern names must equal the actual one by the value semantics of section
//! 3.4 (a decimal compares numerically across scales, a datetime as an instant, a uuid in lower
//! case). Fields the pattern does not name are ignored, objects match recursively, and lists
//! match in length and order.

use std::fmt;

use serde_json::Value as Json;
use veld_ir::{Decl, DeclKind, Ir, Scalar, Type};

use crate::ops;
use crate::value::Value;

/// One place where the actual value does not match the pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Difference {
    /// Where, rooted at `$` (`$.capacity`, `$.items[1].id`).
    pub path: String,
    pub expected: String,
    pub actual: String,
}

impl fmt::Display for Difference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "at {}: expected {}, actual {}",
            self.path, self.expected, self.actual
        )
    }
}

/// Compares `actual`, a value of type `ty`, with `pattern`. An empty result means it matches.
#[must_use]
pub fn diff_pattern(ir: &Ir, ty: &Type, pattern: &Json, actual: &Json) -> Vec<Difference> {
    let mut out = Vec::new();
    Matcher { ir }.walk(ty, pattern, actual, "$", &mut out);
    out
}

struct Matcher<'a> {
    ir: &'a Ir,
}

fn show(v: &Json) -> String {
    v.to_string()
}

fn differ(path: &str, expected: &str, actual: &str, out: &mut Vec<Difference>) {
    out.push(Difference {
        path: path.to_owned(),
        expected: expected.to_owned(),
        actual: actual.to_owned(),
    });
}

/// Equality of two JSON values with no type to guide it.
fn structural(pat: &Json, act: &Json, path: &str, out: &mut Vec<Difference>) {
    if pat != act {
        differ(path, &show(pat), &show(act), out);
    }
}

impl Matcher<'_> {
    fn walk(&self, ty: &Type, pat: &Json, act: &Json, path: &str, out: &mut Vec<Difference>) {
        match ty {
            Type::Scalar(Scalar::Json | Scalar::Bytes) => structural(pat, act, path, out),
            Type::Scalar(_) => self.by_value(ty, pat, act, path, out),
            Type::List(item) => self.list(item, pat, act, path, out),
            Type::Map(item) => self.entries(pat, act, path, out, |_| Some(item.as_ref())),
            Type::Page(item) => {
                let list = Type::List(item.clone());
                self.entries(pat, act, path, out, |k| (k == "items").then_some(&list));
            }
            Type::Decl(id) => match self.ir.decls.get(id.index()) {
                Some(decl) => self.decl(decl, pat, act, path, out),
                None => structural(pat, act, path, out),
            },
        }
    }

    fn by_value(&self, ty: &Type, pat: &Json, act: &Json, path: &str, out: &mut Vec<Difference>) {
        let l = Value::from_json(self.ir, ty, pat);
        let r = Value::from_json(self.ir, ty, act);
        match (l, r) {
            (Ok(l), Ok(r)) => match ops::equals(&l, &r) {
                Ok(true) => {}
                Ok(false) | Err(_) => differ(path, &show(pat), &show(act), out),
            },
            (Err(_), _) | (_, Err(_)) => structural(pat, act, path, out),
        }
    }

    fn list(&self, item: &Type, pat: &Json, act: &Json, path: &str, out: &mut Vec<Difference>) {
        let (Some(p), Some(a)) = (pat.as_array(), act.as_array()) else {
            return structural(pat, act, path, out);
        };
        if p.len() != a.len() {
            let (pl, al) = (format!("{} items", p.len()), format!("{} items", a.len()));
            return differ(&format!("{path}.length"), &pl, &al, out);
        }
        for (i, (pv, av)) in p.iter().zip(a).enumerate() {
            self.walk(item, pv, av, &format!("{path}[{i}]"), out);
        }
    }

    /// The members the pattern names, each with the type `ty_of` gives for its key.
    fn entries<'t>(
        &self,
        pat: &Json,
        act: &Json,
        path: &str,
        out: &mut Vec<Difference>,
        ty_of: impl Fn(&str) -> Option<&'t Type>,
    ) {
        let (Some(p), Some(a)) = (pat.as_object(), act.as_object()) else {
            return structural(pat, act, path, out);
        };
        for (key, pv) in p {
            let here = format!("{path}.{key}");
            match (a.get(key), ty_of(key)) {
                (None, _) => differ(&here, &show(pv), "missing", out),
                (Some(av), Some(t)) => self.walk(t, pv, av, &here, out),
                (Some(av), None) => structural(pv, av, &here, out),
            }
        }
    }

    fn decl(&self, decl: &Decl, pat: &Json, act: &Json, path: &str, out: &mut Vec<Difference>) {
        match &decl.kind {
            DeclKind::Enum(_) => self.by_value(&Type::Decl(decl.id), pat, act, path, out),
            DeclKind::Model(_) | DeclKind::Error(_) | DeclKind::Event(_) => {
                let ty_of = |k: &str| decl.fields().iter().find(|f| f.wire == k).map(|f| &f.ty);
                self.entries(pat, act, path, out, ty_of);
            }
            DeclKind::Union(u) => {
                let label = act.get(&u.discriminator).and_then(Json::as_str);
                let arm = u.arms.iter().find(|a| Some(a.label.as_str()) == label);
                match arm {
                    Some(a) => self.walk(&Type::Decl(a.decl), pat, act, path, out),
                    None => structural(pat, act, path, out),
                }
            }
            DeclKind::Const(_) => structural(pat, act, path, out),
        }
    }
}

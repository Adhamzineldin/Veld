//! Places inside a request: a parameter or a fact, then fields and list items below it.

use serde_json::Value as Json;
use veld_ir::{Binding, Constraints, DeclKind, Endpoint, EndpointFact, Param, Scalar, Type};
use veld_rules::Request;

use super::values::{Gen, Skip};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Step {
    /// A field, by wire name.
    Field(String),
    Item(usize),
}

/// Where a value sits in a request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Loc {
    pub root: String,
    /// True for a fact, false for a parameter.
    pub fact: bool,
    pub steps: Vec<Step>,
}

impl Loc {
    pub fn child(&self, step: Step) -> Loc {
        let mut out = self.clone();
        out.steps.push(step);
        out
    }

    /// The place a violation path names (`body`, `body.items[2].window`), below a parameter.
    pub fn parse(path: &str) -> Option<Loc> {
        let mut rest = path;
        let end = rest.find(['.', '[']).unwrap_or(rest.len());
        let root = rest[..end].to_owned();
        rest = &rest[end..];
        let mut steps = Vec::new();
        while !rest.is_empty() {
            if let Some(r) = rest.strip_prefix('.') {
                let end = r.find(['.', '[']).unwrap_or(r.len());
                steps.push(Step::Field(r[..end].to_owned()));
                rest = &r[end..];
            } else {
                let (n, after) = rest.strip_prefix('[')?.split_once(']')?;
                steps.push(Step::Item(n.parse().ok()?));
                rest = after;
            }
        }
        (!root.is_empty()).then_some(Loc {
            root,
            fact: false,
            steps,
        })
    }
}

/// The value at `loc`, when the request has one.
pub(crate) fn get<'r>(req: &'r Request, loc: &Loc) -> Option<&'r Json> {
    let mut cur = if loc.fact {
        req.facts.get(&loc.root)?.as_ref()?
    } else {
        req.params.get(&loc.root)?
    };
    for step in &loc.steps {
        cur = match step {
            Step::Field(w) => cur.get(w)?,
            Step::Item(i) => cur.get(*i)?,
        };
    }
    Some(cur)
}

/// A value for a parameter. One that travels in the path, the query string or a header is never
/// empty, because an empty value there is indistinguishable from a missing one.
pub(crate) fn param_value(g: &mut Gen<'_>, p: &Param) -> Result<Json, Skip> {
    let mut c = p.constraints.clone();
    let flat = !matches!(p.binding, Binding::Body | Binding::QueryObject);
    if flat && matches!(p.ty, Type::Scalar(Scalar::String) | Type::List(_)) {
        c.min_len = Some(c.min_len.unwrap_or(0).max(1));
    }
    g.value(&p.ty, &c, true)
}

pub(crate) fn fact_value(g: &mut Gen<'_>, f: &EndpointFact) -> Result<Json, Skip> {
    g.value(&f.ty, &Constraints::default(), false)
}

/// A value inside a request, with its type and constraints.
pub(crate) struct Slot<'r> {
    pub json: &'r mut Json,
    pub ty: Type,
    pub constraints: Constraints,
}

/// The value at `loc`. Whatever is missing on the way (a parameter that was not sent, an
/// absent fact, an optional field, the first item of an empty list) is generated.
pub(crate) fn slot<'r>(
    g: &mut Gen<'_>,
    ep: &Endpoint,
    req: &'r mut Request,
    loc: &Loc,
) -> Option<Slot<'r>> {
    if loc.fact {
        let f = ep.rules.facts.iter().find(|f| f.name == loc.root)?;
        let entry = req.facts.entry(f.name.clone()).or_insert(None);
        if entry.is_none() {
            *entry = Some(fact_value(g, f).ok()?);
        }
        let json = entry.as_mut()?;
        return descend(g, f.ty.clone(), Constraints::default(), json, &loc.steps);
    }
    let p = ep.params.iter().find(|p| p.name == loc.root && !p.stream)?;
    if !req.params.contains_key(&p.name) {
        let v = param_value(g, p).ok()?;
        req.params.insert(p.name.clone(), v);
    }
    let json = req.params.get_mut(&p.name)?;
    descend(g, p.ty.clone(), p.constraints.clone(), json, &loc.steps)
}

fn descend<'r>(
    g: &mut Gen<'_>,
    ty: Type,
    constraints: Constraints,
    json: &'r mut Json,
    steps: &[Step],
) -> Option<Slot<'r>> {
    let Some((first, rest)) = steps.split_first() else {
        return Some(Slot {
            json,
            ty,
            constraints,
        });
    };
    match (first, &ty) {
        (Step::Item(i), Type::List(inner)) => {
            let items = json.as_array_mut()?;
            if items.is_empty() {
                items.push(g.value(inner, &Constraints::default(), true).ok()?);
            }
            let item = items.get_mut(*i)?;
            descend(g, (**inner).clone(), Constraints::default(), item, rest)
        }
        (Step::Field(wire), Type::Decl(id)) => {
            let decl = g.decl(*id)?;
            if !matches!(decl.kind, DeclKind::Model(_) | DeclKind::Error(_)) {
                return None;
            }
            let field = decl.fields().iter().find(|f| &f.wire == wire)?;
            let obj = json.as_object_mut()?;
            if !obj.contains_key(wire) {
                obj.insert(wire.clone(), g.field_present(field, true).ok()?);
            }
            let child = obj.get_mut(wire)?;
            descend(g, field.ty.clone(), field.constraints.clone(), child, rest)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_violation_path_becomes_a_place() {
        let loc = Loc::parse("body.items[2].window").unwrap();
        assert_eq!(loc.root, "body");
        assert_eq!(
            loc.steps,
            vec![
                Step::Field("items".into()),
                Step::Item(2),
                Step::Field("window".into())
            ]
        );
        assert_eq!(Loc::parse("body").unwrap().steps, vec![]);
        assert!(Loc::parse("").is_none());
    }
}

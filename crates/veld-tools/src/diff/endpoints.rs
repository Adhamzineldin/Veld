//! Endpoint-level comparison: routes, parameters, responses, auth.

use std::collections::BTreeMap;

use veld_ir::{Endpoint, Ir, Param, Service};

use super::decls::diff_docs;
use super::{Change, ChangeKind, Ctx, push_constraint_changes};

pub(super) type Key = (String, String, String);

pub(super) fn index(ir: &Ir) -> BTreeMap<Key, (&Service, &Endpoint)> {
    let mut m = BTreeMap::new();
    for s in &ir.services {
        let pkg = ir.package(s.package).name.clone();
        for e in &s.endpoints {
            m.insert((pkg.clone(), s.name.clone(), e.name.clone()), (s, e));
        }
    }
    m
}

/// The operation signature without its hash: `veld:orders/Orders.getOrder`.
pub(super) fn subject(e: &Endpoint) -> &str {
    e.sig.split('@').next().unwrap_or(&e.sig)
}

/// Path with parameter names erased, as in the operation signature.
fn erased(path: &str) -> String {
    let mut out = String::new();
    let mut depth = 0;
    for c in path.chars() {
        match c {
            '{' => {
                depth += 1;
                out.push_str("{}");
            }
            '}' => depth -= 1,
            c if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out
}

pub(super) fn diff_endpoints(ctx: &Ctx<'_>, out: &mut Vec<Change>) {
    let (old, new) = (index(ctx.old), index(ctx.new));
    for (k, (_, o)) in &old {
        match new.get(k) {
            None => out.push(removed(o, &new)),
            Some((_, n)) => diff_endpoint(ctx, o, n, out),
        }
    }
    for (k, (_, n)) in &new {
        if !old.contains_key(k) {
            out.push(Change::non_breaking(
                ChangeKind::EndpointAdded,
                subject(n),
                format!("endpoint `{}` was added", n.name),
            ));
        }
    }
}

fn removed(o: &Endpoint, new: &BTreeMap<Key, (&Service, &Endpoint)>) -> Change {
    let twin = new
        .values()
        .find(|(_, n)| n.method == o.method && erased(&n.path) == erased(&o.path))
        .map(|(_, n)| n.name.as_str());
    let hint = twin.map_or_else(String::new, |t| {
        format!(" (same route now served by `{t}`)")
    });
    Change::breaking(
        ChangeKind::EndpointRemoved,
        subject(o),
        format!("endpoint `{}` was removed or renamed{hint}", o.name),
    )
}

fn diff_endpoint(ctx: &Ctx<'_>, o: &Endpoint, n: &Endpoint, out: &mut Vec<Change>) {
    let s = subject(o);
    if o.sig != n.sig {
        if o.method != n.method {
            out.push(Change::breaking(
                ChangeKind::MethodChanged,
                s,
                format!(
                    "method changed from {} to {}",
                    o.method.as_str(),
                    n.method.as_str()
                ),
            ));
        }
        if erased(&o.path) != erased(&n.path) {
            out.push(Change::breaking(
                ChangeKind::PathChanged,
                s,
                format!("path changed from `{}` to `{}`", o.path, n.path),
            ));
        }
    }
    diff_params(ctx, s, o, n, out);
    diff_success(ctx, s, o, n, out);
    diff_errors(ctx, s, o, n, out);
    diff_auth(s, o, n, out);
    super::endpoint_rules::diff_endpoint_rules(ctx, s, o, n, out);
    super::conditional::diff_endpoint_conditions(s, o, n, out);
    super::saga::diff_saga(ctx, s, o, n, out);
    super::service_levels::diff_service_levels(s, o, n, out);
    super::audit::diff_audit(s, o, n, out);
    diff_docs(
        s,
        &o.name,
        (&o.docs, &n.docs),
        (&o.deprecated, &n.deprecated),
        out,
    );
}

fn diff_params(ctx: &Ctx<'_>, s: &str, o: &Endpoint, n: &Endpoint, out: &mut Vec<Change>) {
    for p in &o.params {
        match n.params.iter().find(|q| q.name == p.name) {
            None => out.push(Change::breaking(
                ChangeKind::ParamRemoved,
                s,
                format!("parameter `{}` was removed or renamed", p.name),
            )),
            Some(q) => diff_param(ctx, s, (p, q), out),
        }
    }
    for q in n
        .params
        .iter()
        .filter(|q| !o.params.iter().any(|p| p.name == q.name))
    {
        let required = !q.optional;
        out.push(Change::when(
            required,
            ChangeKind::ParamAdded,
            s,
            format!(
                "{} parameter `{}` was added",
                if required { "required" } else { "optional" },
                q.name
            ),
        ));
    }
}

fn diff_param(ctx: &Ctx<'_>, s: &str, pair: (&Param, &Param), out: &mut Vec<Change>) {
    let (p, q) = pair;
    let name = &p.name;
    if p.binding != q.binding || p.header != q.header {
        out.push(Change::breaking(
            ChangeKind::ParamBindingChanged,
            s,
            format!("binding of parameter `{name}` changed"),
        ));
    }
    if ctx.type_key(true, &p.ty) != ctx.type_key(false, &q.ty) || p.stream != q.stream {
        out.push(Change::breaking(
            ChangeKind::ParamTypeChanged,
            s,
            format!("type of parameter `{name}` changed"),
        ));
    }
    if p.optional && !q.optional {
        out.push(Change::breaking(
            ChangeKind::ParamBecameRequired,
            s,
            format!("parameter `{name}` became required"),
        ));
    } else if !p.optional && q.optional {
        out.push(Change::non_breaking(
            ChangeKind::ParamBecameOptional,
            s,
            format!("parameter `{name}` became optional"),
        ));
    }
    push_constraint_changes(
        out,
        s,
        &format!("parameter `{name}`"),
        &p.constraints,
        &q.constraints,
    );
}

fn diff_success(ctx: &Ctx<'_>, s: &str, o: &Endpoint, n: &Endpoint, out: &mut Vec<Change>) {
    let key = |in_old: bool, e: &Endpoint| e.success.ty.as_ref().map(|t| ctx.type_key(in_old, t));
    if o.success.status != n.success.status {
        out.push(Change::breaking(
            ChangeKind::SuccessChanged,
            s,
            format!(
                "success status changed from {} to {}",
                o.success.status, n.success.status
            ),
        ));
    }
    if key(true, o) != key(false, n) || o.success.stream != n.success.stream {
        out.push(Change::breaking(
            ChangeKind::SuccessChanged,
            s,
            "success type changed",
        ));
    }
}

fn error_set(ir: &Ir, e: &Endpoint) -> Vec<(String, u16)> {
    e.errors
        .iter()
        .map(|r| (ir.decl(r.decl).qualified.clone(), r.status))
        .collect()
}

fn diff_errors(ctx: &Ctx<'_>, s: &str, o: &Endpoint, n: &Endpoint, out: &mut Vec<Change>) {
    let (old, new) = (error_set(ctx.old, o), error_set(ctx.new, n));
    let own = |q: &str| !super::conditional::implied_error(q, o, n);
    for (q, status) in old.iter().filter(|e| !new.contains(e) && own(&e.0)) {
        out.push(Change::breaking(
            ChangeKind::ErrorRemoved,
            s,
            format!("declared error `{q}` ({status}) was removed"),
        ));
    }
    for (q, status) in new.iter().filter(|e| !old.contains(e) && own(&e.0)) {
        out.push(Change::breaking(
            ChangeKind::ErrorAdded,
            s,
            format!("error `{q}` ({status}) was added to the clients' error union"),
        ));
    }
}

fn diff_auth(s: &str, o: &Endpoint, n: &Endpoint, out: &mut Vec<Change>) {
    use veld_ir::AuthKind::None as NoAuth;
    if o.auth == n.auth {
        return;
    }
    if o.auth.kind == NoAuth {
        out.push(Change::breaking(
            ChangeKind::AuthAdded,
            s,
            "authentication is now required",
        ));
    } else if n.auth.kind == NoAuth {
        out.push(Change::non_breaking(
            ChangeKind::AuthRemoved,
            s,
            "authentication is no longer required",
        ));
    } else {
        out.push(Change::breaking(
            ChangeKind::AuthChanged,
            s,
            "authentication scheme changed",
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::erased;

    #[test]
    fn erased_replaces_parameter_names() {
        assert_eq!(erased("/a/{x}/b/{yy}"), "/a/{}/b/{}");
        assert_eq!(erased("/plain"), "/plain");
    }
}

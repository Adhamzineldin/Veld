//! The fields of an event, judged for both sides of the stream (`spec/events.md` section 16): a
//! consumer generated from the old contract reads messages the new publisher writes, and a
//! consumer generated from the new contract reads messages the old publisher wrote.
//!
//! | Change | Result |
//! |---|---|
//! | an optional field, or one with a default, is added | compatible |
//! | a required field without a default is added | breaking: a new consumer cannot read old messages |
//! | a required field is removed, or a field's type changes | breaking: an old consumer rejects new messages |
//! | an optional field (or one with a default) is removed | possibly breaking |
//! | a field becomes required | breaking: a new consumer cannot read old messages |
//! | a field stops being required | breaking: an old consumer rejects the messages that leave it out |
//! | a constraint is tightened | breaking: a new consumer rejects old messages |
//! | a constraint is loosened | possibly breaking: an old consumer rejects the new values |
//! | docs, deprecation | compatible |
//!
//! A rule is judged by its digest in `events.rs`.

use std::collections::BTreeMap;

use veld_ir::{Decl, Field};

use super::decls::diff_docs;
use super::{Change, ChangeKind, Ctx, constraint_changes};

fn wire_map(fields: &[Field]) -> BTreeMap<&str, &Field> {
    fields.iter().map(|f| (f.wire.as_str(), f)).collect()
}

/// A field a message must carry: not optional and without a default (a consumer applies a
/// default, `spec/events.md` section 8, step 3).
fn required(f: &Field) -> bool {
    !f.optional && f.default.is_none()
}

pub(super) fn diff_event_fields(ctx: &Ctx<'_>, o: &Decl, n: &Decl, out: &mut Vec<Change>) {
    let q = &o.qualified;
    let (old, new) = (wire_map(o.fields()), wire_map(n.fields()));
    for (w, of) in &old {
        match new.get(w) {
            Some(nf) => diff_field(ctx, q, (of, nf), out),
            None => out.push(removed(q, w, of)),
        }
    }
    for (w, nf) in &new {
        if !old.contains_key(w) {
            out.push(added(q, w, nf));
        }
    }
}

fn removed(q: &str, w: &str, f: &Field) -> Change {
    if required(f) {
        Change::breaking(
            ChangeKind::FieldRemoved,
            q,
            format!(
                "required field `{w}` was removed from the event: an old consumer rejects the new messages"
            ),
        )
    } else {
        Change::possibly_breaking(
            ChangeKind::FieldRemoved,
            q,
            format!(
                "optional field `{w}` was removed from the event: an old consumer that reads it sees it absent"
            ),
        )
    }
}

fn added(q: &str, w: &str, f: &Field) -> Change {
    if required(f) {
        Change::breaking(
            ChangeKind::RequiredFieldAdded,
            q,
            format!(
                "required field `{w}` was added to the event: a new consumer cannot read the old messages"
            ),
        )
    } else {
        Change::non_breaking(
            ChangeKind::FieldAdded,
            q,
            format!("field `{w}` was added to the event; it is optional or has a default"),
        )
    }
}

fn diff_field(ctx: &Ctx<'_>, q: &str, pair: (&Field, &Field), out: &mut Vec<Change>) {
    let (o, n) = pair;
    let w = &o.wire;
    if ctx.type_key(true, &o.ty) != ctx.type_key(false, &n.ty) {
        out.push(Change::breaking(
            ChangeKind::FieldTypeChanged,
            q,
            format!("type of field `{w}` changed: an old consumer rejects the new messages and a new one the old"),
        ));
    }
    if !required(o) && required(n) {
        out.push(Change::breaking(
            ChangeKind::FieldBecameRequired,
            q,
            format!("field `{w}` became required: a new consumer cannot read the old messages that leave it out"),
        ));
    }
    if required(o) && !required(n) {
        out.push(Change::breaking(
            ChangeKind::FieldBecameOptional,
            q,
            format!("field `{w}` is no longer required: an old consumer rejects the messages that leave it out"),
        ));
    }
    for (name, tightened) in constraint_changes(&o.constraints, &n.constraints) {
        out.push(constraint(q, w, name, tightened));
    }
    diff_docs(
        q,
        w,
        (&o.docs, &n.docs),
        (&o.deprecated, &n.deprecated),
        out,
    );
}

fn constraint(q: &str, w: &str, name: &str, tightened: bool) -> Change {
    if tightened {
        Change::breaking(
            ChangeKind::ConstraintTightened,
            q,
            format!(
                "field `{w}`: {name} constraint tightened: a new consumer rejects the old messages that break it"
            ),
        )
    } else {
        Change::possibly_breaking(
            ChangeKind::ConstraintLoosened,
            q,
            format!(
                "field `{w}`: {name} constraint loosened: an old consumer rejects the new values it no longer allows"
            ),
        )
    }
}

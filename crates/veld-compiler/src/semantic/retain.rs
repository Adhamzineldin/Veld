//! `@retain` checks that need the fields of the model (`spec/platform.md` section 23): the
//! `since` field exists and is a `datetime` or a `date`, and `then: anonymize` has a `@pii` field
//! to work on. The shape of the arguments is checked with the annotation (`E1310`).

use veld_ir::retention::RetainThen;
use veld_ir::{Scalar, Type};

use super::DeclInfo;
use crate::diagnostics::Diagnostic;

pub(super) fn check(d: &DeclInfo<'_>, diags: &mut Vec<Diagnostic>) {
    let Some(r) = &d.res.retain else {
        return;
    };
    match d.res.fields.iter().find(|f| f.name == r.since) {
        None => diags.push(
            Diagnostic::error(
                "E1310",
                format!("`@retain`: `{}` has no field `{}`", d.name, r.since),
                r.since_span,
            )
            .did_you_mean(
                &r.since,
                d.res.fields.iter().map(|f| f.name.as_str()),
                Some("`since:` names the `datetime` or `date` field the time counts from".into()),
            ),
        ),
        Some(f) if !matches!(f.ty, Type::Scalar(Scalar::Datetime | Scalar::Date)) => {
            diags.push(
                Diagnostic::error(
                    "E1310",
                    format!(
                        "`@retain`: `since: {}` must be a `datetime` or a `date` field",
                        r.since
                    ),
                    r.since_span,
                )
                .with_label(f.span, "this field has another type"),
            );
        }
        Some(_) => {}
    }
    if r.then == RetainThen::Anonymize && !d.res.fields.iter().any(|f| f.pii) {
        diags.push(
            Diagnostic::error(
                "E1310",
                format!(
                    "`@retain`: `then: anonymize` needs a `@pii` field in `{}`, and it has none",
                    d.name
                ),
                r.span,
            )
            .with_help("mark the fields that hold personal data with `@pii`, or use `then: delete`"),
        );
    }
}

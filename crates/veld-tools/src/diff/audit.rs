//! `@audited` in `veld diff` (`spec/platform.md` section 4).
//!
//! The audit log changes nothing on the wire, so a caller built against the old contract is not
//! affected by it. Whoever reads the log is:
//!
//! | Change | Judged |
//! |---|---|
//! | an endpoint becomes audited | compatible: the server records more |
//! | an endpoint stops being audited | possibly breaking: an operator may rely on the record |

use veld_ir::Endpoint;

use super::{Change, ChangeKind};

pub(super) fn diff_audit(s: &str, o: &Endpoint, n: &Endpoint, out: &mut Vec<Change>) {
    match (o.audited, n.audited) {
        (false, true) => out.push(Change::non_breaking(
            ChangeKind::AuditAdded,
            s,
            "the endpoint is now audited: the server records every request".to_owned(),
        )),
        (true, false) => out.push(Change::possibly_breaking(
            ChangeKind::AuditRemoved,
            s,
            "the endpoint is no longer audited: the record of its requests stops".to_owned(),
        )),
        (false, false) | (true, true) => {}
    }
}

//! Retention (`spec/platform.md` section 23): `@retain` in the IR, and the plan a server embeds.
//!
//! Veld deletes nothing. A server target with retained models gets `retentionPlan`, data that
//! says for each model what is due and when. Every generator builds it from [`Ir::retention_plan`],
//! so they all list the same entries in the same order, and "does this server use retention" is the
//! same question as "is the plan empty".

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{DeclId, DeclKind, Ir, ServiceId};

/// What happens to a model's data when its time is up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetainThen {
    /// The values are deleted.
    Delete,
    /// The `@pii` fields of the values are anonymized; the rest is kept.
    Anonymize,
}

impl RetainThen {
    /// `delete` or `anonymize`, as written in the contract.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            RetainThen::Delete => "delete",
            RetainThen::Anonymize => "anonymize",
        }
    }
}

/// `@retain(days(30), since: checkedOutAt, then: anonymize)` as the IR keeps it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Retain {
    /// How long a value is kept after `since`, in milliseconds (a whole number of days). A string
    /// in the JSON, like every `long`.
    #[serde(with = "crate::rules::long_text")]
    pub after_ms: i64,
    /// The wire name of the `datetime` or `date` field the time counts from.
    pub since: String,
    pub then: RetainThen,
}

/// One line of the plan a server embeds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetentionEntry {
    pub decl: DeclId,
    /// The qualified name of the model (`bookings:bookings.Guest`).
    pub model: String,
    pub after_ms: i64,
    /// The wire name of the field the time counts from.
    pub since: String,
    pub then: RetainThen,
    /// For `anonymize`: the wire names of the `@pii` fields of the model itself (inherited ones
    /// included), in field order. A field that is itself a model is listed once, whole. Empty for
    /// `delete`.
    pub fields: Vec<String>,
}

impl Ir {
    /// The declarations of a member that a server for `services` is concerned with: what its
    /// endpoints and events reach, and what its packages declare.
    #[must_use]
    pub fn member_decls(&self, services: &[ServiceId]) -> BTreeSet<DeclId> {
        let mut out: BTreeSet<DeclId> = self.reachable_decls(services).into_iter().collect();
        out.extend(self.events_of(services));
        out.extend(self.event_reachable_decls(services));
        let packages: BTreeSet<_> = services.iter().map(|s| self.service(*s).package).collect();
        out.extend(
            self.decls
                .iter()
                .filter(|d| packages.contains(&d.package))
                .map(|d| d.id),
        );
        out
    }

    /// The retention plan of a server that serves `services`: one entry per model with `@retain`
    /// in [`Ir::member_decls`], sorted by qualified name. Empty when there is none, and then the
    /// server uses no retention and generates nothing for it.
    #[must_use]
    pub fn retention_plan(&self, services: &[ServiceId]) -> Vec<RetentionEntry> {
        let mut plan: Vec<RetentionEntry> = self
            .member_decls(services)
            .into_iter()
            .filter_map(|id| {
                let d = self.decl(id);
                let DeclKind::Model(m) = &d.kind else {
                    return None;
                };
                let r = m.retain.as_ref()?;
                let fields = match r.then {
                    RetainThen::Delete => Vec::new(),
                    RetainThen::Anonymize => m
                        .fields
                        .iter()
                        .filter(|f| f.pii)
                        .map(|f| f.wire.clone())
                        .collect(),
                };
                Some(RetentionEntry {
                    decl: id,
                    model: d.qualified.clone(),
                    after_ms: r.after_ms,
                    since: r.since.clone(),
                    then: r.then,
                    fields,
                })
            })
            .collect();
        plan.sort_by(|a, b| a.model.cmp(&b.model));
        plan
    }
}

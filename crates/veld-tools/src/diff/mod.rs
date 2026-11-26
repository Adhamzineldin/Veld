//! Breaking-change detection between two IRs.
//!
//! Declarations are matched by qualified name and endpoints by (package,
//! service, endpoint name). The declaration `sig` is a fast path: equal
//! signatures mean the wire shape is unchanged, so only metadata (constraints,
//! defaults, docs, deprecations) is compared.
//!
//! "Request-reachable" and "response-reachable" are computed on the old IR,
//! because the clients that can break are the ones built against it. A type the
//! old IR never uses falls back to the new IR.

mod changed;
mod conditional;
mod decls;
mod delivery;
mod endpoint_rules;
mod endpoints;
mod events;
pub(crate) mod expr_text;
mod rules;
mod saga;
mod usage;

pub use changed::{ChangedRule, RuleSite, changed_rules};
use serde::Serialize;
use veld_ir::{Constraints, Ir, Type};

/// How a change affects existing clients.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Breaking,
    /// The tool cannot tell: a rule expression changed and no solver says whether it is tighter
    /// or looser, or something other than an addition changed in an event, a channel or a
    /// subscription (`spec/events.md` section 16 is not built yet).
    PossiblyBreaking,
    NonBreaking,
}

/// What changed. The severity can still depend on where a type is used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    DeclRemoved,
    DeclAdded,
    DeclKindChanged,
    FieldRemoved,
    FieldAdded,
    RequiredFieldAdded,
    FieldTypeChanged,
    FieldBecameRequired,
    FieldBecameOptional,
    EnumVariantRemoved,
    EnumVariantAdded,
    UnionArmRemoved,
    UnionArmAdded,
    UnionArmChanged,
    DiscriminatorChanged,
    ConstraintTightened,
    ConstraintLoosened,
    ConstChanged,
    DocsChanged,
    Deprecated,
    Undeprecated,
    EndpointRemoved,
    EndpointAdded,
    MethodChanged,
    PathChanged,
    ParamRemoved,
    ParamAdded,
    ParamBindingChanged,
    ParamTypeChanged,
    ParamBecameRequired,
    ParamBecameOptional,
    SuccessChanged,
    ErrorRemoved,
    ErrorAdded,
    AuthAdded,
    AuthChanged,
    AuthRemoved,
    InvariantAdded,
    InvariantRemoved,
    InvariantChanged,
    RuleMessageChanged,
    RequireAdded,
    RequireRemoved,
    RequireChanged,
    RequireErrorChanged,
    EnsureAdded,
    EnsureRemoved,
    EnsureChanged,
    FactChanged,
    FlowRemoved,
    FlowAdded,
    TransitionChanged,
    EventAdded,
    EventRemoved,
    EventChanged,
    ChannelAdded,
    ChannelRemoved,
    ChannelChanged,
    ChannelRetryAdded,
    ChannelRetryRemoved,
    ChannelRetryChanged,
    ChannelDeadLetterAdded,
    ChannelDeadLetterRemoved,
    ChannelOrderedAdded,
    ChannelOrderedRemoved,
    PublishesAdded,
    PublishesRemoved,
    SubscriptionAdded,
    SubscriptionRemoved,
    SubscriptionChanged,
    VersionAdded,
    VersionRemoved,
    VersionFieldChanged,
    GuardAdded,
    GuardRemoved,
    GuardChanged,
    IdempotencyAdded,
    IdempotencyRemoved,
    IdempotencyHeaderChanged,
    IdempotencyRequiredChanged,
    DerivedChanged,
    FieldBecameDerived,
    FieldNoLongerDerived,
    SagaAdded,
    SagaRemoved,
    SagaChanged,
}

/// One difference between the old and the new contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Change {
    pub kind: ChangeKind,
    pub severity: Severity,
    /// Qualified declaration name, or the operation signature without its hash
    /// (`veld:orders/Orders.getOrder`).
    pub subject: String,
    pub message: String,
}

impl Change {
    pub(crate) fn new(
        kind: ChangeKind,
        severity: Severity,
        subject: &str,
        message: impl Into<String>,
    ) -> Change {
        Change {
            kind,
            severity,
            subject: subject.to_string(),
            message: message.into(),
        }
    }

    pub(crate) fn breaking(kind: ChangeKind, subject: &str, message: impl Into<String>) -> Change {
        Change::new(kind, Severity::Breaking, subject, message)
    }

    pub(crate) fn possibly_breaking(
        kind: ChangeKind,
        subject: &str,
        message: impl Into<String>,
    ) -> Change {
        Change::new(kind, Severity::PossiblyBreaking, subject, message)
    }

    pub(crate) fn non_breaking(
        kind: ChangeKind,
        subject: &str,
        message: impl Into<String>,
    ) -> Change {
        Change::new(kind, Severity::NonBreaking, subject, message)
    }

    /// Breaking when `breaking` holds, otherwise non-breaking.
    pub(crate) fn when(
        breaking: bool,
        kind: ChangeKind,
        subject: &str,
        message: impl Into<String>,
    ) -> Change {
        if breaking {
            Change::breaking(kind, subject, message)
        } else {
            Change::non_breaking(kind, subject, message)
        }
    }
}

/// True when any change is breaking or possibly breaking. A rule whose expression changed fails
/// a CI check even though the tool cannot prove the direction: passing a tightened rule silently
/// is the worse error.
#[must_use]
pub fn has_breaking(changes: &[Change]) -> bool {
    changes.iter().any(|c| c.severity.fails_check())
}

impl Severity {
    /// Whether this severity fails `--strict` and blocks `generate`.
    #[must_use]
    pub fn fails_check(self) -> bool {
        match self {
            Severity::Breaking | Severity::PossiblyBreaking => true,
            Severity::NonBreaking => false,
        }
    }
}

/// Both IRs, for comparisons that need to resolve declaration ids.
pub(crate) struct Ctx<'a> {
    pub old: &'a Ir,
    pub new: &'a Ir,
}

impl Ctx<'_> {
    /// Structural key of a type with declarations by qualified name, so ids
    /// from different IRs compare correctly.
    pub fn type_key(&self, in_old: bool, t: &Type) -> String {
        let ir = if in_old { self.old } else { self.new };
        type_key(ir, t)
    }
}

pub(crate) fn type_key(ir: &Ir, t: &Type) -> String {
    match t {
        Type::Scalar(s) => s.name().to_string(),
        Type::List(i) => format!("[{}]", type_key(ir, i)),
        Type::Map(i) => format!("map<{}>", type_key(ir, i)),
        Type::Page(i) => format!("page<{}>", type_key(ir, i)),
        Type::Decl(id) => format!("ref:{}", ir.decl(*id).qualified),
    }
}

/// Which side of a bound moved: `Some(true)` tightened, `Some(false)` loosened.
fn bound(old: Option<f64>, new: Option<f64>, lower: bool) -> Option<bool> {
    match (old, new) {
        (None, None) => None,
        (None, Some(_)) => Some(true),
        (Some(_), None) => Some(false),
        (Some(a), Some(b)) if (a - b).abs() < f64::EPSILON => None,
        (Some(a), Some(b)) => Some(if lower { b > a } else { b < a }),
    }
}

fn text_rule<T: PartialEq>(old: &Option<T>, new: &Option<T>) -> Option<bool> {
    match (old, new) {
        (None, None) => None,
        (None, Some(_)) => Some(true),
        (Some(_), None) => Some(false),
        (Some(a), Some(b)) => (a != b).then_some(true),
    }
}

/// Constraint changes as (name, tightened).
pub(crate) fn constraint_changes(
    old: &Constraints,
    new: &Constraints,
) -> Vec<(&'static str, bool)> {
    #[allow(clippy::cast_precision_loss)]
    let len = |v: Option<u64>| v.map(|n| n as f64);
    let checks = [
        ("min", bound(old.min, new.min, true)),
        ("max", bound(old.max, new.max, false)),
        (
            "min length",
            bound(len(old.min_len), len(new.min_len), true),
        ),
        (
            "max length",
            bound(len(old.max_len), len(new.max_len), false),
        ),
        ("pattern", text_rule(&old.pattern, &new.pattern)),
        ("format", text_rule(&old.format, &new.format)),
    ];
    checks
        .into_iter()
        .filter_map(|(name, t)| t.map(|t| (name, t)))
        .collect()
}

/// Reports constraint changes of one subject element.
pub(crate) fn push_constraint_changes(
    out: &mut Vec<Change>,
    subject: &str,
    what: &str,
    old: &Constraints,
    new: &Constraints,
) {
    for (name, tightened) in constraint_changes(old, new) {
        let (kind, verb) = if tightened {
            (ChangeKind::ConstraintTightened, "tightened")
        } else {
            (ChangeKind::ConstraintLoosened, "loosened")
        };
        out.push(Change::when(
            tightened,
            kind,
            subject,
            format!("{what}: {name} constraint {verb}"),
        ));
    }
}

/// Compares two IRs and lists every change, breaking or not, in a stable order.
#[must_use]
pub fn diff(old: &Ir, new: &Ir) -> Vec<Change> {
    let ctx = Ctx { old, new };
    let usage = usage::Usage::of(old, new);
    let mut out = Vec::new();
    decls::diff_decls(&ctx, &usage, &mut out);
    conditional::diff_version_fields(&ctx, &mut out);
    rules::diff_decl_rules(&ctx, &usage, &mut out);
    endpoints::diff_endpoints(&ctx, &mut out);
    events::diff_events(&ctx, &mut out);
    delivery::diff_delivery(&ctx, &mut out);
    out
}

#[cfg(test)]
mod changed_tests;
#[cfg(test)]
mod conditional_tests;
#[cfg(test)]
mod delivery_tests;
#[cfg(test)]
mod derived_tests;
#[cfg(test)]
mod event_tests;
#[cfg(test)]
mod rollout_tests;
#[cfg(test)]
mod rule_tests;
#[cfg(test)]
mod saga_tests;
#[cfg(test)]
mod tests;

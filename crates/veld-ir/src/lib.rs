//! Veld IR v1 — see `spec/ir.md`.
//!
//! The IR is immutable and fully resolved: no imports, inheritance already
//! flattened into `fields`, every implicit (status codes, `ValidationError`,
//! bindings, wire names) made explicit. Generators never need to re-validate.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;

pub mod endpoint_rules;
pub mod flags;
pub use flags::FlagUsage;
pub mod exposure;
pub mod message_keys;
pub mod retention;
pub mod rules;
pub mod saga;
pub mod schema;
pub mod suggest;

pub use endpoint_rules::{
    ElseError, ElseField, EndpointAllow, EndpointEnsure, EndpointFact, EndpointRequire,
    EndpointRules, EndpointTransition, EndpointVersion, EventExpect, Example, ExampleArg,
    ExampleErrorOutcome, ExampleEvent, ExampleField, ExampleGiven, ExampleIfMatch, ExampleKey,
    ExampleOutcome, ExampleRef, ExampleValue, ExpectedCall, KeyStore, Policy, PolicyClause,
    PolicyEffect, PolicyParam, ServiceFact, ServicePrincipal,
};
pub use rules::{
    Always, BinExpr, Builtin, CallExpr, EnumLit, Expr, ExprKind, ExprLit, ExprType, Op, PathRef,
    QuantExpr, Quantifier, Rollout, Root, Rule, Step,
};
pub use saga::{
    ExampleSaga, Saga, SagaAnswer, SagaArg, SagaCall, SagaExpect, SagaField, SagaGiven, SagaStep,
    SagaValue,
};

pub const IR_VERSION: &str = "1.1";

macro_rules! id_type {
    ($name:ident) => {
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(pub u32);
        impl $name {
            #[must_use]
            pub fn index(self) -> usize {
                self.0 as usize
            }
        }
    };
}
id_type!(PackageId);
id_type!(DeclId);
id_type!(ServiceId);
id_type!(PolicyId);
id_type!(ChannelId);

/// A whole compiled workspace.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Ir {
    pub ir_version: String,
    pub packages: Vec<Package>,
    pub decls: Vec<Decl>,
    pub services: Vec<Service>,
    /// service name -> consumed service names (sorted). May contain cycles.
    pub consumes: std::collections::BTreeMap<String, Vec<String>>,
    /// Examples of the workspace (`spec/rules.md` section 12). Omitted from the JSON when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub examples: Vec<Example>,
    /// Policies of the workspace, sorted by qualified name (`spec/rules.md` section 11.5).
    /// Omitted from the JSON when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub policies: Vec<Policy>,
    /// Channels of the workspace, sorted by qualified name (`spec/events.md` section 13).
    /// Omitted from the JSON when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub channels: Vec<Channel>,
}

impl Ir {
    #[must_use]
    pub fn channel(&self, id: ChannelId) -> &Channel {
        &self.channels[id.index()]
    }

    #[must_use]
    pub fn decl(&self, id: DeclId) -> &Decl {
        &self.decls[id.index()]
    }
    #[must_use]
    pub fn policy(&self, id: PolicyId) -> &Policy {
        &self.policies[id.index()]
    }
    #[must_use]
    pub fn service(&self, id: ServiceId) -> &Service {
        &self.services[id.index()]
    }
    #[must_use]
    pub fn package(&self, id: PackageId) -> &Package {
        &self.packages[id.index()]
    }
    #[must_use]
    pub fn service_by_name(&self, name: &str) -> Option<&Service> {
        self.services
            .iter()
            .find(|s| self.package(s.package).name == name || s.name == name)
    }
    /// Services belonging to a package name (a workspace service package may
    /// declare several `service` blocks).
    pub fn services_of_package<'a>(
        &'a self,
        pkg: &'a str,
    ) -> impl Iterator<Item = &'a Service> + 'a {
        self.services
            .iter()
            .filter(move |s| self.package(s.package).name == pkg)
    }

    /// Every declaration reachable from the given services' endpoints, sorted
    /// by id (which is canonical-name order). Includes parents, union arms,
    /// errors and field types, transitively.
    #[must_use]
    pub fn reachable_decls(&self, services: &[ServiceId]) -> Vec<DeclId> {
        let mut stack = Vec::new();
        for sid in services {
            for ep in &self.service(*sid).endpoints {
                for p in &ep.params {
                    p.ty.collect_decls(&mut stack);
                }
                if let Some(t) = &ep.success.ty {
                    t.collect_decls(&mut stack);
                }
                for e in &ep.errors {
                    stack.push(e.decl);
                }
            }
        }
        self.close_over(stack)
    }

    /// Every declaration reachable from `stack`, itself included, sorted by id.
    fn close_over(&self, mut stack: Vec<DeclId>) -> Vec<DeclId> {
        let mut seen = BTreeSet::new();
        while let Some(id) = stack.pop() {
            if !seen.insert(id) {
                continue;
            }
            self.decl(id).collect_refs(&mut stack);
        }
        seen.into_iter().collect()
    }

    /// The channels the given services publish to or subscribe to, in id order, once each.
    #[must_use]
    pub fn event_channels(&self, services: &[ServiceId]) -> Vec<ChannelId> {
        let all: BTreeSet<ChannelId> = services
            .iter()
            .flat_map(|s| self.service(*s).channels_used())
            .collect();
        all.into_iter().collect()
    }

    /// The events of the given services: every event carried by a channel one of them publishes
    /// to or subscribes to (not only the ones a subscription handles), sorted by id, once each.
    #[must_use]
    pub fn events_of(&self, services: &[ServiceId]) -> Vec<DeclId> {
        let all: BTreeSet<DeclId> = self
            .event_channels(services)
            .into_iter()
            .flat_map(|c| self.channel(c).events.iter().copied())
            .collect();
        all.into_iter().collect()
    }

    /// The declarations the events of [`Ir::events_of`] reach through field types, parents and
    /// union arms, to any depth, sorted by id. The events themselves are not in the list.
    #[must_use]
    pub fn event_reachable_decls(&self, services: &[ServiceId]) -> Vec<DeclId> {
        let events = self.events_of(services);
        let mut stack = Vec::new();
        for id in &events {
            self.decl(*id).collect_refs(&mut stack);
        }
        let mut reached = self.close_over(stack);
        reached.retain(|id| !events.contains(id));
        reached
    }

    /// The declarations that exist only because of events: reached by an event of the services
    /// and by no endpoint of them. Sorted by id.
    #[must_use]
    pub fn event_only_decls(&self, services: &[ServiceId]) -> Vec<DeclId> {
        let by_endpoints: BTreeSet<DeclId> = self.reachable_decls(services).into_iter().collect();
        let mut only = self.event_reachable_decls(services);
        only.retain(|id| !by_endpoints.contains(id));
        only
    }

    /// Where the first audited invariant of the events of the services stands, in source order:
    /// a rule of an event or of a declaration an event reaches. `None` when there is none.
    #[must_use]
    pub fn first_event_audited_origin(&self, services: &[ServiceId]) -> Option<&Origin> {
        let mut ids = self.events_of(services);
        ids.extend(self.event_reachable_decls(services));
        ids.into_iter()
            .flat_map(|id| self.decl(id).rules().iter())
            .filter(|r| r.is_audited())
            .map(|r| &r.origin)
            .min_by_key(|o| (o.file.as_str(), o.line, o.col))
    }

    /// The features only the events of the services use: `events` when one of them publishes or
    /// subscribes, and, for the events of their channels and everything those reach, what the
    /// declarations use: unions, constraints, `bytes`, pages, invariants and audited invariants
    /// (`spec/rules.md` section 17.1). A client generates nothing for events and keeps only
    /// `events` from these (`check_capabilities`).
    #[must_use]
    pub fn event_features(&self, services: &[ServiceId]) -> BTreeSet<Feature> {
        let mut f = BTreeSet::new();
        if services.iter().any(|s| self.service(*s).uses_events()) {
            f.insert(Feature::Events);
        }
        if services
            .iter()
            .any(|s| self.first_delivery_origin(*s).is_some())
        {
            f.insert(Feature::EventsDelivery);
        }
        let mut ids = self.events_of(services);
        ids.extend(self.event_reachable_decls(services));
        for id in ids {
            self.decl_features(id, &mut f);
        }
        f
    }

    /// The origin of the first channel with `@retry`, `@deadLetter` or `@ordered` that the
    /// service publishes to or subscribes to: its `publishes` lines first, in source order, then
    /// its subscriptions. This is where the gate of `events_delivery` points.
    #[must_use]
    pub fn first_delivery_origin(&self, service: ServiceId) -> Option<&Origin> {
        let s = self.service(service);
        let published = s.publishes.iter().map(|p| p.channel);
        let subscribed = s.subscriptions.iter().map(|x| x.channel);
        published
            .chain(subscribed)
            .map(|c| self.channel(c))
            .find(|c| c.has_delivery_options())
            .map(|c| &c.origin)
    }

    /// Every declaration of the workspace that is not in the prelude and is
    /// reachable from anything (used for library-only generation).
    #[must_use]
    pub fn all_decl_ids(&self) -> Vec<DeclId> {
        (0..self.decls.len() as u32).map(DeclId).collect()
    }

    /// Features a set of services (and everything they reach) uses: [`Ir::http_features_used`]
    /// and [`Ir::event_features`].
    #[must_use]
    pub fn features_used(&self, services: &[ServiceId]) -> BTreeSet<Feature> {
        let mut f = self.http_features_used(services);
        f.extend(self.event_features(services));
        f
    }

    /// The features one declaration uses: its kind, its rules and the types and constraints of
    /// its fields.
    fn decl_features(&self, id: DeclId, f: &mut BTreeSet<Feature>) {
        let d = self.decl(id);
        match &d.kind {
            DeclKind::Union(_) => {
                f.insert(Feature::Unions);
            }
            DeclKind::Event(e) => {
                rule_features(&e.rules, f);
                for fld in &e.fields {
                    field_features(fld, f);
                }
            }
            DeclKind::Model(m) | DeclKind::Error(ErrorDecl { fields: m, .. }) => {
                rule_features(&m.rules, f);
                for fld in &m.fields {
                    field_features(fld, f);
                }
            }
            DeclKind::Const(_) => {
                f.insert(Feature::Consts);
            }
            DeclKind::Enum(e) => {
                if e.is_state_machine() {
                    f.insert(Feature::RulesTransitions);
                }
                if e.has_timed_edge() {
                    f.insert(Feature::RulesTimed);
                }
                if e.flows.iter().any(|fl| fl.flag.is_some()) {
                    f.insert(Feature::RulesFlags);
                }
            }
        }
    }

    /// The version field of the model an endpoint answers with: the field whose value is the
    /// `ETag` of a success response (`spec/rules.md` section 11.7). `None` when the success is no
    /// stream and no model with a `@version` field.
    #[must_use]
    pub fn etag_field(&self, ep: &Endpoint) -> Option<&Field> {
        if ep.success.stream {
            return None;
        }
        let Some(Type::Decl(id)) = &ep.success.ty else {
            return None;
        };
        match &self.decls.get(id.index())?.kind {
            DeclKind::Model(m) => m.fields.iter().find(|f| f.version),
            DeclKind::Error(_)
            | DeclKind::Event(_)
            | DeclKind::Enum(_)
            | DeclKind::Union(_)
            | DeclKind::Const(_) => None,
        }
    }

    /// True when a derived field is on the type of a fact or on the principal of one of the
    /// services, or on anything those reach: a value the application loads is given its derived
    /// fields too (`spec/rules.md` section 17.2), though no endpoint reaches it.
    fn facts_have_derived(&self, services: &[ServiceId]) -> bool {
        self.fact_decls(services)
            .into_iter()
            .any(|id| self.decl(id).has_derived())
    }

    /// True when a value of the type has a derived field: on the declaration of the type or on
    /// any declaration it reaches through field types, parents and union arms.
    #[must_use]
    pub fn type_reaches_derived(&self, ty: &Type) -> bool {
        let mut stack = Vec::new();
        ty.collect_decls(&mut stack);
        self.close_over(stack)
            .into_iter()
            .any(|id| self.decl(id).has_derived())
    }

    /// True when a value of the type can have a `@visible` field: on the declaration of the
    /// type or on any declaration it reaches (`spec/platform.md` section 2).
    #[must_use]
    pub fn type_reaches_visible(&self, ty: &Type) -> bool {
        let mut stack = Vec::new();
        ty.collect_decls(&mut stack);
        self.close_over(stack)
            .into_iter()
            .any(|id| self.decl(id).fields().iter().any(|f| f.visible.is_some()))
    }

    /// The types of the facts and the principal of the services and everything those reach,
    /// sorted by id. No endpoint reaches them (`reachable_decls` leaves them out), but the
    /// application hands the server values of them.
    #[must_use]
    pub fn fact_decls(&self, services: &[ServiceId]) -> Vec<DeclId> {
        let mut stack = Vec::new();
        for sid in services {
            let s = self.service(*sid);
            for fact in &s.facts {
                fact.ty.collect_decls(&mut stack);
            }
            stack.extend(s.principal.iter().map(|p| p.decl));
        }
        self.close_over(stack)
    }

    /// True when a `require`, an `ensure` or a clause of a policy that an `allow` of `ep` names
    /// carries a flag (`spec/platform.md` section 11).
    #[must_use]
    pub fn endpoint_uses_flags(&self, ep: &Endpoint) -> bool {
        ep.rules.first_flag_origin(self).is_some()
    }

    /// Features of the endpoints of the services and the declarations they reach, without what
    /// only events use. A server that merely calls a service (`consumes`) uses these.
    #[must_use]
    pub fn http_features_used(&self, services: &[ServiceId]) -> BTreeSet<Feature> {
        let mut f = BTreeSet::new();
        for id in self.reachable_decls(services) {
            self.decl_features(id, &mut f);
        }
        if self.facts_have_derived(services) {
            f.insert(Feature::RulesDerived);
        }
        for sid in services {
            let s = self.service(*sid);
            if !self.package(s.package).messages.is_empty() {
                f.insert(Feature::Messages);
            }
            if !self.retention_plan(&[*sid]).is_empty() {
                f.insert(Feature::Retention);
            }
            for ep in &s.endpoints {
                if ep.auth.kind != AuthKind::None {
                    f.insert(Feature::Auth);
                }
                if !ep.middleware.is_empty() {
                    f.insert(Feature::Middleware);
                }
                if ep.rules.has_statements() {
                    f.insert(Feature::RulesEndpoint);
                }
                if ep.rules.version.is_some() || self.etag_field(ep).is_some() {
                    f.insert(Feature::RulesVersion);
                }
                if ep.idempotency.is_some() {
                    f.insert(Feature::Idempotency);
                }
                if ep.saga.is_some() {
                    f.insert(Feature::Sagas);
                }
                if ep.timeout_ms.is_some() {
                    f.insert(Feature::Timeouts);
                }
                if ep.slo.is_some() {
                    f.insert(Feature::Slo);
                }
                if ep.rules.transition.is_some() {
                    f.insert(Feature::RulesTransitions);
                }
                if ep.rules.transition.as_ref().is_some_and(|t| t.is_timed()) {
                    f.insert(Feature::RulesTimed);
                }
                if ep.rules.first_policy_origin().is_some() {
                    f.insert(Feature::RulesPolicies);
                }
                if ep.rules.first_audited_origin().is_some() {
                    f.insert(Feature::RulesRollout);
                }
                if self.endpoint_uses_flags(ep) {
                    f.insert(Feature::RulesFlags);
                }
                for p in &ep.params {
                    p.ty.features(&mut f);
                    match p.binding {
                        Binding::Header => {
                            f.insert(Feature::Headers);
                        }
                        Binding::Body if p.stream => {
                            f.insert(Feature::StreamingWs);
                        }
                        Binding::Body | Binding::Path | Binding::Query | Binding::QueryObject => {}
                    }
                    if !p.constraints.is_empty() {
                        f.insert(Feature::Constraints);
                    }
                }
                if let Some(t) = &ep.success.ty {
                    t.features(&mut f);
                }
                if ep.success.stream {
                    f.insert(if ep.method == Method::Ws {
                        Feature::StreamingWs
                    } else {
                        Feature::StreamingSse
                    });
                }
                if ep.method == Method::Ws {
                    f.insert(Feature::StreamingWs);
                }
            }
        }
        f
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Package {
    pub id: PackageId,
    pub name: String,
    pub kind: PackageKind,
    /// The message catalogs of the member (`spec/platform.md` section 22): language tag, then
    /// message key, then text. Sorted. Omitted from the JSON when the member has none.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub messages: std::collections::BTreeMap<String, std::collections::BTreeMap<String, String>>,
    /// The directory of the catalogs, relative to the member (`messages`), so that a diagnostic can
    /// name a catalog file. Omitted when there is no catalog.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub messages_dir: Option<String>,
}

impl Package {
    /// The place of the first catalog file (`<messages_dir>/<tag>.toml`), the origin that
    /// `E0801` names when a generator cannot use catalogs. `None` without a catalog.
    #[must_use]
    pub fn first_catalog_origin(&self) -> Option<Origin> {
        let tag = self.messages.keys().next()?;
        let dir = self.messages_dir.as_deref().unwrap_or("messages");
        Some(Origin {
            file: format!("{dir}/{tag}.toml"),
            line: 1,
            col: 1,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PackageKind {
    Service,
    Library,
    Registry,
    Prelude,
}

/// Where something was declared, for diagnostics only. Paths are relative to
/// the package root so the IR stays machine-independent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Origin {
    pub file: String,
    pub line: u32,
    pub col: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Decl {
    pub id: DeclId,
    pub package: PackageId,
    pub module: String,
    pub name: String,
    pub qualified: String,
    /// Declaration signature, `spec/ir.md` §5.1. The single source of truth.
    pub sig: String,
    pub docs: Option<String>,
    pub deprecated: Option<String>,
    pub origin: Origin,
    #[serde(flatten)]
    pub kind: DeclKind,
}

impl Decl {
    fn collect_refs(&self, out: &mut Vec<DeclId>) {
        match &self.kind {
            DeclKind::Model(m) => {
                if let Some(p) = m.parent {
                    out.push(p);
                }
                for f in &m.fields {
                    f.ty.collect_decls(out);
                }
            }
            DeclKind::Error(e) => {
                for f in &e.fields.fields {
                    f.ty.collect_decls(out);
                }
            }
            DeclKind::Event(e) => {
                for f in &e.fields {
                    f.ty.collect_decls(out);
                }
            }
            DeclKind::Union(u) => out.extend(u.arms.iter().map(|a| a.decl)),
            DeclKind::Const(c) => c.ty.collect_decls(out),
            DeclKind::Enum(_) => {}
        }
    }

    /// Rules for models, errors and events; empty for anything else.
    #[must_use]
    pub fn rules(&self) -> &[Rule] {
        match &self.kind {
            DeclKind::Model(m) => &m.rules,
            DeclKind::Error(e) => &e.fields.rules,
            DeclKind::Event(e) => &e.rules,
            DeclKind::Enum(_) | DeclKind::Union(_) | DeclKind::Const(_) => &[],
        }
    }

    /// Fields for models, errors and events; empty for anything else.
    #[must_use]
    pub fn fields(&self) -> &[Field] {
        match &self.kind {
            DeclKind::Model(m) => &m.fields,
            DeclKind::Error(e) => &e.fields.fields,
            DeclKind::Event(e) => &e.fields,
            DeclKind::Enum(_) | DeclKind::Union(_) | DeclKind::Const(_) => &[],
        }
    }

    /// The derived fields of the declaration in the order their values are computed
    /// (`spec/rules.md` section 17.2): a field after every derived field it reads, and in
    /// declaration order among fields that do not depend on each other. Inherited fields come
    /// first, as they do in [`Decl::fields`]. A cycle (the compiler refuses it, `E0960`) does
    /// not loop: what is left comes last, in declaration order.
    #[must_use]
    pub fn derived_fields(&self) -> Vec<&Field> {
        let mut left: Vec<&Field> = self
            .fields()
            .iter()
            .filter(|f| f.derived.is_some())
            .collect();
        let mut out: Vec<&Field> = Vec::with_capacity(left.len());
        while !left.is_empty() {
            let ready = left.iter().position(|f| {
                f.derived_reads()
                    .iter()
                    .all(|r| !left.iter().any(|o| o.name == *r && !std::ptr::eq(*o, *f)))
            });
            out.push(left.remove(ready.unwrap_or(0)));
        }
        out
    }

    /// True when a field of the declaration is derived.
    #[must_use]
    pub fn has_derived(&self) -> bool {
        self.fields().iter().any(|f| f.derived.is_some())
    }

    #[must_use]
    pub fn is_event(&self) -> bool {
        matches!(self.kind, DeclKind::Event(_))
    }

    #[must_use]
    pub fn is_error(&self) -> bool {
        matches!(self.kind, DeclKind::Error(_))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DeclKind {
    Model(Model),
    Error(ErrorDecl),
    Enum(Enum),
    Union(Union),
    Const(Const),
    /// An event (`spec/events.md` section 3).
    Event(Event),
}

/// An event declaration: a record with event identity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Event {
    pub fields: Vec<Field>,
    /// Invariants. Omitted from the JSON when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<Rule>,
    /// The rules digest of the event (`spec/events.md` section 6.1). Omitted when it has none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rules_digest: Option<String>,
}

/// A channel: one logical stream of events (`spec/events.md` section 4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Channel {
    pub name: String,
    pub package: PackageId,
    pub module: String,
    pub qualified: String,
    pub topic: String,
    /// The carried events, in `carries` order.
    pub events: Vec<DeclId>,
    pub key: Option<ChannelKey>,
    pub docs: Option<String>,
    pub deprecated: Option<String>,
    /// `@retry(max: n, backoff: ..)` (`spec/events.md` section 10.2). Omitted when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry: Option<Retry>,
    /// `@deadLetter`. Omitted when false.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub dead_letter: bool,
    /// `@ordered`. Omitted when false.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub ordered: bool,
    pub origin: Origin,
}

impl Channel {
    /// True when the channel has one of the three annotations of `spec/events.md` section 10.2:
    /// a service that publishes or subscribes to it uses the feature `events_delivery`.
    #[must_use]
    pub fn has_delivery_options(&self) -> bool {
        self.retry.is_some() || self.dead_letter || self.ordered
    }
}

/// `@retry(max: n, backoff: fixed | exponential)`: how often a consumer handles a message again
/// after its handler failed (`spec/events.md` section 10.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Retry {
    /// 0 to 100.
    pub max: u32,
    pub backoff: Backoff,
}

/// The wait between two runs of a handler.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Backoff {
    Fixed,
    Exponential,
}

impl Backoff {
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Backoff::Fixed => "fixed",
            Backoff::Exponential => "exponential",
        }
    }
}

/// The `key` of a channel: the field that orders the stream.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChannelKey {
    pub name: String,
    pub wire: String,
    #[serde(rename = "type")]
    pub ty: Type,
}

/// One channel of a `publishes` line, with the position of its name.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Publication {
    pub channel: ChannelId,
    pub origin: Origin,
}

/// `subscribes C as "group" { on E -> handler }`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Subscription {
    pub channel: ChannelId,
    pub group: String,
    pub handlers: Vec<Handler>,
    pub origin: Origin,
}

/// One `on E -> name` line.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Handler {
    pub event: DeclId,
    pub name: String,
    pub docs: Option<String>,
    pub origin: Origin,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Model {
    pub parent: Option<DeclId>,
    /// Own and inherited fields; inherited (parent-first) come first.
    pub fields: Vec<Field>,
    /// Invariants, inherited first. Omitted from the JSON when empty, so the IR of a contract
    /// without rules is unchanged (`spec/rules.md` section 13).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<Rule>,
    /// `@retain` (`spec/platform.md` section 23). Omitted from the JSON when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retain: Option<retention::Retain>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ErrorDecl {
    pub default_status: Option<u16>,
    #[serde(flatten)]
    pub fields: Model,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Enum {
    pub variants: Vec<Variant>,
    /// The edges of a state machine, in first-declaration order, without duplicates
    /// (`spec/rules.md` section 10.4). Omitted from the JSON when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub flows: Vec<Flow>,
    /// The final states, as variant names in declaration order. Omitted from the JSON when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub finals: Vec<String>,
}

impl Enum {
    /// An enum without a state machine.
    #[must_use]
    pub fn plain(variants: Vec<Variant>) -> Self {
        Enum {
            variants,
            flows: Vec::new(),
            finals: Vec::new(),
        }
    }

    /// True when the enum declares at least one flow.
    #[must_use]
    pub fn is_state_machine(&self) -> bool {
        !self.flows.is_empty()
    }

    /// True when at least one edge is timed (`spec/platform.md` section 10).
    #[must_use]
    pub fn has_timed_edge(&self) -> bool {
        self.flows.iter().any(Flow::is_timed)
    }
}

/// One edge of a state machine, by variant names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Flow {
    pub from: String,
    pub to: String,
    /// `flow a -> b after d` (`spec/platform.md` section 10): the milliseconds a value must have
    /// been in `from` before it may move, a decimal string so it stays exact. Omitted from the
    /// JSON when the edge is not timed, so the IR of a contract without `after` is unchanged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after_ms: Option<String>,
    /// `flow a -> b when flag("name")` (`spec/platform.md` section 11): the edge exists only
    /// while the flag is on. Omitted from the JSON when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flag: Option<String>,
}

impl Flow {
    /// True for a timed edge.
    #[must_use]
    pub fn is_timed(&self) -> bool {
        self.after_ms.is_some()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Variant {
    pub name: String,
    pub wire: String,
    pub docs: Option<String>,
    pub deprecated: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Union {
    pub discriminator: String,
    pub arms: Vec<UnionArm>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnionArm {
    pub label: String,
    pub decl: DeclId,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Const {
    #[serde(rename = "type")]
    pub ty: Type,
    pub value: Literal,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Field {
    pub name: String,
    pub wire: String,
    #[serde(rename = "type")]
    pub ty: Type,
    pub optional: bool,
    pub default: Option<Literal>,
    pub readonly: bool,
    /// `@version` (`spec/rules.md` section 11.7): the version of the model. Omitted from the JSON
    /// when false, so the IR of a contract without the annotation is unchanged.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub version: bool,
    /// The expression of a derived field (`spec/rules.md` section 17.2): `total: decimal =
    /// subtotal - discount`. Omitted from the JSON when the field is not derived, so the IR of a
    /// contract without derived fields is the same bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub derived: Option<Expr>,
    /// The identity of a derived field (`spec/rules.md` sections 6 and 17.2), set exactly when
    /// `derived` is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub derived_id: Option<String>,
    /// `@pii` (`spec/rules.md` section 19.3): the field holds personal data. Omitted from the
    /// JSON when false, so the IR of a contract without the annotation is unchanged.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub pii: bool,
    /// `@secret` (`spec/rules.md` section 19.3): the value must never be shown. Omitted when false.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub secret: bool,
    /// `@visible(when: e)` (`spec/platform.md` section 2): the field is sent to a caller only
    /// when `e` is true for that caller. Omitted from the JSON when absent, so the IR of a
    /// contract without the annotation is unchanged. Not part of any signature.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visible: Option<Expr>,
    /// The principal model `visible` was checked against (`spec/platform.md` section 2): the
    /// caller it reads is a value of this model. Present exactly when `visible` is. A server
    /// whose principal is another model has no caller of that type, and hides the field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visible_principal: Option<DeclId>,
    pub inherited_from: Option<DeclId>,
    pub constraints: Constraints,
    pub example: Option<Literal>,
    pub docs: Option<String>,
    pub deprecated: Option<String>,
    pub origin: Origin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Scalar {
    String,
    Int,
    Long,
    Float,
    Decimal,
    Bool,
    Date,
    Datetime,
    Time,
    Uuid,
    Bytes,
    Json,
}

impl Scalar {
    pub const ALL: [Scalar; 12] = [
        Scalar::String,
        Scalar::Int,
        Scalar::Long,
        Scalar::Float,
        Scalar::Decimal,
        Scalar::Bool,
        Scalar::Date,
        Scalar::Datetime,
        Scalar::Time,
        Scalar::Uuid,
        Scalar::Bytes,
        Scalar::Json,
    ];
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Scalar::String => "string",
            Scalar::Int => "int",
            Scalar::Long => "long",
            Scalar::Float => "float",
            Scalar::Decimal => "decimal",
            Scalar::Bool => "bool",
            Scalar::Date => "date",
            Scalar::Datetime => "datetime",
            Scalar::Time => "time",
            Scalar::Uuid => "uuid",
            Scalar::Bytes => "bytes",
            Scalar::Json => "json",
        }
    }
    #[must_use]
    pub fn from_name(s: &str) -> Option<Scalar> {
        Scalar::ALL.into_iter().find(|x| x.name() == s)
    }
    #[must_use]
    pub fn is_numeric(self) -> bool {
        matches!(
            self,
            Scalar::Int | Scalar::Long | Scalar::Float | Scalar::Decimal
        )
    }
}

impl fmt::Display for Scalar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Type {
    Scalar(Scalar),
    List(Box<Type>),
    Map(Box<Type>),
    Page(Box<Type>),
    Decl(DeclId),
}

impl Type {
    pub fn collect_decls(&self, out: &mut Vec<DeclId>) {
        match self {
            Type::Scalar(_) => {}
            Type::List(t) | Type::Map(t) | Type::Page(t) => t.collect_decls(out),
            Type::Decl(d) => out.push(*d),
        }
    }
    fn features(&self, f: &mut BTreeSet<Feature>) {
        match self {
            Type::Scalar(Scalar::Bytes) => {
                f.insert(Feature::Bytes);
            }
            Type::Scalar(_) | Type::Decl(_) => {}
            Type::List(t) | Type::Map(t) => t.features(f),
            Type::Page(t) => {
                f.insert(Feature::Page);
                t.features(f);
            }
        }
    }
}

/// The features the invariants of a declaration use.
fn rule_features(rules: &[Rule], f: &mut BTreeSet<Feature>) {
    if !rules.is_empty() {
        f.insert(Feature::RulesInvariants);
    }
    if rules.iter().any(Rule::is_audited) {
        f.insert(Feature::RulesRollout);
    }
    if rules.iter().any(|r| r.flag.is_some()) {
        f.insert(Feature::RulesFlags);
    }
}

/// The features the type and the constraints of a field use.
fn field_features(fld: &Field, f: &mut BTreeSet<Feature>) {
    fld.ty.features(f);
    if fld.derived.is_some() {
        f.insert(Feature::RulesDerived);
    }
    if fld.visible.is_some() {
        f.insert(Feature::Visibility);
    }
    if !fld.constraints.is_empty() {
        f.insert(Feature::Constraints);
    }
}

impl Field {
    /// Source names of the fields of the declaration that the derived expression reads, first
    /// use first. Empty when the field is not derived.
    #[must_use]
    pub fn derived_reads(&self) -> Vec<String> {
        self.derived
            .as_ref()
            .map(|e| e.self_reads())
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Constraints {
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub min_len: Option<u64>,
    pub max_len: Option<u64>,
    pub pattern: Option<String>,
    pub format: Option<StringFormat>,
}

impl Constraints {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self == &Constraints::default()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StringFormat {
    Email,
    Uri,
    Hostname,
    Ipv4,
    Ipv6,
}

impl StringFormat {
    #[must_use]
    pub fn from_name(s: &str) -> Option<Self> {
        Some(match s {
            "email" => Self::Email,
            "uri" => Self::Uri,
            "hostname" => Self::Hostname,
            "ipv4" => Self::Ipv4,
            "ipv6" => Self::Ipv6,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "lit", content = "value", rename_all = "snake_case")]
pub enum Literal {
    String(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    Null,
    /// Enum variant by source name; `wire` is the resolved wire value.
    EnumVariant {
        name: String,
        wire: String,
    },
    EmptyList,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Service {
    pub id: ServiceId,
    pub package: PackageId,
    pub name: String,
    pub prefix: String,
    pub base_url: Option<String>,
    pub docs: Option<String>,
    pub auth: Auth,
    pub middleware: Vec<String>,
    pub endpoints: Vec<Endpoint>,
    /// One loader method per fact name (`spec/rules.md` section 9.2), sorted by name.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub facts: Vec<ServiceFact>,
    /// Digest of the invariants the service reaches (`spec/rules.md` section 9.7).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rules_digest: Option<String>,
    /// The package's principal (`spec/rules.md` section 11.1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub principal: Option<ServicePrincipal>,
    /// Channels the service publishes to, in source order, each with the position of the channel
    /// name in its `publishes` line (`spec/events.md` section 13). Omitted from the JSON when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub publishes: Vec<Publication>,
    /// The service's subscriptions, in source order. Omitted from the JSON when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub subscriptions: Vec<Subscription>,
    pub origin: Origin,
}

impl Service {
    /// True when the service publishes or subscribes (the feature `events`).
    #[must_use]
    pub fn uses_events(&self) -> bool {
        !self.publishes.is_empty() || !self.subscriptions.is_empty()
    }

    /// Where the first `publishes` or `subscribes` line is, for diagnostics: the channel name of
    /// the first `publishes` line when the service publishes, otherwise the first `subscribes`
    /// line.
    #[must_use]
    pub fn first_event_origin(&self) -> Option<&Origin> {
        match self.publishes.first() {
            Some(p) => Some(&p.origin),
            None => self.subscriptions.first().map(|s| &s.origin),
        }
    }

    /// The channels the service publishes to, in source order.
    pub fn published_channels(&self) -> impl Iterator<Item = ChannelId> + '_ {
        self.publishes.iter().map(|p| p.channel)
    }

    /// Every channel the service publishes to or subscribes to, in id order, once each.
    #[must_use]
    pub fn channels_used(&self) -> Vec<ChannelId> {
        let all: BTreeSet<ChannelId> = self
            .published_channels()
            .chain(self.subscriptions.iter().map(|s| s.channel))
            .collect();
        all.into_iter().collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Method {
    Get,
    Post,
    Put,
    Patch,
    Delete,
    Head,
    Options,
    Ws,
}

impl Method {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Method::Get => "GET",
            Method::Post => "POST",
            Method::Put => "PUT",
            Method::Patch => "PATCH",
            Method::Delete => "DELETE",
            Method::Head => "HEAD",
            Method::Options => "OPTIONS",
            Method::Ws => "WS",
        }
    }
    #[must_use]
    pub fn lower(self) -> &'static str {
        match self {
            Method::Get => "get",
            Method::Post => "post",
            Method::Put => "put",
            Method::Patch => "patch",
            Method::Delete => "delete",
            Method::Head => "head",
            Method::Options => "options",
            Method::Ws => "ws",
        }
    }
    #[must_use]
    pub fn allows_body(self) -> bool {
        !matches!(self, Method::Get | Method::Head | Method::Options)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Endpoint {
    pub name: String,
    pub method: Method,
    /// Full path (prefix + endpoint path), `{param}` segments.
    pub path: String,
    /// Operation signature, `spec/ir.md` §5.2.
    pub sig: String,
    pub docs: Option<String>,
    pub deprecated: Option<String>,
    pub auth: Auth,
    pub middleware: Vec<String>,
    pub params: Vec<Param>,
    pub success: Success,
    pub errors: Vec<ErrorResponse>,
    /// The statements of the endpoint block. Omitted from the JSON when empty.
    #[serde(default, skip_serializing_if = "EndpointRules::is_empty")]
    pub rules: EndpointRules,
    /// `@idempotent` (`spec/rules.md` section 11.8). Omitted from the JSON when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency: Option<Idempotency>,
    /// The saga that implements the endpoint (`spec/platform.md` section 17). Omitted from the
    /// JSON when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub saga: Option<Saga>,
    /// `@timeout` (`spec/platform.md` section 12): the deadline of a request in milliseconds, a
    /// decimal string so it stays exact. Omitted from the JSON when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<String>,
    /// `@slo` (`spec/platform.md` section 13). Omitted from the JSON when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slo: Option<Slo>,
    pub origin: Origin,
}

/// The service level an endpoint states with `@slo` (`spec/platform.md` section 13). Each
/// latency is the milliseconds within which that share of requests answers, as a decimal string;
/// `errors` is the most percent of requests that may end in a `5xx`, as written. A member that
/// the annotation does not give is omitted from the JSON.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Slo {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub p50_ms: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub p90_ms: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub p95_ms: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub p99_ms: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub errors: Option<String>,
}

impl Slo {
    /// The stated latency objectives as `(name, milliseconds)`, fastest share first.
    #[must_use]
    pub fn latencies(&self) -> Vec<(&'static str, &str)> {
        [
            ("p50", &self.p50_ms),
            ("p90", &self.p90_ms),
            ("p95", &self.p95_ms),
            ("p99", &self.p99_ms),
        ]
        .into_iter()
        .filter_map(|(n, v)| v.as_deref().map(|v| (n, v)))
        .collect()
    }
}

/// `@idempotent` on an endpoint (`spec/rules.md` sections 11.8 and 11.9).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Idempotency {
    /// The request header that carries the key, `Idempotency-Key` unless the annotation names
    /// another.
    pub header: String,
    /// `required: true`: a request without the header is `422 ValidationError`.
    pub required: bool,
    /// True when the scope of a key includes the caller, that is when the endpoint's effective
    /// auth is not `none`.
    pub caller: bool,
    /// The annotation.
    pub origin: Origin,
}

impl Endpoint {
    pub fn params_by(&self, b: Binding) -> impl Iterator<Item = &Param> {
        self.params.iter().filter(move |p| p.binding == b)
    }
    #[must_use]
    pub fn body(&self) -> Option<&Param> {
        self.params_by(Binding::Body).next()
    }
    #[must_use]
    pub fn query_object(&self) -> Option<&Param> {
        self.params_by(Binding::QueryObject).next()
    }
    /// Path split into literal and `{param}` parts.
    #[must_use]
    pub fn path_segments(&self) -> Vec<PathSegment<'_>> {
        split_path(&self.path)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathSegment<'a> {
    Literal(&'a str),
    Param(&'a str),
}

/// Splits `/a/{id}/b` into `["/a/", Param(id), "/b"]`.
#[must_use]
pub fn split_path(path: &str) -> Vec<PathSegment<'_>> {
    let mut out = Vec::new();
    let mut rest = path;
    while let Some(start) = rest.find('{') {
        if start > 0 {
            out.push(PathSegment::Literal(&rest[..start]));
        }
        let after = &rest[start + 1..];
        let Some(end) = after.find('}') else {
            out.push(PathSegment::Literal(&rest[start..]));
            return out;
        };
        out.push(PathSegment::Param(&after[..end]));
        rest = &after[end + 1..];
    }
    if !rest.is_empty() {
        out.push(PathSegment::Literal(rest));
    }
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Binding {
    Path,
    Query,
    QueryObject,
    Header,
    Body,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Param {
    pub name: String,
    pub binding: Binding,
    pub header: Option<String>,
    #[serde(rename = "type")]
    pub ty: Type,
    pub optional: bool,
    pub stream: bool,
    pub constraints: Constraints,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Success {
    pub status: u16,
    #[serde(rename = "type")]
    pub ty: Option<Type>,
    pub stream: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub status: u16,
    pub decl: DeclId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Auth {
    pub kind: AuthKind,
    pub header: Option<String>,
    /// True when the author wrote `@auth` on the endpoint or on its service, `@auth(none)`
    /// included. It is not part of the JSON and not of equality: only the `auth-required` lint
    /// reads it, in the process that compiled the contract (`spec/rules.md` section 11.6).
    #[serde(skip)]
    pub declared: bool,
}

impl PartialEq for Auth {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind && self.header == other.header
    }
}

impl Eq for Auth {}

impl Auth {
    /// An auth scheme that is not marked as written by the author.
    #[must_use]
    pub fn new(kind: AuthKind, header: Option<String>) -> Self {
        Auth {
            kind,
            header,
            declared: false,
        }
    }

    #[must_use]
    pub fn none() -> Self {
        Auth::new(AuthKind::None, None)
    }

    /// The same scheme, marked as written by the author.
    #[must_use]
    pub fn declared(mut self) -> Self {
        self.declared = true;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthKind {
    None,
    Bearer,
    Basic,
    ApiKey,
}

/// Language features a generator may or may not support (`spec/ir.md` §4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Feature {
    Unions,
    Constraints,
    Auth,
    Middleware,
    Headers,
    StreamingSse,
    StreamingWs,
    Page,
    Consts,
    Bytes,
    RulesInvariants,
    RulesEndpoint,
    RulesTransitions,
    RulesPolicies,
    /// `@rollout(audit)` on a rule (`spec/rules.md` section 17.1). A generator that does not
    /// know it would enforce a rule that must not reject.
    RulesRollout,
    /// `@version`: the `If-Match` guard of a fact and the `ETag` of a response
    /// (`spec/rules.md` section 11.7).
    RulesVersion,
    /// `@idempotent` (`spec/rules.md` section 11.8).
    Idempotency,
    /// A derived field: `total: decimal = subtotal - discount` (`spec/rules.md` section 17.2).
    /// A generator that does not know it would leave the field without a value.
    RulesDerived,
    /// A `saga` endpoint (`spec/platform.md` section 17).
    Sagas,
    /// `publishes` or `subscribes` (`spec/events.md` section 13).
    Events,
    /// `@retry`, `@deadLetter` or `@ordered` on a channel that a service publishes or subscribes
    /// to (`spec/events.md` section 10.8).
    EventsDelivery,
    /// `flow ... after` and `transition ... since` (`spec/platform.md` section 10).
    RulesTimed,
    /// `when flag("name")` on a rule, a `require`, an `ensure`, a `permit` or `forbid` clause or
    /// a `flow` (`spec/platform.md` section 11).
    RulesFlags,
    /// `@timeout` (`spec/platform.md` section 12).
    Timeouts,
    /// `@slo` (`spec/platform.md` section 13): the metrics hook.
    Slo,
    /// A member with message catalogs (`spec/platform.md` section 22). Opt-in: a generator that
    /// does not declare it would send the contract's text where a catalog was meant to be used.
    Messages,
    /// A server with `@retain` models (`spec/platform.md` section 23). Opt-in, and only a server
    /// is concerned: a client generates nothing for it.
    Retention,
    /// `@visible(when: e)` on a field (`spec/platform.md` section 2). Opt-in: a generator that
    /// does not declare it would send a field that must be hidden. Clients generate nothing for
    /// it, so the gate does not ask them.
    Visibility,
}

impl Feature {
    /// The rules features (`spec/rules.md` section 8).
    pub const RULES: [Feature; 4] = [
        Feature::RulesInvariants,
        Feature::RulesEndpoint,
        Feature::RulesTransitions,
        Feature::RulesPolicies,
    ];
    /// Features no generator supports unless it says so: [`Feature::ALL`] minus these is what
    /// `all_except` hands out. A feature that changes what a rule means must be opted into.
    pub const OPT_IN: [Feature; 13] = [
        Feature::RulesRollout,
        Feature::RulesVersion,
        Feature::Idempotency,
        Feature::RulesDerived,
        Feature::Sagas,
        Feature::EventsDelivery,
        Feature::RulesTimed,
        Feature::RulesFlags,
        Feature::Timeouts,
        Feature::Slo,
        Feature::Messages,
        Feature::Retention,
        Feature::Visibility,
    ];
    pub const ALL: [Feature; 28] = [
        Feature::Unions,
        Feature::Constraints,
        Feature::Auth,
        Feature::Middleware,
        Feature::Headers,
        Feature::StreamingSse,
        Feature::StreamingWs,
        Feature::Page,
        Feature::Consts,
        Feature::Bytes,
        Feature::RulesInvariants,
        Feature::RulesEndpoint,
        Feature::RulesTransitions,
        Feature::RulesPolicies,
        Feature::RulesRollout,
        Feature::RulesVersion,
        Feature::Idempotency,
        Feature::RulesDerived,
        Feature::Sagas,
        Feature::Events,
        Feature::EventsDelivery,
        Feature::RulesTimed,
        Feature::RulesFlags,
        Feature::Timeouts,
        Feature::Slo,
        Feature::Messages,
        Feature::Retention,
        Feature::Visibility,
    ];
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Feature::Unions => "unions",
            Feature::Constraints => "constraints",
            Feature::Auth => "auth",
            Feature::Middleware => "middleware",
            Feature::Headers => "headers",
            Feature::StreamingSse => "streaming_sse",
            Feature::StreamingWs => "streaming_ws",
            Feature::Page => "page",
            Feature::Consts => "consts",
            Feature::Bytes => "bytes",
            Feature::RulesInvariants => "rules_invariants",
            Feature::RulesEndpoint => "rules_endpoint",
            Feature::RulesTransitions => "rules_transitions",
            Feature::RulesPolicies => "rules_policies",
            Feature::RulesRollout => "rules_rollout",
            Feature::RulesVersion => "rules_version",
            Feature::Idempotency => "idempotency",
            Feature::RulesDerived => "rules_derived",
            Feature::Sagas => "sagas",
            Feature::Events => "events",
            Feature::EventsDelivery => "events_delivery",
            Feature::RulesTimed => "rules_timed",
            Feature::RulesFlags => "rules_flags",
            Feature::Timeouts => "timeouts",
            Feature::Slo => "slo",
            Feature::Messages => "messages",
            Feature::Retention => "retention",
            Feature::Visibility => "visibility",
        }
    }
    #[must_use]
    pub fn from_name(s: &str) -> Option<Feature> {
        Feature::ALL.into_iter().find(|f| f.name() == s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_path_handles_params_and_literals() {
        assert_eq!(
            split_path("/orders/{id}/items/{sku}"),
            vec![
                PathSegment::Literal("/orders/"),
                PathSegment::Param("id"),
                PathSegment::Literal("/items/"),
                PathSegment::Param("sku"),
            ]
        );
        assert_eq!(split_path("/"), vec![PathSegment::Literal("/")]);
    }

    #[test]
    fn scalar_round_trips_names() {
        for s in Scalar::ALL {
            assert_eq!(Scalar::from_name(s.name()), Some(s));
        }
    }

    #[test]
    fn feature_round_trips_names() {
        for f in Feature::ALL {
            assert_eq!(Feature::from_name(f.name()), Some(f));
        }
    }
}

#[cfg(test)]
mod derived_tests;
#[cfg(test)]
mod events_tests;
#[cfg(test)]
mod platform_p1_tests;

#[cfg(test)]
mod delivery_tests;

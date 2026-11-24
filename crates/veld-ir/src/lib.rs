//! Veld IR v1 — see `spec/ir.md`.
//!
//! The IR is immutable and fully resolved: no imports, inheritance already
//! flattened into `fields`, every implicit (status codes, `ValidationError`,
//! bindings, wire names) made explicit. Generators never need to re-validate.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;

pub mod rules;
pub mod schema;
pub mod suggest;

pub use rules::{
    Always, BinExpr, Builtin, CallExpr, EnumLit, Expr, ExprKind, ExprLit, ExprType, Op, PathRef,
    QuantExpr, Quantifier, Root, Rule, Step,
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

/// A whole compiled workspace.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Ir {
    pub ir_version: String,
    pub packages: Vec<Package>,
    pub decls: Vec<Decl>,
    pub services: Vec<Service>,
    /// service name -> consumed service names (sorted). May contain cycles.
    pub consumes: std::collections::BTreeMap<String, Vec<String>>,
}

impl Ir {
    #[must_use]
    pub fn decl(&self, id: DeclId) -> &Decl {
        &self.decls[id.index()]
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
        let mut seen = BTreeSet::new();
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
        while let Some(id) = stack.pop() {
            if !seen.insert(id) {
                continue;
            }
            self.decl(id).collect_refs(&mut stack);
        }
        seen.into_iter().collect()
    }

    /// Every declaration of the workspace that is not in the prelude and is
    /// reachable from anything (used for library-only generation).
    #[must_use]
    pub fn all_decl_ids(&self) -> Vec<DeclId> {
        (0..self.decls.len() as u32).map(DeclId).collect()
    }

    /// Features a set of services (and everything they reach) uses.
    #[must_use]
    pub fn features_used(&self, services: &[ServiceId]) -> BTreeSet<Feature> {
        let mut f = BTreeSet::new();
        let decls = self.reachable_decls(services);
        for id in &decls {
            let d = self.decl(*id);
            match &d.kind {
                DeclKind::Union(_) => {
                    f.insert(Feature::Unions);
                }
                DeclKind::Model(m) | DeclKind::Error(ErrorDecl { fields: m, .. }) => {
                    if !m.rules.is_empty() {
                        f.insert(Feature::RulesInvariants);
                    }
                    for fld in &m.fields {
                        fld.ty.features(&mut f);
                        if !fld.constraints.is_empty() {
                            f.insert(Feature::Constraints);
                        }
                    }
                }
                DeclKind::Const(_) => {
                    f.insert(Feature::Consts);
                }
                DeclKind::Enum(_) => {}
            }
        }
        for sid in services {
            let s = self.service(*sid);
            for ep in &s.endpoints {
                if ep.auth.kind != AuthKind::None {
                    f.insert(Feature::Auth);
                }
                if !ep.middleware.is_empty() {
                    f.insert(Feature::Middleware);
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
            DeclKind::Union(u) => out.extend(u.arms.iter().map(|a| a.decl)),
            DeclKind::Const(c) => c.ty.collect_decls(out),
            DeclKind::Enum(_) => {}
        }
    }

    /// Rules for models and errors; empty for anything else.
    #[must_use]
    pub fn rules(&self) -> &[Rule] {
        match &self.kind {
            DeclKind::Model(m) => &m.rules,
            DeclKind::Error(e) => &e.fields.rules,
            DeclKind::Enum(_) | DeclKind::Union(_) | DeclKind::Const(_) => &[],
        }
    }

    /// Fields for models and errors; empty for anything else.
    #[must_use]
    pub fn fields(&self) -> &[Field] {
        match &self.kind {
            DeclKind::Model(m) => &m.fields,
            DeclKind::Error(e) => &e.fields.fields,
            DeclKind::Enum(_) | DeclKind::Union(_) | DeclKind::Const(_) => &[],
        }
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
    pub origin: Origin,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Auth {
    pub kind: AuthKind,
    pub header: Option<String>,
}

impl Auth {
    #[must_use]
    pub fn none() -> Self {
        Auth {
            kind: AuthKind::None,
            header: None,
        }
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
}

impl Feature {
    /// The rules features (`spec/rules.md` section 8).
    pub const RULES: [Feature; 4] = [
        Feature::RulesInvariants,
        Feature::RulesEndpoint,
        Feature::RulesTransitions,
        Feature::RulesPolicies,
    ];
    pub const ALL: [Feature; 14] = [
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

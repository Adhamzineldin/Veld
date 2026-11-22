//! Endpoint descriptions: static data shared by the router core and the client core.

use super::schema::Schema;

/// Where a parameter travels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Binding {
    Path,
    Query,
    /// A model whose fields are the query keys.
    QueryObject,
    Header,
    Body,
}

/// How an endpoint authenticates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthKind {
    Bearer,
    Basic,
    ApiKey,
}

/// An endpoint's authentication: the kind and, for an API key, the header.
#[derive(Debug, Clone, Copy)]
pub struct AuthSpec {
    pub kind: AuthKind,
    pub header: Option<&'static str>,
}

/// What an endpoint speaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A request and a response.
    Http,
    /// Server-Sent Events: a `get` whose response is a stream.
    Sse,
    /// A WebSocket.
    Ws,
}

/// One parameter of an endpoint.
#[derive(Debug, Clone, Copy)]
pub struct ParamSpec {
    /// The name in the request struct (the contract's name).
    pub name: &'static str,
    pub binding: Binding,
    /// The header name, for [`Binding::Header`].
    pub header: Option<&'static str>,
    pub schema: &'static Schema,
    pub optional: bool,
    /// `body: stream T` of a WebSocket: the client-to-server message type, not an HTTP body.
    pub stream: bool,
}

impl ParamSpec {
    /// The location used in validation issues: `path.id`, `query.limit`, `header.X-Trace`, `body`.
    #[must_use]
    pub fn location(&self) -> String {
        match self.binding {
            Binding::Path => format!("path.{}", self.name),
            Binding::Body => "body".to_string(),
            Binding::QueryObject => "query".to_string(),
            Binding::Query => format!("query.{}", self.name),
            Binding::Header => format!("header.{}", self.header.unwrap_or(self.name)),
        }
    }
}

/// An error an endpoint declares.
#[derive(Debug, Clone, Copy)]
pub struct ErrorSpec {
    pub status: u16,
    pub name: &'static str,
    pub sig: &'static str,
    pub schema: &'static Schema,
}

/// One endpoint of a service.
#[derive(Debug, Clone, Copy)]
pub struct EndpointSpec {
    pub name: &'static str,
    pub method: &'static str,
    pub path: &'static str,
    /// The operation signature, verbatim from the contract.
    pub sig: &'static str,
    /// The success status.
    pub status: u16,
    pub auth: Option<AuthSpec>,
    pub middleware: &'static [&'static str],
    pub params: &'static [ParamSpec],
    /// The success type (an item for streams); `None` for no body.
    pub success: Option<&'static Schema>,
    pub errors: &'static [ErrorSpec],
    pub kind: Kind,
}

/// The signatures of the prelude errors the runtime itself raises.
#[derive(Debug, Clone, Copy)]
pub struct StdSigs {
    pub unauthorized: &'static str,
    pub validation: &'static str,
    pub internal: &'static str,
}

/// A path without its trailing slash (the root `/` stays): routing ignores a trailing slash.
#[must_use]
pub fn path_without_trailing_slash(p: &str) -> &str {
    if p.len() > 1 { p.strip_suffix('/').unwrap_or(p) } else { p }
}

//! Plain syntax tree. Every node carries its span.

use crate::diagnostics::Span;

#[derive(Debug, Clone, PartialEq)]
pub struct Ident {
    pub name: String,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct File {
    pub uses: Vec<Use>,
    pub decls: Vec<Decl>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Use {
    /// `iam` or `@acme/billing`; `None` means the current package.
    pub package: Option<Ident>,
    pub module: Vec<Ident>,
    pub imports: Vec<Import>,
    pub span: Span,
}

impl Use {
    /// Dotted module path.
    #[must_use]
    pub fn module_path(&self) -> String {
        self.module
            .iter()
            .map(|i| i.name.as_str())
            .collect::<Vec<_>>()
            .join(".")
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Import {
    pub name: Ident,
    pub alias: Option<Ident>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Doc {
    pub text: String,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Decl {
    pub docs: Vec<Doc>,
    pub annotations: Vec<Annotation>,
    pub kind: DeclKind,
    pub span: Span,
}

impl Decl {
    #[must_use]
    pub fn name(&self) -> &Ident {
        match &self.kind {
            DeclKind::Model(d) => &d.name,
            DeclKind::Enum(d) => &d.name,
            DeclKind::Error(d) => &d.name,
            DeclKind::Union(d) => &d.name,
            DeclKind::Service(d) => &d.name,
            DeclKind::Const(d) => &d.name,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum DeclKind {
    Model(ModelDecl),
    Enum(EnumDecl),
    Error(ErrorDecl),
    Union(UnionDecl),
    Service(ServiceDecl),
    Const(ConstDecl),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModelDecl {
    pub name: Ident,
    pub extends: Option<TypeExpr>,
    pub fields: Vec<Field>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ErrorDecl {
    pub name: Ident,
    pub status: Option<(i64, Span)>,
    pub fields: Vec<Field>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    pub docs: Vec<Doc>,
    /// Annotations before and after the field, in source order.
    pub annotations: Vec<Annotation>,
    pub name: Ident,
    pub optional: bool,
    pub ty: TypeExpr,
    pub default: Option<Literal>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnumDecl {
    pub name: Ident,
    pub variants: Vec<Variant>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Variant {
    pub docs: Vec<Doc>,
    pub annotations: Vec<Annotation>,
    pub name: Ident,
    pub wire: Option<(String, Span)>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UnionDecl {
    pub name: Ident,
    pub arms: Vec<UnionArm>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UnionArm {
    pub label: Ident,
    pub ty: TypeExpr,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConstDecl {
    pub name: Ident,
    pub ty: TypeExpr,
    pub value: Literal,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ServiceDecl {
    pub name: Ident,
    pub prefix: StrLit,
    pub endpoints: Vec<Endpoint>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StrLit {
    pub value: String,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
    Head,
    Options,
    Ws,
}

impl HttpMethod {
    #[must_use]
    pub fn from_word(w: &str) -> Option<HttpMethod> {
        Some(match w {
            "get" => HttpMethod::Get,
            "post" => HttpMethod::Post,
            "put" => HttpMethod::Put,
            "patch" => HttpMethod::Patch,
            "delete" => HttpMethod::Delete,
            "head" => HttpMethod::Head,
            "options" => HttpMethod::Options,
            "ws" => HttpMethod::Ws,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Endpoint {
    pub docs: Vec<Doc>,
    pub annotations: Vec<Annotation>,
    pub method: HttpMethod,
    pub method_span: Span,
    pub path: StrLit,
    pub name: Ident,
    pub params: Vec<Param>,
    pub responses: Option<Responses>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub annotations: Vec<Annotation>,
    pub name: Ident,
    pub optional: bool,
    pub stream: bool,
    pub ty: TypeExpr,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Responses {
    pub items: Vec<RespItem>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RespItem {
    pub status: Option<(i64, Span)>,
    pub stream: bool,
    pub ty: Option<TypeExpr>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypeExpr {
    pub kind: TypeExprKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypeExprKind {
    Named { name: Ident, args: Vec<TypeExpr> },
    List(Box<TypeExpr>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Literal {
    pub kind: LitKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LitKind {
    Str(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    Null,
    /// `.name`
    Variant(String),
    /// `[]`
    EmptyList,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Annotation {
    pub name: Ident,
    /// `None` when written without parentheses.
    pub args: Option<Vec<Arg>>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Arg {
    pub name: Option<Ident>,
    pub value: ArgValue,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ArgValue {
    Literal(Literal),
    Ident(Ident),
    /// `apiKey(header: "X")`
    Call {
        name: Ident,
        args: Vec<Arg>,
    },
}

/// Joins doc lines into the Markdown text stored in the IR.
#[must_use]
pub fn doc_text(docs: &[Doc]) -> Option<String> {
    if docs.is_empty() {
        return None;
    }
    Some(
        docs.iter()
            .map(|d| d.text.as_str())
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

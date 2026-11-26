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
    /// Top-level `example "..." { }` declarations (`spec/rules.md` section 12). They have no
    /// name of their own, so they live beside `decls`.
    pub examples: Vec<Example>,
    /// Top-level `policy` declarations (`spec/rules.md` section 11.2). Like examples they sit
    /// beside `decls`: the checker names and imports them in a table of their own.
    pub policies: Vec<PolicyDecl>,
    /// Top-level `principal` declarations (`spec/rules.md` section 11.1).
    pub principals: Vec<PrincipalDecl>,
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
            DeclKind::Event(d) => &d.name,
            DeclKind::Channel(d) => &d.name,
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
    /// `event Name { fields and rules }` (`spec/events.md` section 3).
    Event(EventDecl),
    /// `channel Name { carries .. key .. }` (`spec/events.md` section 4).
    Channel(ChannelDecl),
}

#[derive(Debug, Clone, PartialEq)]
pub struct EventDecl {
    pub name: Ident,
    pub fields: Vec<Field>,
    /// Invariants, in source order.
    pub rules: Vec<Rule>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChannelDecl {
    pub name: Ident,
    /// The `carries` lines, in source order.
    pub carries: Vec<Carries>,
    /// The `key` lines, in source order (more than one is an error).
    pub keys: Vec<ChannelKeyLine>,
    /// From the `channel` word to the closing `}`.
    pub span: Span,
}

/// `carries A, B`.
#[derive(Debug, Clone, PartialEq)]
pub struct Carries {
    pub events: Vec<TypeExpr>,
    pub span: Span,
}

/// `key field`.
#[derive(Debug, Clone, PartialEq)]
pub struct ChannelKeyLine {
    pub field: Ident,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModelDecl {
    pub name: Ident,
    pub extends: Option<TypeExpr>,
    pub fields: Vec<Field>,
    /// Invariants, in source order (`spec/rules.md` section 5).
    pub rules: Vec<Rule>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ErrorDecl {
    pub name: Ident,
    pub status: Option<(i64, Span)>,
    pub fields: Vec<Field>,
    pub rules: Vec<Rule>,
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

/// `rule name: expr [message "text"]` inside a model or error (`spec/rules.md` section 4).
#[derive(Debug, Clone, PartialEq)]
pub struct Rule {
    pub docs: Vec<Doc>,
    pub annotations: Vec<Annotation>,
    pub name: Ident,
    pub expr: Expr,
    pub message: Option<StrLit>,
    /// From the `rule` keyword to the end of the message or expression.
    pub span: Span,
}

/// An expression. Parentheses leave no node: the tree shape carries the grouping.
#[derive(Debug, Clone, PartialEq)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExprKind {
    Bool(bool),
    Int(i64),
    /// A float literal keeps its source text: a `decimal` is that text (`1.50` stays `1.50`).
    Float {
        value: f64,
        text: String,
    },
    Str(String),
    /// `.name`
    EnumLit(Ident),
    Name(Ident),
    /// `base.name`
    Field {
        base: Box<Expr>,
        name: Ident,
    },
    Call {
        name: Ident,
        args: Vec<CallArg>,
    },
    List(Vec<Expr>),
    Unary {
        op: UnOp,
        operand: Box<Expr>,
    },
    Binary {
        op: BinOp,
        l: Box<Expr>,
        r: Box<Expr>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum CallArg {
    Expr(Expr),
    /// `x => body`
    Lambda {
        var: Ident,
        body: Expr,
        span: Span,
    },
}

impl CallArg {
    #[must_use]
    pub fn span(&self) -> Span {
        match self {
            CallArg::Expr(e) => e.span,
            CallArg::Lambda { span, .. } => *span,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOp {
    /// `!` (`word: false`) or `not` (`word: true`); the formatter keeps the user's choice.
    Not {
        word: bool,
    },
    Neg,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    And,
    Or,
    Implies,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    In,
    Add,
    Sub,
    Mul,
    Div,
    Rem,
}

impl BinOp {
    /// The operator as written.
    #[must_use]
    pub fn text(self) -> &'static str {
        match self {
            BinOp::And => "&&",
            BinOp::Or => "||",
            BinOp::Implies => "implies",
            BinOp::Eq => "==",
            BinOp::Ne => "!=",
            BinOp::Lt => "<",
            BinOp::Le => "<=",
            BinOp::Gt => ">",
            BinOp::Ge => ">=",
            BinOp::In => "in",
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::Rem => "%",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnumDecl {
    pub name: Ident,
    pub variants: Vec<Variant>,
    /// `flow a -> b -> c` statements, in source order (`spec/rules.md` section 10).
    pub flows: Vec<FlowStmt>,
    /// `final a, b` statements, in source order.
    pub finals: Vec<FinalStmt>,
}

/// `flow a -> b -> c`: a chain of at least two states.
#[derive(Debug, Clone, PartialEq)]
pub struct FlowStmt {
    pub states: Vec<Ident>,
    pub span: Span,
}

/// `final a, b`.
#[derive(Debug, Clone, PartialEq)]
pub struct FinalStmt {
    pub states: Vec<Ident>,
    pub span: Span,
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
    /// `None` for a service written without `at` (`spec/events.md` section 5).
    pub prefix: Option<StrLit>,
    pub endpoints: Vec<Endpoint>,
    /// `publishes A, B` lines, in source order.
    pub publishes: Vec<Publishes>,
    /// `subscribes C as "g" { .. }` blocks, in source order.
    pub subscribes: Vec<Subscribes>,
}

/// `publishes A, B`.
#[derive(Debug, Clone, PartialEq)]
pub struct Publishes {
    pub channels: Vec<TypeExpr>,
    pub span: Span,
}

/// `subscribes C as "group" { on E -> handler .. }`.
#[derive(Debug, Clone, PartialEq)]
pub struct Subscribes {
    pub channel: TypeExpr,
    pub group: StrLit,
    pub handlers: Vec<OnLine>,
    pub span: Span,
}

/// `on E -> handler`.
#[derive(Debug, Clone, PartialEq)]
pub struct OnLine {
    pub docs: Vec<Doc>,
    pub event: TypeExpr,
    pub handler: Ident,
    pub span: Span,
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
    /// The `{ ... }` block of endpoint rules (`spec/rules.md` section 9).
    pub block: Option<EndpointBlock>,
    /// From the method to the closing `}` of the block, or to the end of the responses.
    pub span: Span,
}

/// The statements of an endpoint block, in source order.
#[derive(Debug, Clone, PartialEq)]
pub struct EndpointBlock {
    pub stmts: Vec<Stmt>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    Fact(FactStmt),
    Require(RequireStmt),
    Ensure(EnsureStmt),
    Transition(TransitionStmt),
    Allow(AllowStmt),
}

impl Stmt {
    /// The first byte of the statement, docs and annotations included.
    #[must_use]
    pub fn start(&self) -> u32 {
        let (docs, anns, span) = match self {
            Stmt::Fact(f) => (&[][..], &[][..], f.span),
            Stmt::Require(r) => (&r.docs[..], &r.annotations[..], r.span),
            Stmt::Ensure(e) => (&e.docs[..], &e.annotations[..], e.span),
            Stmt::Transition(t) => (&[][..], &[][..], t.span),
            Stmt::Allow(a) => (&[][..], &[][..], a.span),
        };
        let doc = docs.first().map(|d| d.span.start);
        let ann = anns.first().map(|a| a.span.start);
        [doc, ann].into_iter().flatten().fold(span.start, u32::min)
    }
}

/// `allow Policy(args)` (`spec/rules.md` section 11.3).
#[derive(Debug, Clone, PartialEq)]
pub struct AllowStmt {
    pub policy: Ident,
    pub args: Vec<Expr>,
    pub span: Span,
}

/// `[docs] policy Name(param: T, ..) { clause* }` (`spec/rules.md` section 11.2).
#[derive(Debug, Clone, PartialEq)]
pub struct PolicyDecl {
    pub docs: Vec<Doc>,
    pub name: Ident,
    pub params: Vec<PolicyParam>,
    pub clauses: Vec<PolicyClause>,
    /// From the `policy` keyword to the closing `}`.
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PolicyParam {
    pub name: Ident,
    pub ty: TypeExpr,
}

/// `permit when expr [message "text"]` or `forbid when ...`.
#[derive(Debug, Clone, PartialEq)]
pub struct PolicyClause {
    pub forbid: bool,
    pub expr: Expr,
    pub message: Option<StrLit>,
    /// From the `permit` or `forbid` word to the end of the clause.
    pub span: Span,
}

/// `principal Type [as name]` (`spec/rules.md` section 11.1).
#[derive(Debug, Clone, PartialEq)]
pub struct PrincipalDecl {
    pub docs: Vec<Doc>,
    pub ty: TypeExpr,
    pub name: Option<Ident>,
    pub span: Span,
}

/// `transition fact.path: a, b -> c` (`spec/rules.md` section 10.2).
#[derive(Debug, Clone, PartialEq)]
pub struct TransitionStmt {
    /// The path: the fact first, then the fields down to the state field.
    pub path: Vec<Ident>,
    pub from: Vec<Ident>,
    pub to: Ident,
    pub span: Span,
}

/// `fact name[?]: T [= key]`
#[derive(Debug, Clone, PartialEq)]
pub struct FactStmt {
    pub name: Ident,
    pub optional: bool,
    pub ty: TypeExpr,
    pub key: Option<Expr>,
    pub span: Span,
}

/// `require [name:] expr [else Error [{ payload }]] [message "text"]`
#[derive(Debug, Clone, PartialEq)]
pub struct RequireStmt {
    pub docs: Vec<Doc>,
    /// Annotations after the docs and before `require` (`spec/rules.md` section 17.1).
    pub annotations: Vec<Annotation>,
    pub name: Option<Ident>,
    pub expr: Expr,
    pub otherwise: Option<Else>,
    pub message: Option<StrLit>,
    pub span: Span,
}

/// `else Error { field: expr, .. }`
#[derive(Debug, Clone, PartialEq)]
pub struct Else {
    pub error: TypeExpr,
    /// `None` when written without braces.
    pub payload: Option<Vec<PayloadField>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PayloadField {
    pub name: Ident,
    pub value: Expr,
}

/// `ensure [name:] expr [message "text"]`
#[derive(Debug, Clone, PartialEq)]
pub struct EnsureStmt {
    pub docs: Vec<Doc>,
    /// Annotations after the docs and before `ensure`.
    pub annotations: Vec<Annotation>,
    pub name: Option<Ident>,
    pub expr: Expr,
    pub message: Option<StrLit>,
    pub span: Span,
}

/// `example "name" { given.. [as v] [at "t"] call }` (`spec/rules.md` section 12).
#[derive(Debug, Clone, PartialEq)]
pub struct Example {
    pub docs: Vec<Doc>,
    pub name: StrLit,
    /// Empty in an event example.
    pub givens: Vec<Given>,
    pub principal: Option<ExampleAs>,
    pub at: Option<ExampleAt>,
    pub body: ExampleBody,
    /// From the `example` keyword to the closing `}`.
    pub span: Span,
}

/// What an example is about: one call, or one delivery of an event.
#[derive(Debug, Clone, PartialEq)]
pub enum ExampleBody {
    Call(ExampleCall),
    Event(EventExample),
}

impl ExampleBody {
    /// The call of a call example.
    #[must_use]
    pub fn as_call(&self) -> Option<&ExampleCall> {
        match self {
            ExampleBody::Call(c) => Some(c),
            ExampleBody::Event(_) => None,
        }
    }
}

/// `given event E { .. } [on C] expect ..` (`spec/events.md` section 12).
#[derive(Debug, Clone, PartialEq)]
pub struct EventExample {
    pub event: TypeExpr,
    /// The record, always an object value.
    pub value: Value,
    pub channel: Option<TypeExpr>,
    pub expects: Vec<Expectation>,
    /// From the `given` word to the last `expect` line.
    pub span: Span,
}

/// `expect S.handler [not] called` or `expect rejected`.
#[derive(Debug, Clone, PartialEq)]
pub struct Expectation {
    pub kind: ExpectKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExpectKind {
    Called {
        service: Ident,
        handler: Ident,
        not: bool,
    },
    Rejected,
}

/// `given name = value` (`None` is `absent`).
#[derive(Debug, Clone, PartialEq)]
pub struct Given {
    pub name: Ident,
    pub value: Option<Value>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExampleAs {
    pub value: Value,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExampleAt {
    pub time: StrLit,
    pub span: Span,
}

/// `[Service.]endpoint(arg: value, ..) [returns value] -> outcome`
#[derive(Debug, Clone, PartialEq)]
pub struct ExampleCall {
    /// `ifMatch 3` or `ifMatch any`, before the call.
    pub if_match: Option<IfMatchLine>,
    /// `key "k" ..`, before the call.
    pub key: Option<Box<KeyLine>>,
    pub service: Option<Ident>,
    pub endpoint: Ident,
    pub args: Vec<ExampleArgument>,
    pub returns: Option<Value>,
    pub outcome: Outcome,
    pub span: Span,
}

/// `ifMatch 3` or `ifMatch any` (`spec/rules.md` section 11.9).
#[derive(Debug, Clone, PartialEq)]
pub struct IfMatchLine {
    /// The version, or `None` for `any`.
    pub version: Option<i64>,
    pub span: Span,
}

/// `key "k" [stored outcome | other | running]` (`spec/rules.md` section 11.9).
#[derive(Debug, Clone, PartialEq)]
pub struct KeyLine {
    pub key: StrLit,
    pub store: KeyStoreLine,
    pub span: Span,
}

/// What the line says the idempotency store holds for the key.
#[derive(Debug, Clone, PartialEq)]
pub enum KeyStoreLine {
    /// Nothing: the line is the key alone.
    New,
    Stored(Outcome),
    Other,
    Running,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExampleArgument {
    pub name: Ident,
    pub value: Value,
}

/// `ok [value]` or `ErrorName [value]`.
#[derive(Debug, Clone, PartialEq)]
pub struct Outcome {
    /// `None` is `ok`.
    pub error: Option<TypeExpr>,
    pub value: Option<Value>,
    pub span: Span,
}

/// A value written in an example.
#[derive(Debug, Clone, PartialEq)]
pub struct Value {
    pub kind: ValueKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ValueKind {
    Bool(bool),
    Int(i64),
    /// A float keeps its source text: a `decimal` is that text.
    Float {
        value: f64,
        text: String,
    },
    Str(String),
    /// `.name`
    EnumLit(Ident),
    /// `a.b.c`: a given and the fields read from it.
    Path(Vec<Ident>),
    Object(Vec<ValueField>),
    List(Vec<Value>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ValueField {
    pub name: Ident,
    pub value: Value,
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

//! The v1 syntax tree (`internal/ast/ast.go`), reduced to what the migration
//! needs. Comments are carried along as plain lines.

/// A comment line to emit before an item (`//` markers already removed).
pub type Comments = Vec<String>;

#[derive(Debug, Clone, PartialEq)]
pub enum V1Type {
    Named(String),
    List(Box<V1Type>),
    Map(Box<V1Type>),
}

/// One alternative of a v1 field-level union (`"A" | "B"` or `string | int`).
#[derive(Debug, Clone, PartialEq)]
pub enum UnionAlt {
    Literal(String),
    Type(String),
}

/// A v1 default or example value, as written.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Str(String),
    Num(String),
    Ident(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    pub name: String,
    pub optional: bool,
    pub ty: V1Type,
    pub union: Vec<UnionAlt>,
    pub default: Option<Value>,
    pub example: Option<Value>,
    pub deprecated: Option<String>,
    pub unique: bool,
    pub index: bool,
    pub server_set: bool,
    pub relation: Option<String>,
    /// Annotations the v1 tool does not know, as written.
    pub unknown: Vec<String>,
    pub comments: Comments,
    pub trailing: Option<String>,
}

impl Field {
    pub fn new(name: String, ty: V1Type) -> Field {
        Field {
            name,
            optional: false,
            ty,
            union: Vec::new(),
            default: None,
            example: None,
            deprecated: None,
            unique: false,
            index: false,
            server_set: false,
            relation: None,
            unknown: Vec::new(),
            comments: Vec::new(),
            trailing: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Model {
    pub name: String,
    pub extends: Option<String>,
    pub description: Option<String>,
    pub fields: Vec<Field>,
    pub comments: Comments,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Enum {
    pub name: String,
    pub description: Option<String>,
    pub values: Vec<String>,
    pub comments: Comments,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Constant {
    pub name: String,
    pub ty: String,
    pub value: Value,
    pub comments: Comments,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConstantGroup {
    pub name: String,
    pub description: Option<String>,
    pub items: Vec<Constant>,
    pub comments: Comments,
}

/// A type position that may be a named type or an inline `{ ... }` object.
#[derive(Debug, Clone, PartialEq)]
pub enum TypeRef {
    Named(String),
    Inline(Vec<Field>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Output {
    pub ty: TypeRef,
    pub array: bool,
    pub status: Option<u16>,
}

/// A header declared in `headers: { ... }`.
#[derive(Debug, Clone, PartialEq)]
pub struct Header {
    pub name: String,
    pub optional: bool,
    pub ty: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Headers {
    Named(String),
    Inline(Vec<Header>),
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Action {
    pub name: String,
    pub description: Option<String>,
    pub method: String,
    pub path: String,
    pub input: Option<TypeRef>,
    pub output: Option<Output>,
    pub query: Option<TypeRef>,
    pub headers: Option<Headers>,
    pub stream: Option<String>,
    pub emit: Option<String>,
    pub middleware: Vec<String>,
    pub errors: Vec<(String, Option<u16>)>,
    pub deprecated: Option<String>,
    pub comments: Comments,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Module {
    pub name: String,
    pub description: Option<String>,
    pub prefix: Option<String>,
    pub base_url: Option<String>,
    pub actions: Vec<Action>,
    pub comments: Comments,
}

/// A top-level import, as written.
#[derive(Debug, Clone, PartialEq)]
pub struct Import {
    pub text: String,
    /// Glob imports (`@models/*`) cannot be resolved to one file.
    pub glob: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    Model(Model),
    Enum(Enum),
    Constants(ConstantGroup),
    Module(Module),
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct V1File {
    pub imports: Vec<Import>,
    /// Top-level `prefix:`, prepended to every module of the file.
    pub prefix: Option<String>,
    pub items: Vec<Item>,
    /// Comments after the last item.
    pub trailing: Comments,
}

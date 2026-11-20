//! Recursive-descent parser for the v1 grammar (`internal/parser/parser.go`).

use super::ast::{
    Action, Comments, Constant, ConstantGroup, Enum, Field, Header, Headers, Import, Item, Model,
    Module, Output, TypeRef, UnionAlt, V1File, V1Type, Value,
};
use super::lexer::{Comment, Tok, Token};

type R<T> = Result<T, String>;

pub(super) struct Parser {
    toks: Vec<Token>,
    pos: usize,
    comments: Vec<Comment>,
    cpos: usize,
}

impl Parser {
    pub fn new(toks: Vec<Token>, comments: Vec<Comment>) -> Parser {
        Parser {
            toks,
            pos: 0,
            comments,
            cpos: 0,
        }
    }

    // ---- token helpers --------------------------------------------------

    fn peek_at(&self, n: usize) -> &Tok {
        let i = (self.pos + n).min(self.toks.len().saturating_sub(1));
        self.toks.get(i).map_or(&Tok::Eof, |t| &t.tok)
    }

    fn peek(&self) -> &Tok {
        self.peek_at(0)
    }

    fn line(&self) -> usize {
        let i = self.pos.min(self.toks.len().saturating_sub(1));
        self.toks.get(i).map_or(0, |t| t.line)
    }

    fn prev_line(&self) -> usize {
        self.toks
            .get(self.pos.saturating_sub(1))
            .map_or(0, |t| t.line)
    }

    fn advance(&mut self) -> Tok {
        let t = self.peek().clone();
        if self.pos < self.toks.len().saturating_sub(1) {
            self.pos += 1;
        }
        t
    }

    fn err<T>(&self, msg: impl Into<String>) -> R<T> {
        Err(format!("line {}: {}", self.line(), msg.into()))
    }

    fn at_word(&self, w: &str) -> bool {
        matches!(self.peek(), Tok::Ident(s) if s == w)
    }

    fn eat(&mut self, t: &Tok) -> bool {
        if self.peek() == t {
            self.advance();
            true
        } else {
            false
        }
    }

    fn expect(&mut self, t: &Tok) -> R<()> {
        if self.eat(t) {
            Ok(())
        } else {
            self.err(format!(
                "expected {}, found {}",
                t.describe(),
                self.peek().describe()
            ))
        }
    }

    fn ident(&mut self, what: &str) -> R<String> {
        match self.peek().clone() {
            Tok::Ident(s) => {
                self.advance();
                Ok(s)
            }
            other => self.err(format!("expected {what}, found {}", other.describe())),
        }
    }

    fn string(&mut self, what: &str) -> R<String> {
        match self.peek().clone() {
            Tok::Str(s) => {
                self.advance();
                Ok(s)
            }
            other => self.err(format!("expected {what}, found {}", other.describe())),
        }
    }

    fn path(&mut self, what: &str) -> R<String> {
        match self.peek().clone() {
            Tok::Path(s) => {
                self.advance();
                Ok(s)
            }
            other => self.err(format!("expected {what}, found {}", other.describe())),
        }
    }

    fn value(&mut self) -> R<Value> {
        match self.advance() {
            Tok::Str(s) => Ok(Value::Str(s)),
            Tok::Num(n) => Ok(Value::Num(n)),
            Tok::Ident(i) => Ok(Value::Ident(i)),
            other => self.err(format!("expected a value, found {}", other.describe())),
        }
    }

    // ---- comments -------------------------------------------------------

    /// Takes every unconsumed comment that starts on a line before `line`.
    fn take_comments(&mut self, line: usize) -> Comments {
        self.take_comments_through(line.saturating_sub(1))
    }

    /// Takes every unconsumed comment that starts on or before `line`.
    fn take_comments_through(&mut self, line: usize) -> Comments {
        let mut out = Vec::new();
        while let Some(c) = self.comments.get(self.cpos) {
            if c.line > line {
                break;
            }
            out.extend(comment_lines(&c.text));
            self.cpos += 1;
        }
        out
    }

    /// A single-line comment on the line of the token just consumed.
    fn take_trailing(&mut self) -> Option<String> {
        let line = self.prev_line();
        let c = self.comments.get(self.cpos)?;
        if c.line != line || c.text.contains('\n') {
            return None;
        }
        let text = c.text.clone();
        self.cpos += 1;
        Some(text)
    }

    // ---- file -----------------------------------------------------------

    pub fn parse_file(&mut self) -> R<V1File> {
        let mut file = V1File::default();
        while !matches!(self.peek(), Tok::Eof) {
            let pending = self.take_comments(self.line());
            let Tok::Ident(word) = self.peek().clone() else {
                return self.err(format!("unexpected token {}", self.peek().describe()));
            };
            match word.as_str() {
                "import" => file.imports.push(self.import()?),
                "from" => file.imports.push(self.import_from()?),
                "prefix" if matches!(self.peek_at(1), Tok::Colon) => {
                    self.advance();
                    self.advance();
                    file.prefix = Some(self.path("a prefix path")?);
                }
                "model" => file.items.push(Item::Model(self.model(pending)?)),
                "enum" => file.items.push(Item::Enum(self.enum_decl(pending)?)),
                "constants" | "constant" => {
                    file.items.push(Item::Constants(self.constants(pending)?))
                }
                "module" => file.items.push(Item::Module(self.module(pending)?)),
                other => return self.err(format!("unexpected token `{other}`")),
            }
        }
        file.trailing = self.take_comments_through(usize::MAX);
        Ok(file)
    }

    // ---- imports --------------------------------------------------------

    fn import(&mut self) -> R<Import> {
        self.advance();
        let text = match self.peek().clone() {
            Tok::At => {
                self.advance();
                let alias = self.ident("an import alias")?;
                match self.peek().clone() {
                    Tok::Path(p) => {
                        self.advance();
                        format!("{alias}{p}")
                    }
                    _ => format!("{alias}/*"),
                }
            }
            Tok::Path(p) => {
                self.advance();
                p.trim_start_matches('/').to_string()
            }
            Tok::Ident(a) => {
                self.advance();
                match self.peek().clone() {
                    Tok::Path(p) => {
                        self.advance();
                        format!("{a}{p}")
                    }
                    _ => format!("{a}/*"),
                }
            }
            Tok::Str(s) => {
                self.advance();
                s.trim_start_matches("./")
                    .trim_start_matches("../")
                    .to_string()
            }
            other => {
                return self.err(format!(
                    "expected an import path, found {}",
                    other.describe()
                ));
            }
        };
        let text = text.trim_end_matches(".veld").to_string();
        let glob = text.ends_with('*');
        Ok(Import { text, glob })
    }

    fn import_from(&mut self) -> R<Import> {
        self.advance();
        let folder = match self.advance() {
            Tok::At => self.ident("a folder alias")?,
            Tok::Path(p) => p.trim_start_matches('/').to_string(),
            Tok::Ident(a) => a,
            other => return self.err(format!("expected a folder, found {}", other.describe())),
        };
        if !self.at_word("import") {
            return self.err("expected `import`");
        }
        self.advance();
        if self.eat(&Tok::Star) {
            return Ok(Import {
                text: format!("{folder}/*"),
                glob: true,
            });
        }
        let first = self.ident("an imported name")?;
        while self.eat(&Tok::Comma) {
            self.ident("an imported name")?;
        }
        Ok(Import {
            text: format!("{folder}/{first}"),
            glob: false,
        })
    }

    // ---- enums and constants -------------------------------------------

    fn description(&mut self) -> R<Option<String>> {
        if self.at_word("description")
            && matches!(self.peek_at(1), Tok::Colon)
            && matches!(self.peek_at(2), Tok::Str(_))
        {
            self.advance();
            self.advance();
            return Ok(Some(self.string("a description")?));
        }
        Ok(None)
    }

    fn enum_decl(&mut self, mut comments: Comments) -> R<Enum> {
        self.advance();
        let name = self.ident("an enum name")?;
        self.expect(&Tok::LBrace)?;
        let description = self.description()?;
        let mut values = Vec::new();
        while !matches!(self.peek(), Tok::RBrace | Tok::Eof) {
            values.push(self.ident("an enum value")?);
            self.eat(&Tok::Comma);
        }
        self.expect(&Tok::RBrace)?;
        comments.extend(self.take_comments_through(self.prev_line()));
        Ok(Enum {
            name,
            description,
            values,
            comments,
        })
    }

    fn constants(&mut self, mut comments: Comments) -> R<ConstantGroup> {
        self.advance();
        let name = self.ident("a constants group name")?;
        self.expect(&Tok::LBrace)?;
        let description = self.description()?;
        let mut items = Vec::new();
        while !matches!(self.peek(), Tok::RBrace | Tok::Eof) {
            let c = self.take_comments(self.line());
            let cname = self.ident("a constant name")?;
            self.expect(&Tok::Colon)?;
            let ty = self.ident("a constant type")?;
            self.expect(&Tok::Eq)?;
            let value = self.value()?;
            items.push(Constant {
                name: cname,
                ty,
                value,
                comments: c,
            });
        }
        self.expect(&Tok::RBrace)?;
        comments.extend(self.take_comments_through(self.prev_line()));
        Ok(ConstantGroup {
            name,
            description,
            items,
            comments,
        })
    }

    // ---- models ---------------------------------------------------------

    fn model(&mut self, mut comments: Comments) -> R<Model> {
        self.advance();
        let name = self.ident("a model name")?;
        let extends = if self.at_word("extends") {
            self.advance();
            Some(self.ident("a parent model")?)
        } else {
            None
        };
        self.expect(&Tok::LBrace)?;
        let description = self.description()?;
        let mut fields = Vec::new();
        while !matches!(self.peek(), Tok::RBrace | Tok::Eof) {
            fields.push(self.field()?);
        }
        self.expect(&Tok::RBrace)?;
        comments.extend(self.take_comments_through(self.prev_line()));
        Ok(Model {
            name,
            extends,
            description,
            fields,
            comments,
        })
    }

    fn field(&mut self) -> R<Field> {
        let comments = self.take_comments(self.line());
        let name = self.ident("a field name")?;
        let optional = self.eat(&Tok::Question);
        self.expect(&Tok::Colon)?;
        let (ty, union) = self.field_type()?;
        let mut f = Field::new(name, ty);
        f.optional = optional;
        f.union = union;
        f.comments = comments;
        if self.eat(&Tok::Eq) {
            f.default = Some(self.value()?);
        }
        while self.eat(&Tok::At) {
            self.field_annotation(&mut f)?;
        }
        f.trailing = self.take_trailing();
        Ok(f)
    }

    fn field_type(&mut self) -> R<(V1Type, Vec<UnionAlt>)> {
        let mut union = Vec::new();
        let mut ty = match self.advance() {
            Tok::Ident(n) => self.generic_or_named(n)?,
            Tok::Str(s) => {
                union.push(UnionAlt::Literal(s));
                V1Type::Named("string".into())
            }
            other => return self.err(format!("expected a type, found {}", other.describe())),
        };
        while matches!(self.peek(), Tok::LBracket) && matches!(self.peek_at(1), Tok::RBracket) {
            self.advance();
            self.advance();
            ty = V1Type::List(Box::new(ty));
        }
        if matches!(self.peek(), Tok::Pipe) {
            if let (true, V1Type::Named(n)) = (union.is_empty(), &ty) {
                union.push(UnionAlt::Type(n.clone()));
            }
            while self.eat(&Tok::Pipe) {
                match self.advance() {
                    Tok::Ident(n) => union.push(UnionAlt::Type(n)),
                    Tok::Str(s) => union.push(UnionAlt::Literal(s)),
                    other => {
                        return self.err(format!(
                            "expected a type after `|`, found {}",
                            other.describe()
                        ));
                    }
                }
            }
        }
        Ok((ty, union))
    }

    fn generic_or_named(&mut self, name: String) -> R<V1Type> {
        if !matches!(self.peek(), Tok::Lt) || !(name == "List" || name == "Map") {
            return Ok(V1Type::Named(name));
        }
        self.advance();
        if name == "List" {
            let inner = self.ident("an element type")?;
            self.expect(&Tok::Gt)?;
            return Ok(V1Type::List(Box::new(V1Type::Named(inner))));
        }
        let key = self.ident("a map key type")?;
        if key != "string" {
            return self.err(format!("Map key type must be `string`, got `{key}`"));
        }
        self.expect(&Tok::Comma)?;
        let value = self.ident("a map value type")?;
        self.expect(&Tok::Gt)?;
        Ok(V1Type::Map(Box::new(V1Type::Named(value))))
    }

    fn paren_value(&mut self) -> R<Value> {
        self.expect(&Tok::LParen)?;
        let v = self.value()?;
        self.expect(&Tok::RParen)?;
        Ok(v)
    }

    fn field_annotation(&mut self, f: &mut Field) -> R<()> {
        let name = self.ident("an annotation name")?;
        match name.as_str() {
            "default" => f.default = Some(self.paren_value()?),
            "example" => f.example = Some(self.paren_value()?),
            "deprecated" => f.deprecated = Some(self.deprecated_message()?),
            "unique" => f.unique = true,
            "index" => f.index = true,
            "serverSet" => f.server_set = true,
            "relation" => {
                self.expect(&Tok::LParen)?;
                f.relation = Some(self.ident("a model name")?);
                self.expect(&Tok::RParen)?;
            }
            _ => {
                let args = self.skip_parens();
                f.unknown.push(format!("@{name}{args}"));
            }
        }
        Ok(())
    }

    fn deprecated_message(&mut self) -> R<String> {
        if self.eat(&Tok::LParen) {
            let m = self.string("a deprecation message")?;
            self.expect(&Tok::RParen)?;
            return Ok(m);
        }
        self.string("a quoted deprecation message")
    }

    /// Skips an optional parenthesised argument list and returns its text.
    fn skip_parens(&mut self) -> String {
        if !matches!(self.peek(), Tok::LParen) {
            return String::new();
        }
        let mut text = String::from("(");
        self.advance();
        let mut depth = 1;
        while depth > 0 && !matches!(self.peek(), Tok::Eof) {
            match self.advance() {
                Tok::LParen => {
                    depth += 1;
                    text.push('(');
                }
                Tok::RParen => {
                    depth -= 1;
                    text.push(')');
                }
                Tok::Str(s) => text.push_str(&format!("\"{s}\"")),
                Tok::Ident(s) | Tok::Num(s) | Tok::Path(s) => text.push_str(&s),
                other => text.push_str(other.describe().trim_matches('`')),
            }
        }
        text
    }

    // ---- modules and actions -------------------------------------------

    fn module(&mut self, mut comments: Comments) -> R<Module> {
        self.advance();
        let name = self.ident("a module name")?;
        self.expect(&Tok::LBrace)?;
        let mut m = Module {
            name,
            ..Module::default()
        };
        while !matches!(self.peek(), Tok::RBrace | Tok::Eof) {
            self.module_item(&mut m)?;
        }
        self.expect(&Tok::RBrace)?;
        comments.extend(self.take_comments_through(self.prev_line()));
        m.comments = comments;
        Ok(m)
    }

    fn module_item(&mut self, m: &mut Module) -> R<()> {
        if let Some(d) = self.description()? {
            m.description = Some(d);
        } else if self.at_word("prefix") {
            self.advance();
            self.expect(&Tok::Colon)?;
            m.prefix = Some(self.path("a module prefix")?);
        } else if self.at_word("baseUrl") && matches!(self.peek_at(1), Tok::Colon) {
            self.advance();
            self.advance();
            m.base_url = Some(match self.advance() {
                Tok::Ident(s) | Tok::Str(s) | Tok::Path(s) | Tok::Num(s) => s,
                other => other.describe(),
            });
        } else if self.at_word("action") {
            let a = self.action()?;
            m.actions.push(a);
        } else {
            return self.err(format!(
                "unexpected token {} in module body",
                self.peek().describe()
            ));
        }
        Ok(())
    }

    fn action(&mut self) -> R<Action> {
        let mut comments = self.take_comments(self.line());
        self.advance();
        let name = self.ident("an action name")?;
        self.expect(&Tok::LBrace)?;
        let mut a = Action {
            name,
            ..Action::default()
        };
        while !matches!(self.peek(), Tok::RBrace | Tok::Eof) {
            self.action_item(&mut a)?;
        }
        self.expect(&Tok::RBrace)?;
        comments.extend(self.take_comments_through(self.prev_line()));
        a.comments = comments;
        if a.method.is_empty() || a.path.is_empty() {
            return self.err(format!(
                "action `{}` needs both `method` and `path`",
                a.name
            ));
        }
        Ok(a)
    }

    fn key(&mut self) -> R<String> {
        let k = self.ident("a property name")?;
        self.expect(&Tok::Colon)?;
        Ok(k)
    }

    fn action_item(&mut self, a: &mut Action) -> R<()> {
        if self.eat(&Tok::At) {
            let n = self.ident("an annotation name")?;
            if n != "deprecated" {
                return self.err(format!("unknown action annotation `@{n}`"));
            }
            a.deprecated = Some(self.deprecated_message()?);
            return Ok(());
        }
        let key = self.key()?;
        match key.as_str() {
            "method" => a.method = self.ident("an HTTP method")?,
            "path" => a.path = self.path("a path")?,
            "description" => a.description = Some(self.string("a description")?),
            "input" => a.input = Some(self.type_ref()?),
            "query" => a.query = Some(self.type_ref()?),
            "output" | "response" => a.output = Some(self.output()?),
            "headers" => a.headers = Some(self.headers()?),
            "stream" => a.stream = Some(self.ident("a stream type")?),
            "emit" => a.emit = Some(self.ident("an emit type")?),
            "middleware" => a.middleware = self.middleware()?,
            "errors" => a.errors = self.errors()?,
            other => return self.err(format!("unknown action property `{other}`")),
        }
        Ok(())
    }

    fn inline_fields(&mut self) -> R<Vec<Field>> {
        self.expect(&Tok::LBrace)?;
        let mut fields = Vec::new();
        while !matches!(self.peek(), Tok::RBrace | Tok::Eof) {
            fields.push(self.field()?);
            self.eat(&Tok::Comma);
        }
        self.expect(&Tok::RBrace)?;
        Ok(fields)
    }

    fn type_ref(&mut self) -> R<TypeRef> {
        if matches!(self.peek(), Tok::LBrace) {
            return Ok(TypeRef::Inline(self.inline_fields()?));
        }
        Ok(TypeRef::Named(self.ident("a type name")?))
    }

    fn output(&mut self) -> R<Output> {
        let ty = self.type_ref()?;
        let array =
            matches!(self.peek(), Tok::LBracket) && matches!(self.peek_at(1), Tok::RBracket);
        if array {
            self.advance();
            self.advance();
        }
        let mut status = None;
        if self.eat(&Tok::Colon) {
            let Tok::Num(n) = self.advance() else {
                return self.err("expected a status code after `:`");
            };
            status = n.parse::<u16>().ok();
        }
        Ok(Output { ty, array, status })
    }

    fn headers(&mut self) -> R<Headers> {
        if !self.eat(&Tok::LBrace) {
            return Ok(Headers::Named(self.ident("a headers model")?));
        }
        let mut hs = Vec::new();
        while !matches!(self.peek(), Tok::RBrace | Tok::Eof) {
            let name = match self.advance() {
                Tok::Str(s) | Tok::Ident(s) => s,
                other => {
                    return self.err(format!(
                        "expected a header name, found {}",
                        other.describe()
                    ));
                }
            };
            let optional = self.eat(&Tok::Question);
            self.expect(&Tok::Colon)?;
            let ty = self.ident("a header type")?;
            hs.push(Header { name, optional, ty });
            self.eat(&Tok::Comma);
        }
        self.expect(&Tok::RBrace)?;
        Ok(Headers::Inline(hs))
    }

    fn middleware(&mut self) -> R<Vec<String>> {
        if !self.eat(&Tok::LBracket) {
            return Ok(vec![self.ident("a middleware name")?]);
        }
        let mut out = Vec::new();
        while !matches!(self.peek(), Tok::RBracket | Tok::Eof) {
            out.push(self.ident("a middleware name")?);
            self.eat(&Tok::Comma);
        }
        self.expect(&Tok::RBracket)?;
        Ok(out)
    }

    fn errors(&mut self) -> R<Vec<(String, Option<u16>)>> {
        self.expect(&Tok::LBracket)?;
        let mut out = Vec::new();
        while !matches!(self.peek(), Tok::RBracket | Tok::Eof) {
            let name = self.ident("an error name")?;
            let mut status = None;
            if self.eat(&Tok::Colon) {
                let Tok::Num(n) = self.advance() else {
                    return self.err("expected a status code after `:`");
                };
                status = n.parse::<u16>().ok();
            }
            out.push((name, status));
            self.eat(&Tok::Comma);
        }
        self.expect(&Tok::RBracket)?;
        Ok(out)
    }
}

/// A comment as lines to print after `//`. Block comments span several lines.
fn comment_lines(text: &str) -> Vec<String> {
    if !text.contains('\n') {
        return vec![text.to_string()];
    }
    text.lines()
        .map(|l| format!(" {}", l.trim()))
        .filter(|l| !l.trim().is_empty())
        .collect()
}

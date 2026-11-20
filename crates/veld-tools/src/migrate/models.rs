//! Models, enums and constants.

use veld_compiler::language::is_reserved;

use super::conv::{FileConv, comment_block, doc_lines, scalar, todo_comment};
use super::names::{capitalize, is_ident, sanitize_ident};
use super::v1::ast::{Constant, ConstantGroup, Enum, Field, Model, UnionAlt};
use crate::fmt::text::quote;

impl FileConv<'_> {
    /// The v2 text of a v1 model.
    pub fn model(&mut self, m: &Model) -> String {
        self.locals.insert(m.name.clone());
        self.model_text(&m.name, m.extends.as_deref(), &m.description, &m.fields, &m.comments)
    }

    /// Shared by named and inline (synthetic) models.
    pub fn model_text(
        &mut self,
        name: &str,
        extends: Option<&str>,
        description: &Option<String>,
        fields: &[Field],
        comments: &[String],
    ) -> String {
        let mut lines = comment_block(comments);
        lines.extend(doc_lines(description));
        let ext = extends.map_or_else(String::new, |p| format!(" extends {}", self.type_name(p)));
        lines.push(format!("model {name}{ext} {{"));
        for f in fields {
            for l in self.field_lines(name, f) {
                lines.push(format!("  {l}"));
            }
        }
        lines.push("}".to_string());
        lines.join("\n")
    }

    /// Comment lines followed by the field line.
    fn field_lines(&mut self, model: &str, f: &Field) -> Vec<String> {
        let mut lines = comment_block(&f.comments);
        let ty = self.field_type(model, f, &mut lines);
        let (name, wire) = self.field_name(model, &f.name);
        let mut line = format!("{name}{}: {ty}", if f.optional && f.default.is_none() { "?" } else { "" });
        if f.optional && f.default.is_some() {
            self.warn(format!(
                "{model}.{}: optional field with a default; v2 fields with a default are never optional, kept the default and dropped `?`",
                f.name
            ));
        }
        for a in self.field_annotations(model, f, &mut lines) {
            line.push(' ');
            line.push_str(&a);
        }
        if let Some(w) = wire {
            line.push_str(&format!(" @wire({})", quote(&w)));
        }
        if let Some(t) = &f.trailing {
            line.push_str(&format!(" //{t}"));
        }
        lines.push(line);
        lines
    }

    /// The v2 field name, and the wire name when the source name is reserved.
    fn field_name(&mut self, model: &str, name: &str) -> (String, Option<String>) {
        if !is_reserved(name) {
            return (name.to_string(), None);
        }
        let new = format!("{name}_");
        self.warn(format!(
            "{model}.{name}: `{name}` is a reserved word in v2; renamed to `{new}` with @wire(\"{name}\")"
        ));
        (new, Some(name.to_string()))
    }

    fn field_type(&mut self, model: &str, f: &Field, lines: &mut Vec<String>) -> String {
        if f.union.is_empty() {
            return self.type_text(&f.ty);
        }
        let literals: Vec<&str> = f
            .union
            .iter()
            .filter_map(|a| if let UnionAlt::Literal(s) = a { Some(s.as_str()) } else { None })
            .collect();
        let only_literals = f.union.iter().all(|a| match a {
            UnionAlt::Literal(_) => true,
            UnionAlt::Type(t) => scalar(t) == Some("string"),
        });
        if only_literals && !literals.is_empty() {
            return self.union_enum(model, &f.name, &literals);
        }
        let msg = format!("{model}.{}: field-level union of types has no v2 form; typed as json", f.name);
        self.warn(&msg);
        lines.push(todo_comment(&format!("{} was a union of types; narrow the json type", f.name)));
        "json".to_string()
    }

    /// A union of string literals becomes a generated enum.
    fn union_enum(&mut self, model: &str, field: &str, literals: &[&str]) -> String {
        let name = format!("{model}{}", capitalize(field));
        let variants: Vec<String> = literals.iter().map(|l| variant_text(l)).collect();
        self.generated.push(format!("enum {name} {{ {} }}", variants.join(", ")));
        self.locals.insert(name.clone());
        self.warn(format!(
            "{model}.{field}: union of string literals became the generated enum `{name}`"
        ));
        name
    }

    /// `= default` is handled here too, because it is part of the field line.
    fn field_annotations(&mut self, model: &str, f: &Field, lines: &mut Vec<String>) -> Vec<String> {
        let mut out = Vec::new();
        if let Some(d) = &f.default {
            match self.literal(d, &f.ty) {
                Some(l) => out.insert(0, format!("= {l}")),
                None => self.dropped(model, f, "default", lines),
            }
        }
        if let Some(m) = &f.deprecated {
            out.push(format!("@deprecated({})", quote(m)));
        }
        if let Some(e) = &f.example {
            match self.literal(e, &f.ty) {
                Some(l) => out.push(format!("@example({l})")),
                None => self.dropped(model, f, "example", lines),
            }
        }
        if f.unique {
            out.push("@unique".into());
        }
        if f.index {
            out.push("@index".into());
        }
        if f.server_set {
            out.push("@readonly".into());
        }
        self.unmapped(model, f, lines);
        out
    }

    fn dropped(&mut self, model: &str, f: &Field, what: &str, lines: &mut Vec<String>) {
        self.warn(format!("{model}.{}: the {what} value does not fit type and was dropped", f.name));
        lines.push(todo_comment(&format!("{what} of `{}` could not be converted", f.name)));
    }

    fn unmapped(&mut self, model: &str, f: &Field, lines: &mut Vec<String>) {
        if let Some(r) = &f.relation {
            self.warn(format!("{model}.{}: @relation({r}) has no v2 equivalent and was dropped", f.name));
            lines.push(todo_comment(&format!("@relation({r}) on `{}` has no v2 equivalent", f.name)));
        }
        for u in &f.unknown {
            self.warn(format!("{model}.{}: unknown annotation {u} was dropped", f.name));
            lines.push(todo_comment(&format!("annotation {u} on `{}` was dropped", f.name)));
        }
    }

    pub fn enum_decl(&mut self, e: &Enum) -> String {
        self.locals.insert(e.name.clone());
        let mut lines = comment_block(&e.comments);
        lines.extend(doc_lines(&e.description));
        let variants: Vec<String> = e.values.iter().map(|v| variant_text(v)).collect();
        lines.push(if variants.is_empty() {
            format!("enum {} {{}}", e.name)
        } else {
            format!("enum {} {{ {} }}", e.name, variants.join(", "))
        });
        lines.join("\n")
    }

    pub fn constants(&mut self, g: &ConstantGroup) -> String {
        let mut lines = comment_block(&g.comments);
        lines.extend(doc_lines(&g.description));
        self.warn(format!(
            "constants group `{}` was flattened; v2 constants have no group",
            g.name
        ));
        lines.push(format!("// v1 constants group `{}`", g.name));
        for c in &g.items {
            lines.extend(self.constant(&g.name, c));
        }
        lines.join("\n")
    }

    fn constant(&mut self, group: &str, c: &Constant) -> Vec<String> {
        let mut lines = comment_block(&c.comments);
        let name = if self.ctx.dup_consts.contains(&c.name) { format!("{group}_{}", c.name) } else { c.name.clone() };
        let ty = crate::migrate::v1::ast::V1Type::Named(c.ty.clone());
        let lit = scalar(&c.ty).and_then(|_| self.literal(&c.value, &ty));
        match (scalar(&c.ty), lit) {
            (Some(s), Some(l)) => lines.push(format!("const {name}: {s} = {l}")),
            _ => {
                self.warn(format!("constant {group}.{} could not be converted", c.name));
                lines.push(todo_comment(&format!("constant `{}` ({}) could not be converted", c.name, c.ty)));
            }
        }
        lines
    }
}

/// An enum variant, with a wire value when the source name is not a plain identifier.
fn variant_text(v: &str) -> String {
    if is_ident(v) && !is_reserved(v) {
        return v.to_string();
    }
    format!("{} = {}", sanitize_ident(v), quote(v))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variants_with_odd_names_get_wire_values() {
        assert_eq!(variant_text("pending"), "pending");
        assert_eq!(variant_text("in-progress"), "in_progress = \"in-progress\"");
        assert_eq!(variant_text("error"), "error_ = \"error\"");
    }
}

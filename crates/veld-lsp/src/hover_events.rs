//! Hover on a channel (`spec/events.md` section 15): its topic, its key and the signatures of the
//! events it carries.
//!
//! A channel is not a declaration of the index, so the name is found by its position: the name
//! after `channel`, a name on a `publishes` or `subscribes` line, or the channel an event example
//! names after `on`.

use veld_compiler::FileId;
use veld_compiler::Span;
use veld_compiler::index::Index;
use veld_ir::{Channel, Ir, Type};

use crate::hover::HoverText;
use crate::workspace::Analysis;

fn is_word(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// The word under `offset` and its span.
fn word_at(text: &str, file: FileId, offset: usize) -> Option<(&str, Span)> {
    let start = text
        .get(..offset)?
        .rfind(|c: char| !is_word(c))
        .map_or(0, |i| i + 1);
    let end = offset + text.get(offset..)?.find(|c: char| !is_word(c))?;
    let word = text.get(start..end).filter(|w| !w.is_empty())?;
    Some((word, Span::new(file, start, end)))
}

/// True when the word that starts at `start` stands where a channel name stands.
fn in_channel_position(text: &str, start: usize) -> bool {
    let line_start = text
        .get(..start)
        .and_then(|t| t.rfind('\n'))
        .map_or(0, |i| i + 1);
    let before = text.get(line_start..start).unwrap_or("").trim();
    let line_end = text
        .get(start..)
        .and_then(|t| t.find('\n'))
        .map_or(text.len(), |i| start + i);
    let line = text.get(line_start..line_end).unwrap_or("").trim_start();
    let first = before.split_whitespace().next().unwrap_or("");
    match first {
        "channel" | "subscribes" => before == first,
        "publishes" => true,
        "given" => line.contains("event") && before.ends_with("on"),
        _ => false,
    }
}

/// The IR channel called `name` in the package of `file`.
fn channel_of<'a>(ix: &'a Index, ir: &'a Ir, file: FileId, name: &str) -> Option<&'a Channel> {
    let pkg = &ix.file(file)?.package;
    let own = ir
        .channels
        .iter()
        .find(|c| c.name == name && ir.package(c.package).name == *pkg);
    own.or_else(|| ir.channels.iter().find(|c| c.name == name))
}

fn type_word(t: &Type) -> String {
    match t {
        Type::Scalar(s) => s.name().to_owned(),
        Type::List(_) | Type::Map(_) | Type::Page(_) | Type::Decl(_) => "value".to_owned(),
    }
}

fn describe(ir: &Ir, c: &Channel) -> String {
    let mut out = format!("```veld\nchannel {}\n```\n", c.name);
    if let Some(d) = &c.docs {
        out.push_str(d);
        out.push_str("\n\n");
    }
    out.push_str(&format!("- topic: `{}`\n", c.topic));
    match &c.key {
        Some(k) => out.push_str(&format!("- key: `{}` ({})\n", k.name, type_word(&k.ty))),
        None => out.push_str("- key: none (no promise about order)\n"),
    }
    out.push_str("- carries:\n");
    for id in &c.events {
        let d = ir.decl(*id);
        out.push_str(&format!("  - `{}`, sig: `{}`\n", d.name, d.sig));
    }
    out
}

/// The hover for a channel name under `offset`, `None` when the word is not one.
#[must_use]
pub fn channel_hover(a: &Analysis, file: FileId, offset: u32) -> Option<HoverText> {
    let ix = &a.index;
    let text = ix.sources.text(file)?;
    let (word, span) = word_at(text, file, offset as usize)?;
    if !in_channel_position(text, span.start as usize) {
        return None;
    }
    let ir = ix.ir.as_ref()?;
    let channel = channel_of(ix, ir, file, word)?;
    Some(HoverText {
        markdown: describe(ir, channel),
        span,
    })
}

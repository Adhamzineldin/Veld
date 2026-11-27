//! Completion for events (`spec/events.md` section 15): event names after `carries`, `on` and
//! `given event`, and channel names after `publishes`, `subscribes` and the `on` of an event
//! example.
//!
//! The line being typed is rarely valid yet, so the position is read from the text of the cursor
//! line before the partial word, as the rest of completion reads its context.

use lsp_types::{CompletionItem, CompletionItemKind};
use veld_compiler::FileId;
use veld_compiler::syntax::ast::{ChannelDecl, DeclKind, TypeExprKind};

use crate::complete::{Cx, TypePos, item};

/// What the cursor line asks for.
#[derive(Debug, PartialEq, Eq)]
enum Wanted {
    /// Events, without the ones already listed on the line.
    Events(Vec<String>),
    /// The events of the channel the enclosing `subscribes` names.
    HandledEvents,
    Channels,
}

/// The identifiers of `list` (`A, B, `) when it is only a comma-separated list of names that
/// ends where a name is expected.
fn names_before_cursor(list: &str) -> Option<Vec<String>> {
    let ok = list
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | ',' | '.') || c.is_whitespace());
    let t = list.trim_end();
    if !ok || !(t.is_empty() || t.ends_with(',')) {
        return None;
    }
    Some(
        t.split(',')
            .map(str::trim)
            .filter(|n| !n.is_empty())
            .map(str::to_owned)
            .collect(),
    )
}

/// The word that opens the line and the rest of it, when a space follows the word.
fn first_word(line: &str) -> Option<(&str, &str)> {
    let t = line.trim_start();
    let end = t.find(char::is_whitespace)?;
    Some((&t[..end], &t[end..]))
}

fn wanted(line: &str) -> Option<Wanted> {
    let (word, rest) = first_word(line)?;
    match word {
        "carries" => names_before_cursor(rest).map(Wanted::Events),
        "publishes" => names_before_cursor(rest).map(|_| Wanted::Channels),
        "subscribes" => rest.trim().is_empty().then_some(Wanted::Channels),
        "on" => rest.trim().is_empty().then_some(Wanted::HandledEvents),
        "given" => given_event(rest),
        _ => None,
    }
}

/// `given event |` wants an event; `given event X { ... } on |` wants a channel.
fn given_event(rest: &str) -> Option<Wanted> {
    let after = rest.trim_start().strip_prefix("event")?;
    if !after.starts_with(char::is_whitespace) {
        return None;
    }
    if after.trim().is_empty() {
        return Some(Wanted::Events(Vec::new()));
    }
    let close = after.rfind('}')?;
    let tail = after.get(close + 1..)?.trim();
    (tail == "on").then_some(Wanted::Channels)
}

impl Cx<'_> {
    /// The completion for an event or a channel position, `None` when the cursor is elsewhere.
    pub(crate) fn event_position(&self) -> Option<Vec<CompletionItem>> {
        match wanted(&self.ctx.line)? {
            Wanted::Events(exclude) => {
                let mut v = self.types(TypePos::Event);
                v.retain(|i| !exclude.contains(&i.label));
                Some(v)
            }
            Wanted::HandledEvents => Some(self.handled_events()),
            Wanted::Channels => Some(self.channels()),
        }
    }

    /// The events of the channel of the nearest `subscribes` above the cursor; every visible
    /// event when that channel is not known.
    fn handled_events(&self) -> Vec<CompletionItem> {
        let mut all = self.types(TypePos::Event);
        let carried = self
            .subscribed_channel()
            .and_then(|name| self.channel_ast(&name))
            .map(carried_names);
        if let Some(names) = carried.filter(|n| !n.is_empty()) {
            all.retain(|i| names.contains(&i.label));
        }
        all
    }

    /// The channel named by the last `subscribes` line before the cursor.
    fn subscribed_channel(&self) -> Option<String> {
        self.prefix.lines().rev().find_map(|l| {
            let (word, rest) = first_word(l)?;
            (word == "subscribes")
                .then(|| rest.split_whitespace().next().map(str::to_owned))
                .flatten()
        })
    }

    /// Channels declared in this file or imported into it, in name order.
    fn channels(&self) -> Vec<CompletionItem> {
        let mut out: Vec<CompletionItem> = self
            .visible_channels()
            .into_iter()
            .map(|(local, _, module)| {
                let mut i = item(&local, CompletionItemKind::INTERFACE, Some("channel"));
                i.documentation = None;
                i.detail = Some(format!("channel in {module}"));
                i
            })
            .collect();
        out.sort_by(|a, b| a.label.cmp(&b.label));
        out.dedup_by(|a, b| a.label == b.label);
        out
    }

    /// `(local name, file, module)` of every channel the file can name.
    fn visible_channels(&self) -> Vec<(String, FileId, String)> {
        let ix = self.ix();
        let Some(f) = ix.file(self.file) else {
            return Vec::new();
        };
        let mut found = Vec::new();
        for d in &f.ast.decls {
            if let DeclKind::Channel(c) = &d.kind {
                found.push((c.name.name.clone(), self.file, f.module.clone()));
            }
        }
        for u in &f.ast.uses {
            let pkg = u
                .package
                .as_ref()
                .map_or(f.package.as_str(), |p| p.name.as_str());
            let module = u.module_path();
            let Some(target) = ix.module_file(pkg, &module) else {
                continue;
            };
            let Some(tf) = ix.file(target) else { continue };
            for imp in &u.imports {
                let is_channel = tf.ast.decls.iter().any(
                    |d| matches!(&d.kind, DeclKind::Channel(c) if c.name.name == imp.name.name),
                );
                if is_channel {
                    let local = imp.alias.as_ref().unwrap_or(&imp.name);
                    found.push((local.name.clone(), target, module.clone()));
                }
            }
        }
        found
    }

    /// The declaration of the channel that this file calls `name`.
    fn channel_ast(&self, name: &str) -> Option<&ChannelDecl> {
        let (_, file, _) = self
            .visible_channels()
            .into_iter()
            .find(|(local, _, _)| local == name)?;
        let f = self.ix().file(file)?;
        let real = self.real_name(name);
        f.ast.decls.iter().find_map(|d| {
            let DeclKind::Channel(c) = &d.kind else {
                return None;
            };
            (c.name.name == real).then_some(c)
        })
    }

    /// The declared name behind a local name: an alias of `use` points at another name.
    fn real_name(&self, local: &str) -> String {
        let Some(f) = self.ix().file(self.file) else {
            return local.to_owned();
        };
        f.ast
            .uses
            .iter()
            .flat_map(|u| u.imports.iter())
            .find(|i| i.alias.as_ref().is_some_and(|a| a.name == local))
            .map_or_else(|| local.to_owned(), |i| i.name.name.clone())
    }
}

/// The names written after `carries` in a channel.
fn carried_names(c: &ChannelDecl) -> Vec<String> {
    c.carries
        .iter()
        .flat_map(|line| line.events.iter())
        .filter_map(|t| match &t.kind {
            TypeExprKind::Named { name, .. } => Some(name.name.clone()),
            TypeExprKind::List(_) => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_line_decides_what_is_wanted() {
        assert_eq!(wanted("  carries "), Some(Wanted::Events(Vec::new())));
        assert_eq!(
            wanted("  carries A, "),
            Some(Wanted::Events(vec!["A".into()]))
        );
        assert_eq!(wanted("  carries A "), None);
        assert_eq!(wanted("  publishes A, "), Some(Wanted::Channels));
        assert_eq!(wanted("  subscribes "), Some(Wanted::Channels));
        assert_eq!(wanted("  subscribes A "), None);
        assert_eq!(wanted("    on "), Some(Wanted::HandledEvents));
        assert_eq!(wanted("    on A -> "), None);
        assert_eq!(wanted("  given event "), Some(Wanted::Events(Vec::new())));
        assert_eq!(wanted("  given event A { } on "), Some(Wanted::Channels));
        assert_eq!(wanted("  given event A { } "), None);
        assert_eq!(wanted("  on: int"), None);
        assert_eq!(wanted("  key "), None);
    }
}

//! Printing of services, channels and event examples (`spec/events.md` sections 4, 5, 12, 15).

use veld_compiler::syntax::ast::{ChannelDecl, EventExample, ServiceDecl, Subscribes};

use super::super::layout;
use super::super::stmt;
use super::super::text::{quote, type_expr};
use super::{Printer, endpoint_lines};

impl Printer<'_> {
    /// `publishes` lines first, then the endpoints, then the `subscribes` blocks.
    pub(super) fn service(&mut self, key: u32, s: &ServiceDecl) {
        let head = match &s.prefix {
            Some(p) => format!("service {} at {}", s.name.name, quote(&p.value)),
            None => format!("service {}", s.name.name),
        };
        let rendered: Vec<Vec<String>> = s.endpoints.iter().map(|e| endpoint_lines(e, 2)).collect();
        let heavy: Vec<bool> = s
            .endpoints
            .iter()
            .zip(&rendered)
            .map(|(e, r)| {
                !e.docs.is_empty()
                    || e.block.is_some()
                    || !e.annotations.is_empty()
                    || r.len() > 1
                    || self
                        .att(layout::endpoint_key(e))
                        .is_some_and(|a| !a.leading.is_empty())
            })
            .collect();
        let empty = s.endpoints.is_empty() && s.publishes.is_empty() && s.subscribes.is_empty();
        self.block(key, &head, 0, empty, |p| {
            for line in &s.publishes {
                let names: Vec<String> = line.channels.iter().map(type_expr).collect();
                p.example_line(line.span.start, &format!("publishes {}", names.join(", ")));
            }
            if !s.publishes.is_empty() && heavy.first().copied().unwrap_or(false) {
                p.out.push('\n');
            }
            for (i, e) in s.endpoints.iter().enumerate() {
                if i > 0 && (heavy[i] || heavy[i - 1]) {
                    p.out.push('\n');
                }
                p.endpoint(e, &rendered[i], 2);
            }
            let before = !s.publishes.is_empty() || !s.endpoints.is_empty();
            for (i, sub) in s.subscribes.iter().enumerate() {
                if before || i > 0 {
                    p.out.push('\n');
                }
                p.subscribes(sub);
            }
        });
    }

    /// One block per subscription; the arrows of its `on` lines are aligned.
    fn subscribes(&mut self, s: &Subscribes) {
        let head = format!(
            "subscribes {} as {}",
            type_expr(&s.channel),
            quote(&s.group.value)
        );
        let events: Vec<String> = s.handlers.iter().map(|h| type_expr(&h.event)).collect();
        let width = events.iter().map(|e| e.chars().count()).max().unwrap_or(0);
        self.leading(s.span.start, 2);
        self.block(s.span.start, &head, 2, s.handlers.is_empty(), |p| {
            for (h, ev) in s.handlers.iter().zip(&events) {
                let key = layout::on_key(h);
                p.leading(key, 4);
                p.docs(&h.docs, 4);
                let text = format!("on {ev:<width$} -> {}", h.handler.name);
                p.line_no_newline(4, &text);
                p.end_line(key, 4);
            }
        });
    }

    /// `carries` lines, then `key` lines.
    pub(super) fn channel(&mut self, key: u32, c: &ChannelDecl) {
        let head = format!("channel {}", c.name.name);
        self.block(
            key,
            &head,
            0,
            c.carries.is_empty() && c.keys.is_empty(),
            |p| {
                for x in &c.carries {
                    let names: Vec<String> = x.events.iter().map(type_expr).collect();
                    p.example_line(x.span.start, &format!("carries {}", names.join(", ")));
                }
                for k in &c.keys {
                    p.example_line(k.span.start, &format!("key {}", k.field.name));
                }
            },
        );
    }

    /// The `given event` line, then one `expect` line each.
    pub(super) fn event_example(&mut self, e: &EventExample) {
        self.example_line(e.span.start, &stmt::given_event_line(e));
        for x in &e.expects {
            self.example_line(x.span.start, &stmt::expect_line(x));
        }
    }
}

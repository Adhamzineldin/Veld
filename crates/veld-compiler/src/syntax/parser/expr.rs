//! Rule declarations and the expression grammar (`spec/rules.md` sections 3.1 and 4).
//!
//! Precedence, loosest first: `implies` (right), `||`, `&&`, one comparison (not chainable),
//! `+ -`, `* / %`, unary `! not -`, postfix `.name`. Every level of the tree counts against
//! [`MAX_EXPR_NESTING`](super::MAX_EXPR_NESTING), also the links of a left-associative chain, so later
//! passes can recurse over any tree the parser returns.
//!
//! The recursion is kept shallow on purpose (about five frames per nested level): a legal
//! expression at the limit must parse on a small stack, also in a debug build. `self.depth`
//! is restored by [`Parser::parse_expr`] after a failure, so the helpers below only restore it
//! on success.

use super::{Parser, Rule};
use crate::diagnostics::Diagnostic;
use crate::syntax::ast::{Annotation, BinOp, CallArg, Doc, Expr, ExprKind, Ident, UnOp};
use crate::syntax::lexer::Tok;

const P_IMPLIES: u8 = 1;
const P_OR: u8 = 2;
const P_AND: u8 = 3;
const P_CMP: u8 = 4;
const P_ADD: u8 = 5;
const P_MUL: u8 = 6;

fn binary(op: BinOp, l: Expr, r: Expr) -> Expr {
    Expr {
        span: l.span.to(r.span),
        kind: ExprKind::Binary {
            op,
            l: Box::new(l),
            r: Box::new(r),
        },
    }
}

impl Parser<'_> {
    /// `rule` IDENT `:` at token `n`: the start of a rule (`rule: T` stays a field).
    pub(super) fn is_rule_start_at(&self, n: usize) -> bool {
        matches!(self.tok_at(n), Tok::Ident(w) if w == "rule")
            && matches!(self.tok_at(n + 1), Tok::Ident(_))
            && matches!(self.tok_at(n + 2), Tok::Colon)
    }

    pub(super) fn parse_rule(
        &mut self,
        docs: Vec<Doc>,
        annotations: Vec<Annotation>,
    ) -> Option<Rule> {
        let kw = self.advance().span;
        let name = self.expect_ident("a rule name")?;
        self.expect(&Tok::Colon, "`:` after the rule name")?;
        let expr = self.parse_expr()?;
        let mut end = expr.span;
        let mut message = None;
        if self.at_word("message") && matches!(self.tok_at(1), Tok::Str(_)) {
            self.advance();
            let text = self.expect_str("a message string")?;
            end = text.span;
            message = Some(text);
        }
        Some(Rule {
            docs,
            annotations,
            name,
            expr,
            message,
            span: kw.to(end),
        })
    }

    /// Parses one expression. Entry point of every nested expression; restores the nesting
    /// depth whether or not the parse succeeds.
    pub(super) fn parse_expr(&mut self) -> Option<Expr> {
        let base = self.depth;
        if !self.enter_expr() {
            return None;
        }
        let parsed = self.parse_binary(P_IMPLIES);
        self.depth = base;
        parsed
    }

    fn word_operator(&self, word: &str) -> bool {
        self.at_word(word) && !matches!(self.tok_at(1), Tok::Colon | Tok::Question)
    }

    /// The binary operator at the cursor with its precedence (higher binds tighter).
    fn binary_op(&self) -> Option<(BinOp, u8)> {
        let op = match self.peek() {
            Tok::OrOr => BinOp::Or,
            Tok::AndAnd => BinOp::And,
            Tok::EqEq => BinOp::Eq,
            Tok::NotEq => BinOp::Ne,
            Tok::Lt => BinOp::Lt,
            Tok::LtEq => BinOp::Le,
            Tok::Gt => BinOp::Gt,
            Tok::GtEq => BinOp::Ge,
            Tok::Plus => BinOp::Add,
            Tok::Minus => BinOp::Sub,
            Tok::Star => BinOp::Mul,
            Tok::Slash => BinOp::Div,
            Tok::Percent => BinOp::Rem,
            _ if self.word_operator("in") => BinOp::In,
            _ if self.word_operator("implies") => BinOp::Implies,
            _ => return None,
        };
        let prec = match op {
            BinOp::Implies => P_IMPLIES,
            BinOp::Or => P_OR,
            BinOp::And => P_AND,
            BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge | BinOp::In => {
                P_CMP
            }
            BinOp::Add | BinOp::Sub => P_ADD,
            BinOp::Mul | BinOp::Div | BinOp::Rem => P_MUL,
        };
        Some((op, prec))
    }

    /// Precedence climbing over every binary level. Each operator deepens the tree by one
    /// level, also in a left-associative chain; the levels are released when the call ends.
    fn parse_binary(&mut self, min: u8) -> Option<Expr> {
        let base = self.depth;
        let mut l = self.parse_unary()?;
        let mut after_comparison = false;
        while let Some((op, prec)) = self.binary_op() {
            if prec < min {
                break;
            }
            if prec == P_CMP && after_comparison {
                self.chained_comparison();
            }
            after_comparison = prec == P_CMP;
            self.advance();
            if !self.enter_expr() {
                return None;
            }
            // `implies` leans to the right; every other level to the left.
            let next_min = if op == BinOp::Implies { prec } else { prec + 1 };
            let r = self.parse_binary(next_min)?;
            l = binary(op, l, r);
        }
        self.depth = base;
        Some(l)
    }

    /// `E0902` for `a < b < c`. The chain is still parsed, so one mistake gives one error.
    fn chained_comparison(&mut self) {
        let span = self.span();
        let already = self
            .diags
            .iter()
            .any(|d| d.code == "E0902" && d.primary.start == span.start);
        if !already {
            self.diags.push(
                Diagnostic::error("E0902", "comparisons cannot be chained", span)
                    .with_help("write `a < b && b < c`; a comparison is not associative"),
            );
        }
    }

    /// True when the token at `n` can begin an operand.
    fn starts_operand_at(&self, n: usize) -> bool {
        matches!(
            self.tok_at(n),
            Tok::Ident(_)
                | Tok::Int(_)
                | Tok::Float(_)
                | Tok::Str(_)
                | Tok::LParen
                | Tok::LBracket
                | Tok::Dot
                | Tok::Bang
                | Tok::Minus
        )
    }

    /// `not` is an operator only when an operand follows; otherwise it is a field called `not`.
    /// `not in x` and `not implies y` read as the field `not` followed by a binary operator.
    fn not_is_operator(&self) -> bool {
        let operator_next = matches!(self.tok_at(1), Tok::Ident(w) if w == "in" || w == "implies")
            && self.starts_operand_at(2);
        self.starts_operand_at(1) && !operator_next
    }

    fn unary_op(&self) -> Option<UnOp> {
        match self.peek() {
            Tok::Bang => Some(UnOp::Not { word: false }),
            Tok::Minus => Some(UnOp::Neg),
            Tok::Ident(w) if w == "not" && self.not_is_operator() => Some(UnOp::Not { word: true }),
            _ => None,
        }
    }

    /// A `-` directly followed by a numeric literal that has no sign of its own
    /// (`spec/rules.md` section 2.1): the two tokens are one negative literal. The lexer reads a
    /// `-` after a word such as `implies` as an operator, so the parser joins them here.
    pub(super) fn parse_negative_literal(&mut self) -> Option<Expr> {
        if !matches!(self.peek(), Tok::Minus) {
            return None;
        }
        let tok = self.tok_at(1).clone();
        let lit_span = self.toks.get(self.pos + 1)?.span;
        let text = self
            .text
            .get(lit_span.start as usize..lit_span.end as usize)
            .unwrap_or_default()
            .to_string();
        if text.starts_with('-') {
            return None;
        }
        let kind = match tok {
            Tok::Int(i64::MIN) => {
                // `9223372036854775808` overflowed in the lexer, which reported it; with the
                // minus sign it is the smallest `long`.
                self.diags
                    .retain(|d| !(d.code == "E0002" && d.primary == lit_span));
                ExprKind::Int(i64::MIN)
            }
            Tok::Int(n) => ExprKind::Int(-n),
            Tok::Float(value) => ExprKind::Float {
                value: -value,
                text: format!("-{text}"),
            },
            _ => return None,
        };
        let start = self.advance().span;
        self.advance();
        Some(Expr {
            kind,
            span: start.to(lit_span),
        })
    }

    /// Prefix operators, a primary, then `.name` steps.
    fn parse_unary(&mut self) -> Option<Expr> {
        if let Some(lit) = self.parse_negative_literal() {
            return Some(lit);
        }
        if let Some(op) = self.unary_op() {
            let start = self.advance().span;
            if !self.enter_expr() {
                return None;
            }
            let operand = self.parse_unary()?;
            self.depth -= 1;
            return Some(Expr {
                span: start.to(operand.span),
                kind: ExprKind::Unary {
                    op,
                    operand: Box::new(operand),
                },
            });
        }
        let base = self.depth;
        let mut e = self.parse_primary()?;
        while matches!(self.peek(), Tok::Dot) && matches!(self.tok_at(1), Tok::Ident(_)) {
            self.advance();
            if !self.enter_expr() {
                return None;
            }
            let name = self.expect_ident("a field name")?;
            e = Expr {
                span: e.span.to(name.span),
                kind: ExprKind::Field {
                    base: Box::new(e),
                    name,
                },
            };
        }
        self.depth = base;
        Some(e)
    }

    fn parse_primary(&mut self) -> Option<Expr> {
        let span = self.span();
        match self.peek() {
            Tok::Ident(_) => self.parse_word(),
            Tok::LParen => self.parse_group(),
            Tok::LBracket => self.parse_list(),
            Tok::Dot => {
                self.advance();
                let name = self.expect_ident("an enum variant name")?;
                Some(Expr {
                    span: span.to(name.span),
                    kind: ExprKind::EnumLit(name),
                })
            }
            Tok::Int(_) | Tok::Float(_) | Tok::Str(_) => self.parse_literal_expr(),
            _ => {
                self.unexpected("an expression");
                None
            }
        }
    }

    fn parse_literal_expr(&mut self) -> Option<Expr> {
        let span = self.span();
        let kind = match self.advance().tok {
            Tok::Int(n) => ExprKind::Int(n),
            Tok::Float(value) => {
                let text = self
                    .text
                    .get(span.start as usize..span.end as usize)
                    .unwrap_or_default()
                    .to_string();
                ExprKind::Float { value, text }
            }
            Tok::Str(s) => ExprKind::Str(s),
            _ => return None,
        };
        Some(Expr { kind, span })
    }

    /// A name, `true`, `false` or a call.
    fn parse_word(&mut self) -> Option<Expr> {
        let span = self.span();
        // The next member of the body, not an operand: an expression cut short by a typo
        // must not swallow it.
        if self.is_rule_start_at(0) || self.is_field_start_at(0) {
            self.unexpected("an expression");
            return None;
        }
        if self.at_word("true") || self.at_word("false") {
            let value = self.at_word("true");
            self.advance();
            return Some(Expr {
                kind: ExprKind::Bool(value),
                span,
            });
        }
        if self.at_word("null") {
            self.advance();
            self.diags.push(
                Diagnostic::error("E0003", "expected an expression, found `null`", span)
                    .with_help("rules have no `null`; ask `has(field)` instead"),
            );
            return None;
        }
        let name = self.expect_ident("a name")?;
        if !matches!(self.peek(), Tok::LParen) {
            return Some(Expr {
                span: name.span,
                kind: ExprKind::Name(name),
            });
        }
        self.advance();
        self.parse_call(name)
    }

    fn parse_call(&mut self, name: Ident) -> Option<Expr> {
        let base = self.depth;
        if !self.enter_expr() {
            return None;
        }
        let mut args = Vec::new();
        while !matches!(self.peek(), Tok::RParen | Tok::Eof) {
            args.push(self.parse_call_arg()?);
            if !self.eat(&Tok::Comma) {
                break;
            }
        }
        self.depth = base;
        let end = self.expect(&Tok::RParen, "`)` to close the call")?;
        Some(Expr {
            span: name.span.to(end),
            kind: ExprKind::Call { name, args },
        })
    }

    fn parse_call_arg(&mut self) -> Option<CallArg> {
        if matches!(self.peek(), Tok::Ident(_)) && matches!(self.tok_at(1), Tok::FatArrow) {
            let var = self.expect_ident("a lambda variable")?;
            self.advance();
            let body = self.parse_expr()?;
            let span = var.span.to(body.span);
            return Some(CallArg::Lambda { var, body, span });
        }
        self.parse_expr().map(CallArg::Expr)
    }

    /// `( expr )`: leaves no node; the span of the inner expression grows to include the
    /// parentheses.
    fn parse_group(&mut self) -> Option<Expr> {
        let open = self.advance().span;
        let mut inner = self.parse_expr()?;
        let close = self.expect(&Tok::RParen, "`)` to close the parenthesis")?;
        inner.span = open.to(close);
        Some(inner)
    }

    fn parse_list(&mut self) -> Option<Expr> {
        let base = self.depth;
        let open = self.advance().span;
        if !self.enter_expr() {
            return None;
        }
        let mut items = Vec::new();
        while !matches!(self.peek(), Tok::RBracket | Tok::Eof) {
            items.push(self.parse_expr()?);
            if !self.eat(&Tok::Comma) {
                break;
            }
        }
        self.depth = base;
        let close = self.expect(&Tok::RBracket, "`]` to close the list")?;
        Some(Expr {
            span: open.to(close),
            kind: ExprKind::List(items),
        })
    }
}

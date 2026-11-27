//! `@timeout` and `@slo` (`spec/platform.md` sections 12 and 13): their arguments, and the
//! `E0602` of a wrong one.

use super::duration::const_duration_arg;
use crate::diagnostics::Span;
use crate::syntax::ast::{Arg, ArgValue, LitKind};

/// The longest deadline: one hour.
pub const MAX_TIMEOUT_MS: i64 = 3_600_000;

/// A checked `@timeout`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timeout {
    pub ms: i64,
    pub span: Span,
}

/// A checked `@slo`: the milliseconds of each stated latency and the text of `errors`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SloArgs {
    pub p50: Option<i64>,
    pub p90: Option<i64>,
    pub p95: Option<i64>,
    pub p99: Option<i64>,
    /// The percent, as written, in the shortest form (`0.1`, `1`).
    pub errors: Option<String>,
    pub span: Option<Span>,
}

/// `@timeout(d)`: one positional constant duration, positive, of at most one hour.
pub fn timeout_args(args: &[Arg], span: Span) -> Result<Timeout, String> {
    let [one] = args else {
        return Err("expected one duration, such as `seconds(2)`".into());
    };
    if one.name.is_some() {
        return Err("named arguments are not accepted here".into());
    }
    let ms = const_duration_arg(one)?;
    if ms <= 0 {
        return Err("the deadline must be greater than zero".into());
    }
    if ms > MAX_TIMEOUT_MS {
        return Err("the deadline is at most one hour: `hours(1)`".into());
    }
    Ok(Timeout { ms, span })
}

/// `@slo(p99: ms(300), errors: 0.1)`: named objectives, at least one, each once.
pub fn slo_args(args: &[Arg], span: Span) -> Result<SloArgs, String> {
    if args.is_empty() {
        return Err("state at least one objective: `p50`, `p90`, `p95`, `p99` or `errors`".into());
    }
    let mut out = SloArgs {
        span: Some(span),
        ..SloArgs::default()
    };
    for a in args {
        let Some(name) = &a.name else {
            return Err("every argument is named: `p99: ms(300)`".into());
        };
        let slot = match name.name.as_str() {
            "p50" => &mut out.p50,
            "p90" => &mut out.p90,
            "p95" => &mut out.p95,
            "p99" => &mut out.p99,
            "errors" => {
                if out.errors.is_some() {
                    return Err("`errors` is given twice".into());
                }
                out.errors = Some(errors_text(a)?);
                continue;
            }
            other => {
                return Err(format!(
                    "`{other}` is not an objective: use p50, p90, p95, p99 or errors"
                ));
            }
        };
        if slot.is_some() {
            return Err(format!("`{}` is given twice", name.name));
        }
        let ms = const_duration_arg(a)?;
        if ms <= 0 {
            return Err(format!("`{}` must be a positive duration", name.name));
        }
        *slot = Some(ms);
    }
    check_order(&out)?;
    Ok(out)
}

/// A faster share cannot take longer: `p99` below `p95` contradicts itself.
fn check_order(s: &SloArgs) -> Result<(), String> {
    let stated: Vec<(&str, i64)> = [
        ("p50", s.p50),
        ("p90", s.p90),
        ("p95", s.p95),
        ("p99", s.p99),
    ]
    .into_iter()
    .filter_map(|(n, v)| v.map(|v| (n, v)))
    .collect();
    for pair in stated.windows(2) {
        if pair[0].1 > pair[1].1 {
            return Err(format!(
                "`{}` ({} ms) is slower than `{}` ({} ms): a share that is further out cannot be faster",
                pair[0].0, pair[0].1, pair[1].0, pair[1].1
            ));
        }
    }
    Ok(())
}

/// `errors`: a decimal above 0 and below 100.
fn errors_text(a: &Arg) -> Result<String, String> {
    let ArgValue::Literal(l) = &a.value else {
        return Err("`errors` is a number of percent, such as `0.1`".into());
    };
    let value = if let LitKind::Int(n) = &l.kind {
        #[allow(clippy::cast_precision_loss)]
        let f = *n as f64;
        f
    } else if let LitKind::Float(f) = &l.kind {
        *f
    } else {
        return Err("`errors` is a number of percent, such as `0.1`".into());
    };
    if !(value > 0.0 && value < 100.0) {
        return Err("`errors` is above 0 and below 100 (percent)".into());
    }
    Ok(format!("{value}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::Span;
    use crate::syntax::ast::{Ident, Literal};

    fn ident(name: &str) -> Ident {
        Ident {
            name: name.into(),
            span: Span::none(),
        }
    }

    fn call(unit: &str, n: i64) -> ArgValue {
        ArgValue::Call {
            name: ident(unit),
            args: vec![Arg {
                name: None,
                value: ArgValue::Literal(Literal {
                    kind: LitKind::Int(n),
                    span: Span::none(),
                }),
                span: Span::none(),
            }],
        }
    }

    fn arg(name: Option<&str>, value: ArgValue) -> Arg {
        Arg {
            name: name.map(ident),
            value,
            span: Span::none(),
        }
    }

    fn float(f: f64) -> ArgValue {
        ArgValue::Literal(Literal {
            kind: LitKind::Float(f),
            span: Span::none(),
        })
    }

    #[test]
    fn a_timeout_is_one_positive_duration_of_at_most_an_hour() {
        let s = Span::none();
        assert_eq!(
            timeout_args(&[arg(None, call("seconds", 2))], s).map(|t| t.ms),
            Ok(2000)
        );
        assert_eq!(
            timeout_args(&[arg(None, call("hours", 1))], s).map(|t| t.ms),
            Ok(3_600_000)
        );
        assert!(timeout_args(&[arg(None, call("hours", 2))], s).is_err());
        assert!(timeout_args(&[arg(None, call("ms", 0))], s).is_err());
        assert!(timeout_args(&[arg(None, call("ms", -5))], s).is_err());
        assert!(timeout_args(&[], s).is_err());
        assert!(timeout_args(&[arg(Some("d"), call("ms", 5))], s).is_err());
        assert!(timeout_args(&[arg(None, call("furlongs", 5))], s).is_err());
    }

    #[test]
    fn an_slo_takes_named_objectives_once_in_any_order() {
        let s = Span::none();
        let ok = slo_args(
            &[
                arg(Some("errors"), float(0.1)),
                arg(Some("p99"), call("ms", 300)),
                arg(Some("p50"), call("ms", 100)),
            ],
            s,
        )
        .unwrap();
        assert_eq!((ok.p50, ok.p99, ok.p90), (Some(100), Some(300), None));
        assert_eq!(ok.errors.as_deref(), Some("0.1"));
    }

    #[test]
    fn a_wrong_slo_is_an_error_with_a_reason() {
        let s = Span::none();
        let bad: [Vec<Arg>; 8] = [
            vec![],
            vec![arg(None, call("ms", 1))],
            vec![arg(Some("p98"), call("ms", 1))],
            vec![arg(Some("p99"), call("ms", 0))],
            vec![
                arg(Some("p99"), call("ms", 1)),
                arg(Some("p99"), call("ms", 2)),
            ],
            vec![arg(Some("errors"), float(0.0))],
            vec![arg(Some("errors"), float(100.0))],
            vec![
                arg(Some("p95"), call("ms", 500)),
                arg(Some("p99"), call("ms", 300)),
            ],
        ];
        for args in bad {
            assert!(slo_args(&args, s).is_err(), "{args:?}");
        }
    }

    #[test]
    fn errors_is_written_in_the_shortest_form() {
        let s = Span::none();
        let one = |v| slo_args(&[arg(Some("errors"), v)], s).unwrap().errors;
        assert_eq!(one(float(0.1)).as_deref(), Some("0.1"));
        assert_eq!(one(float(99.99)).as_deref(), Some("99.99"));
        assert_eq!(
            one(ArgValue::Literal(Literal {
                kind: LitKind::Int(5),
                span: Span::none()
            }))
            .as_deref(),
            Some("5")
        );
    }
}

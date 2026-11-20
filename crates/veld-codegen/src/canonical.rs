//! RFC 8785 (JSON Canonicalization Scheme) serializer for `serde_json::Value`.
//!
//! Numbers are formatted like ECMAScript's `Number::toString` applied to the
//! nearest `f64`, as the RFC requires. Object keys are sorted by UTF-16 code
//! units.

use serde_json::Value;
use std::fmt::Write;

/// Canonical JSON text of `value`.
#[must_use]
pub fn canonical(value: &Value) -> String {
    let mut out = String::new();
    write_value(value, &mut out);
    out
}

/// Canonical JSON bytes (UTF-8) of `value`.
#[must_use]
pub fn canonical_bytes(value: &Value) -> Vec<u8> {
    canonical(value).into_bytes()
}

fn write_value(value: &Value, out: &mut String) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(true) => out.push_str("true"),
        Value::Bool(false) => out.push_str("false"),
        Value::Number(n) => out.push_str(&es_number(n.as_f64().unwrap_or(0.0))),
        Value::String(s) => write_string(s, out),
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_value(item, out);
            }
            out.push(']');
        }
        Value::Object(map) => {
            let mut entries: Vec<(&String, &Value)> = map.iter().collect();
            entries.sort_by(|a, b| a.0.encode_utf16().cmp(b.0.encode_utf16()));
            out.push('{');
            for (i, (k, v)) in entries.into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_string(k, out);
                out.push(':');
                write_value(v, out);
            }
            out.push('}');
        }
    }
}

fn write_string(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

/// ECMAScript `Number::toString(10)` for a finite `f64` (`-0` prints as `0`).
#[must_use]
pub fn es_number(v: f64) -> String {
    if v == 0.0 || !v.is_finite() {
        return "0".to_string();
    }
    let sign = if v < 0.0 { "-" } else { "" };
    // `{:e}` yields the shortest digits that round-trip, e.g. `1.2345e-7`.
    let sci = format!("{:e}", v.abs());
    let (mantissa, exp) = sci.split_once('e').unwrap_or((&sci, "0"));
    let exp: i32 = exp.parse().unwrap_or(0);
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    let k = digits.len() as i32;
    let n = exp + 1; // position of the decimal point relative to the digits
    let body = if k <= n && n <= 21 {
        format!("{digits}{}", "0".repeat((n - k) as usize))
    } else if 0 < n && n <= 21 {
        format!("{}.{}", &digits[..n as usize], &digits[n as usize..])
    } else if -6 < n && n <= 0 {
        format!("0.{}{digits}", "0".repeat((-n) as usize))
    } else {
        let e = n - 1;
        let esign = if e < 0 { '-' } else { '+' };
        if k == 1 {
            format!("{digits}e{esign}{}", e.abs())
        } else {
            format!("{}.{}e{esign}{}", &digits[..1], &digits[1..], e.abs())
        }
    };
    format!("{sign}{body}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn numbers_follow_ecmascript() {
        let cases: &[(f64, &str)] = &[
            (0.0, "0"),
            (-0.0, "0"),
            (1.0, "1"),
            (-1.0, "-1"),
            (0.1, "0.1"),
            (1e21, "1e+21"),
            (1e20, "100000000000000000000"),
            (1.5e300, "1.5e+300"),
            (123456789.125, "123456789.125"),
            (0.000001, "0.000001"),
            (0.0000001, "1e-7"),
            (1.2345e-7, "1.2345e-7"),
            (4.5, "4.5"),
            (100.0, "100"),
            (9007199254740992.0, "9007199254740992"),
            (f64::MAX, "1.7976931348623157e+308"),
            (5e-324, "5e-324"),
        ];
        for (v, want) in cases {
            assert_eq!(es_number(*v), *want, "{v:?}");
        }
    }

    #[test]
    fn integers_and_big_numbers_through_json() {
        let v: Value = serde_json::from_str("[1, -0, 1e21, 0.1, 12345678901234567890, 1.0]").unwrap();
        assert_eq!(canonical(&v), "[1,0,1e+21,0.1,12345678901234567000,1]");
    }

    #[test]
    fn keys_sort_by_utf16_units() {
        // U+1F600 is the surrogate pair D83D DE00, so it sorts before U+FF5E.
        let v = json!({ "\u{ff5e}": 1, "\u{1f600}": 2, "b": 3, "a": 4, "": 5 });
        assert_eq!(canonical(&v), "{\"\":5,\"a\":4,\"b\":3,\"\u{1f600}\":2,\"\u{ff5e}\":1}");
    }

    #[test]
    fn strings_escape_like_json_stringify() {
        let v = json!("a\"b\\c\n\t\u{1}\u{7f}é");
        assert_eq!(canonical(&v), "\"a\\\"b\\\\c\\n\\t\\u0001\u{7f}é\"");
    }

    #[test]
    fn nested_structures() {
        let v = json!({ "z": [true, null, { "b": 1, "a": [] }], "a": {} });
        assert_eq!(canonical(&v), "{\"a\":{},\"z\":[true,null,{\"a\":[],\"b\":1}]}");
    }
}

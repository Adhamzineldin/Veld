//! Fixing the variables of a question to the parts of a value, so that the encoding can be
//! compared with the reference evaluator on that value.

use serde_json::Value as Json;
use veld_ir::{Scalar, Type};
use veld_rules::helpers::instant::{parse_date, parse_datetime, parse_time};
use veld_rules::helpers::text::{base64_byte_len, normalize_uuid};

use crate::problem::Problem;
use crate::smt;
use crate::types::{RootKey, Sort, VarInfo, VarKind};

/// The exact value of a double as an SMT real, when it is small enough to write down.
pub fn exact_real(x: f64) -> Option<String> {
    if !x.is_finite() {
        return None;
    }
    if x == 0.0 {
        return Some("0.0".to_owned());
    }
    let bits = x.to_bits();
    let neg = bits >> 63 == 1;
    let exp = i32::try_from((bits >> 52) & 0x7ff).ok()?;
    let frac = i128::from(bits & ((1u64 << 52) - 1));
    let (mant, e) = if exp == 0 {
        (frac, -1074)
    } else {
        (frac | (1i128 << 52), exp - 1075)
    };
    let body = if e >= 0 {
        if e > 60 {
            return None;
        }
        smt::real_int(mant << e)
    } else {
        if -e > 120 {
            return None;
        }
        format!("(/ {} {})", smt::real_int(mant), smt::real_int(1i128 << -e))
    };
    Some(if neg { format!("(- {body})") } else { body })
}

fn dig<'j>(json: &'j Json, path: &[String]) -> Option<&'j Json> {
    path.iter().try_fold(json, |cur, w| cur.get(w))
}

fn int_of(v: &VarInfo, leaf: &Json) -> Option<i128> {
    if !v.variants.is_empty() {
        let wire = leaf.as_str()?;
        return v.variants.iter().position(|w| w == wire).map(|i| i as i128);
    }
    match &v.ty {
        Some(Type::Scalar(Scalar::Int)) => Some(i128::from(leaf.as_i64()?)),
        Some(Type::Scalar(Scalar::Long)) => leaf.as_str()?.parse().ok(),
        Some(Type::Scalar(Scalar::Date)) => parse_date(leaf.as_str()?).map(i128::from),
        Some(Type::Scalar(Scalar::Datetime)) => parse_datetime(leaf.as_str()?).map(i128::from),
        Some(Type::Scalar(Scalar::Time)) => parse_time(leaf.as_str()?).map(i128::from),
        _ => None,
    }
}

fn term_of(p: &mut Problem<'_>, v: &VarInfo, leaf: &Json) -> Option<String> {
    match v.sort {
        Sort::Bool => Some(leaf.as_bool()?.to_string()),
        Sort::Int => Some(smt::int(int_of(v, leaf)?)),
        Sort::Real => match &v.ty {
            Some(Type::Scalar(Scalar::Decimal)) => Some(smt::real_text(leaf.as_str()?)),
            Some(Type::Scalar(Scalar::Float)) => exact_real(leaf.as_f64()?),
            _ => None,
        },
        Sort::Str => Some(p.literal(Sort::Str, leaf.as_str()?)),
        Sort::Uuid => Some(p.literal(Sort::Uuid, &normalize_uuid(leaf.as_str()?)?)),
    }
}

fn length_of(leaf: &Json) -> Option<usize> {
    match leaf {
        Json::Array(a) => Some(a.len()),
        Json::Object(o) => Some(o.len()),
        Json::String(s) => base64_byte_len(s),
        Json::Null | Json::Bool(_) | Json::Number(_) => None,
    }
}

/// Fixes every variable under `key` to the part of `json` it stands for. Returns false when a
/// part could not be fixed, so that the comparison is then one-sided.
pub fn pin_root(p: &mut Problem<'_>, key: &RootKey, json: &Json) -> bool {
    let vars: Vec<VarInfo> = p.vars().filter(|v| &v.root == key).cloned().collect();
    let mut complete = true;
    for v in vars {
        let name = smt::sym(&v.name);
        let leaf = dig(json, &v.path);
        match v.kind {
            VarKind::Has => p.assert_plain(&format!("(= {name} {})", leaf.is_some())),
            VarKind::Value => {
                if let Some(l) = leaf {
                    match term_of(p, &v, l) {
                        Some(t) => p.assert_plain(&format!("(= {name} {t})")),
                        None => complete = false,
                    }
                }
            }
            VarKind::Len => match leaf.and_then(length_of) {
                Some(n) => p.assert_plain(&format!("(= {name} {n})")),
                None => complete = false,
            },
        }
    }
    complete
}

#[test]
fn doubles_become_exact_reals() {
    assert_eq!(
        exact_real(0.5).as_deref(),
        Some("(/ 4503599627370496.0 9007199254740992.0)")
    );
    assert_eq!(
        exact_real(-2.0).as_deref(),
        Some("(- (/ 4503599627370496.0 2251799813685248.0))")
    );
    assert_eq!(exact_real(0.0).as_deref(), Some("0.0"));
    assert_eq!(exact_real(f64::NAN), None);
    let tenth = exact_real(0.1).unwrap();
    assert!(tenth.starts_with("(/ 7205759403792794.0"), "{tenth}");
}

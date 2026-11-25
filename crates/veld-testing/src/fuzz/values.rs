//! Values of the types of a contract, drawn from a seeded generator.
//!
//! Every value is well-formed for its type and its constraints: the fuzzer tests rules, not
//! shape validation. Strings are plain lower-case letters and digits, so a value can travel in a
//! path, a query string or a header, and be written in an `example` block, unchanged.

use serde_json::{Map, Number, Value as Json, json};
use veld_ir::{Constraints, Decl, DeclId, DeclKind, Field, Ir, Literal, Scalar, Type};
use veld_rules::helpers::instant::{MS_PER_DAY, parse_datetime};
use veld_rules::{DEFAULT_CLOCK, format_date, format_datetime, format_time};

use super::rng::Rng;

/// Why a value cannot be generated: the endpoint is skipped, with this reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Skip(pub String);

const MAX_DEPTH: usize = 6;
/// The longest string, list, map or `bytes` value the generator makes for an `@length(min)`.
const MAX_LEN: u64 = 4096;
const ALPHABET: &[u8; 36] = b"abcdefghijklmnopqrstuvwxyz0123456789";

pub(crate) struct Gen<'a> {
    pub ir: &'a Ir,
    pub rng: Rng,
    /// The default clock, in milliseconds: datetimes are drawn around it.
    pub clock: i64,
    /// True while a value is made for the path, the query string or a header: no string, list or
    /// map is empty there, because an empty value cannot be told from a missing one.
    pub flat: bool,
    /// The model of the principal of the service being fuzzed, if it has one.
    pub principal: Option<DeclId>,
    depth: usize,
}

impl<'a> Gen<'a> {
    pub fn new(ir: &'a Ir, seed: u64) -> Self {
        Gen {
            ir,
            rng: Rng::new(seed),
            clock: parse_datetime(DEFAULT_CLOCK).unwrap_or(0),
            flat: false,
            principal: None,
            depth: 0,
        }
    }

    pub fn decl(&self, id: DeclId) -> Option<&'a Decl> {
        self.ir.decls.get(id.index())
    }

    /// A value of type `ty` under the constraints `c`. A `@readonly` field is generated too: the
    /// filler of an `example` adds a required one, and the live examples send it.
    pub fn value(&mut self, ty: &Type, c: &Constraints) -> Result<Json, Skip> {
        if self.depth >= MAX_DEPTH {
            return Err(Skip("the type nests too deeply to generate".to_owned()));
        }
        self.depth += 1;
        let out = match ty {
            Type::Scalar(s) => self.scalar(*s, c),
            Type::List(t) => self.list(t, c),
            Type::Map(t) => self.map(t, c),
            Type::Page(t) => self.page(t),
            Type::Decl(id) => self.of_decl(*id),
        };
        self.depth -= 1;
        out
    }

    /// A field's value when the field is present: its `@example` or default (sometimes), or a
    /// random value.
    pub fn field_present(&mut self, f: &Field) -> Result<Json, Skip> {
        let preset = preset(f);
        let random = self.value(&f.ty, &f.constraints);
        match (preset, random) {
            (Some(p), Ok(v)) => Ok(if self.rng.chance(60) { p } else { v }),
            (Some(p), Err(_)) => Ok(p),
            (None, Ok(v)) => Ok(v),
            (None, Err(Skip(why))) => Err(Skip(format!("`{}`: {why}", f.name))),
        }
    }

    /// `None` when the field is left out of the value.
    fn field(&mut self, f: &Field) -> Result<Option<Json>, Skip> {
        // A field with `@example` or a default is never left out: an `example` block fills it in
        // again, so a request without it could not be written as one.
        if preset(f).is_some() {
            return self.field_present(f).map(Some);
        }
        if f.optional && (needs_example(&f.constraints) || !self.rng.chance(50)) {
            return Ok(None);
        }
        self.field_present(f).map(Some)
    }

    fn of_decl(&mut self, id: DeclId) -> Result<Json, Skip> {
        let decl = self
            .decl(id)
            .ok_or_else(|| Skip("a type refers to a missing declaration".to_owned()))?;
        match &decl.kind {
            DeclKind::Enum(e) => {
                let v = e.variants.get(self.rng.index(e.variants.len()));
                v.map(|v| Json::String(v.wire.clone()))
                    .ok_or_else(|| Skip(format!("enum `{}` has no variant", decl.name)))
            }
            DeclKind::Model(_) | DeclKind::Error(_) => self.model(decl),
            DeclKind::Union(u) => {
                let arm = u.arms.get(self.rng.index(u.arms.len()));
                let arm = arm.ok_or_else(|| Skip(format!("union `{}` has no arm", decl.name)))?;
                let arm_decl = self
                    .decl(arm.decl)
                    .ok_or_else(|| Skip("a union arm is missing".to_owned()))?;
                let mut obj = self.model(arm_decl)?;
                if let Json::Object(map) = &mut obj {
                    map.insert(u.discriminator.clone(), Json::String(arm.label.clone()));
                }
                Ok(obj)
            }
            DeclKind::Const(_) => Err(Skip(format!("`{}` is a constant", decl.name))),
        }
    }

    fn model(&mut self, decl: &Decl) -> Result<Json, Skip> {
        let mut out = Map::new();
        for f in decl.fields() {
            if let Some(v) = self.field(f)? {
                out.insert(f.wire.clone(), v);
            }
        }
        Ok(Json::Object(out))
    }

    /// A length in the bounds of `@length`, small unless the bounds ask for more. The minimum is
    /// never cut short: a value shorter than `@length(min)` would be rejected by a correct server
    /// and passed by the reference pipeline, which does not validate shape.
    pub fn length(&mut self, c: &Constraints, spread: u64) -> Result<usize, Skip> {
        let floor = u64::from(self.flat);
        let lo = c.min_len.unwrap_or(0).max(floor);
        if lo > MAX_LEN {
            return Err(Skip(format!(
                "a value must be at least {lo} long, more than the generator makes ({MAX_LEN})"
            )));
        }
        let hi = c.max_len.unwrap_or(lo + spread).min(lo + spread).max(lo);
        usize::try_from(lo + self.rng.below(hi - lo + 1))
            .map_err(|_| Skip("a length does not fit this platform".to_owned()))
    }

    fn list(&mut self, t: &Type, c: &Constraints) -> Result<Json, Skip> {
        let n = self.length(c, 3)?;
        let items: Result<Vec<Json>, Skip> = (0..n)
            .map(|_| self.value(t, &Constraints::default()))
            .collect();
        items.map(Json::Array)
    }

    fn map(&mut self, t: &Type, c: &Constraints) -> Result<Json, Skip> {
        let n = self.length(c, 3)?;
        let mut out = Map::new();
        for i in 0..n {
            out.insert(
                format!("k{}", i + 1),
                self.value(t, &Constraints::default())?,
            );
        }
        Ok(Json::Object(out))
    }

    fn page(&mut self, t: &Type) -> Result<Json, Skip> {
        let n = self.length(&Constraints::default(), 2)?;
        let items: Result<Vec<Json>, Skip> = (0..n)
            .map(|_| self.value(t, &Constraints::default()))
            .collect();
        Ok(json!({ "items": items?, "total": n.to_string(), "page": 1, "size": n }))
    }

    /// A string of `n` random lower-case letters and digits.
    pub fn text(&mut self, n: usize) -> String {
        (0..n)
            .map(|_| char::from(ALPHABET[self.rng.index(ALPHABET.len())]))
            .collect()
    }

    /// A string, list, map or `bytes` value of exactly `n` code points, items, entries or bytes.
    pub fn sized(&mut self, ty: &Type, n: usize) -> Result<Json, Skip> {
        let none = Constraints::default();
        match ty {
            Type::Scalar(Scalar::String) => Ok(Json::String(self.text(n))),
            Type::Scalar(Scalar::Bytes) => {
                let bytes: Vec<u8> = (0..n).map(|_| self.rng.below(256) as u8).collect();
                Ok(Json::String(base64(&bytes)))
            }
            Type::List(t) => (0..n)
                .map(|_| self.value(t, &none))
                .collect::<Result<Vec<Json>, Skip>>()
                .map(Json::Array),
            Type::Map(t) => {
                let mut out = Map::new();
                for i in 0..n {
                    out.insert(format!("k{}", i + 1), self.value(t, &none)?);
                }
                Ok(Json::Object(out))
            }
            Type::Scalar(_) | Type::Page(_) | Type::Decl(_) => {
                Err(Skip("this type has no length".to_owned()))
            }
        }
    }

    fn scalar(&mut self, s: Scalar, c: &Constraints) -> Result<Json, Skip> {
        Ok(match s {
            Scalar::String => {
                if needs_example(c) {
                    let why = "a string with @pattern or @format and no @example";
                    return Err(Skip(why.to_owned()));
                }
                let n = self.length(c, 6)?;
                Json::String(self.text(n))
            }
            Scalar::Int => {
                let (lo, hi) = self.bounds(c, i64::from(i32::MIN), i64::from(i32::MAX));
                Json::from(self.pick(lo, hi))
            }
            Scalar::Long => {
                let limit = 1 << 53;
                let (lo, hi) = self.bounds(c, -limit, limit);
                Json::String(self.pick(lo, hi).to_string())
            }
            Scalar::Float => {
                let n = Number::from_f64(self.fraction(c).0).map(Json::Number);
                n.ok_or_else(|| Skip("the bounds of a float are not finite".to_owned()))?
            }
            Scalar::Decimal => Json::String(self.decimal(c)),
            Scalar::Bool => Json::Bool(self.rng.chance(50)),
            Scalar::Uuid => Json::String(self.uuid()),
            Scalar::Date => {
                let day = self.clock.div_euclid(MS_PER_DAY) + self.rng.range(-60, 60);
                Json::String(format_date(day))
            }
            Scalar::Datetime => Json::String(format_datetime(self.datetime())),
            Scalar::Time => Json::String(format_time(self.rng.range(0, MS_PER_DAY - 1))),
            Scalar::Bytes => {
                let n = self.length(c, 6)?;
                self.sized(&Type::Scalar(Scalar::Bytes), n)?
            }
            Scalar::Json => json!({}),
        })
    }

    /// An integer range: `@min`..`@max`, or a small range around zero.
    fn bounds(&self, c: &Constraints, floor: i64, ceil: i64) -> (i64, i64) {
        let lo = match (c.min, c.max) {
            (Some(m), _) => m.ceil() as i64,
            (None, Some(m)) => (m.floor() as i64).saturating_sub(1000),
            (None, None) => -50,
        };
        let lo = lo.clamp(floor, ceil);
        let hi = c.max.map_or(lo.saturating_add(1000), |m| m.floor() as i64);
        (lo, hi.clamp(lo, ceil))
    }

    /// Uniform in `lo..=hi`, with the bounds themselves over-represented.
    fn pick(&mut self, lo: i64, hi: i64) -> i64 {
        match self.rng.below(10) {
            0 => lo,
            1 => hi,
            _ => self.rng.range(lo, hi),
        }
    }

    /// A number with at most two decimals inside `@min`..`@max`, and whether it is a bound.
    fn fraction(&mut self, c: &Constraints) -> (f64, bool) {
        let lo = c.min.unwrap_or_else(|| c.max.map_or(-50.0, |m| m - 1000.0));
        let hi = c.max.unwrap_or(lo + 1000.0).max(lo);
        let x = lo + (hi - lo) * (self.rng.below(10_001) as f64 / 10_000.0);
        let rounded = (x * 100.0).round() / 100.0;
        if rounded < lo {
            (lo, true)
        } else if rounded > hi {
            (hi, true)
        } else {
            (if rounded == 0.0 { 0.0 } else { rounded }, false)
        }
    }

    fn decimal(&mut self, c: &Constraints) -> String {
        match self.fraction(c) {
            (v, true) => v.to_string(),
            (v, false) => format!("{v:.2}"),
        }
    }

    /// A datetime near the default clock, in whole milliseconds.
    pub fn datetime(&mut self) -> i64 {
        const UNITS: [i64; 5] = [1, 1000, 60_000, 3_600_000, MS_PER_DAY];
        let unit = UNITS[self.rng.index(UNITS.len())];
        self.clock + self.rng.range(-40, 40) * unit
    }

    fn uuid(&mut self) -> String {
        let hex: String = (0..32)
            .map(|_| char::from(b"0123456789abcdef"[self.rng.index(16)]))
            .collect();
        format!(
            "{}-{}-4{}-8{}-{}",
            &hex[0..8],
            &hex[8..12],
            &hex[13..16],
            &hex[17..20],
            &hex[20..32]
        )
    }
}

/// True for a string constraint that no random string can satisfy.
pub(crate) fn needs_example(c: &Constraints) -> bool {
    c.pattern.is_some() || c.format.is_some()
}

/// `@example`, then the default, as wire JSON.
fn preset(f: &Field) -> Option<Json> {
    let lit = f.example.as_ref().or(f.default.as_ref())?;
    literal_json(lit, &f.ty)
}

/// A literal as wire JSON, typed by the field: `long` and `decimal` are strings.
fn literal_json(lit: &Literal, ty: &Type) -> Option<Json> {
    let as_text = matches!(ty, Type::Scalar(Scalar::Long | Scalar::Decimal));
    match lit {
        Literal::Null => None,
        Literal::String(s) => Some(Json::String(s.clone())),
        Literal::Bool(b) => Some(Json::Bool(*b)),
        Literal::EnumVariant { wire, .. } => Some(Json::String(wire.clone())),
        Literal::Int(n) if as_text => Some(Json::String(n.to_string())),
        Literal::Int(n) => Some(Json::from(*n)),
        Literal::Float(x) if as_text => Some(Json::String(x.to_string())),
        Literal::Float(x) => Number::from_f64(*x).map(Json::Number),
        Literal::EmptyList if matches!(ty, Type::Map(_)) => Some(Json::Object(Map::new())),
        Literal::EmptyList => Some(Json::Array(Vec::new())),
    }
}

/// Padded standard base64.
fn base64(bytes: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(char::from(A[((n >> (18 - 6 * i)) & 63) as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_is_padded() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
    }
}

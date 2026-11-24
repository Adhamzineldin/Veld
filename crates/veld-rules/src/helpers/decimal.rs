//! Exact decimal numbers of any length, parsed from `-?\d+(\.\d+)?` text.

use std::cmp::Ordering;
use std::fmt;

/// A decimal number: `mag` is the little-endian digit string of the unscaled integer and the
/// value is `mag * 10^-scale`. Values are normalised (no leading zeros, no trailing fractional
/// zeros, zero is non-negative with scale 0), so derived equality is numeric equality.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Decimal {
    neg: bool,
    mag: Vec<u8>,
    scale: usize,
}

fn trim(v: &mut Vec<u8>) {
    while v.last() == Some(&0) {
        v.pop();
    }
}

fn cmp_mag(a: &[u8], b: &[u8]) -> Ordering {
    a.len()
        .cmp(&b.len())
        .then_with(|| a.iter().rev().cmp(b.iter().rev()))
}

fn add_mag(a: &[u8], b: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(a.len().max(b.len()) + 1);
    let mut carry = 0u8;
    for i in 0..a.len().max(b.len()) {
        let sum = a.get(i).copied().unwrap_or(0) + b.get(i).copied().unwrap_or(0) + carry;
        out.push(sum % 10);
        carry = sum / 10;
    }
    if carry > 0 {
        out.push(carry);
    }
    out
}

/// `a - b` for `a >= b`.
fn sub_mag(a: &[u8], b: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(a.len());
    let mut borrow = 0u8;
    for (i, &x) in a.iter().enumerate() {
        let sub = b.get(i).copied().unwrap_or(0) + borrow;
        if x >= sub {
            out.push(x - sub);
            borrow = 0;
        } else {
            out.push(x + 10 - sub);
            borrow = 1;
        }
    }
    trim(&mut out);
    out
}

fn mul_mag(a: &[u8], b: &[u8]) -> Vec<u8> {
    let mut out = vec![0u32; a.len() + b.len() + 1];
    for (i, &x) in a.iter().enumerate() {
        for (j, &y) in b.iter().enumerate() {
            if let Some(slot) = out.get_mut(i + j) {
                *slot += u32::from(x) * u32::from(y);
            }
        }
    }
    let mut digits = Vec::with_capacity(out.len());
    let mut carry = 0u32;
    for cell in out {
        let total = cell + carry;
        digits.push((total % 10) as u8);
        carry = total / 10;
    }
    trim(&mut digits);
    digits
}

impl Decimal {
    fn new(neg: bool, mut mag: Vec<u8>, mut scale: usize) -> Decimal {
        trim(&mut mag);
        if mag.is_empty() {
            return Decimal {
                neg: false,
                mag,
                scale: 0,
            };
        }
        while scale > 0 && mag.first() == Some(&0) {
            mag.remove(0);
            scale -= 1;
        }
        Decimal { neg, mag, scale }
    }

    /// Parses `-?\d+(\.\d+)?`; anything else is `None`.
    #[must_use]
    pub fn parse(text: &str) -> Option<Decimal> {
        let (neg, rest) = match text.strip_prefix('-') {
            Some(r) => (true, r),
            None => (false, text),
        };
        let (int, frac) = rest.split_once('.').unwrap_or((rest, ""));
        let dotted = rest.contains('.');
        let digits = |s: &str| s.bytes().all(|b| b.is_ascii_digit());
        if int.is_empty() || !digits(int) || !digits(frac) || (dotted && frac.is_empty()) {
            return None;
        }
        let mag = int
            .bytes()
            .chain(frac.bytes())
            .rev()
            .map(|b| b - b'0')
            .collect();
        Some(Decimal::new(neg, mag, frac.len()))
    }

    #[must_use]
    pub fn from_i64(n: i64) -> Decimal {
        let mut rest = n.unsigned_abs();
        let mut mag = Vec::new();
        while rest > 0 {
            mag.push((rest % 10) as u8);
            rest /= 10;
        }
        Decimal::new(n < 0, mag, 0)
    }

    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.mag.is_empty()
    }

    fn aligned(&self, scale: usize) -> Vec<u8> {
        let mut out = vec![0u8; scale.saturating_sub(self.scale)];
        out.extend_from_slice(&self.mag);
        trim(&mut out);
        out
    }

    #[must_use]
    pub fn negated(&self) -> Decimal {
        Decimal::new(!self.neg, self.mag.clone(), self.scale)
    }

    #[must_use]
    pub fn add(&self, other: &Decimal) -> Decimal {
        let scale = self.scale.max(other.scale);
        let (a, b) = (self.aligned(scale), other.aligned(scale));
        if self.neg == other.neg {
            return Decimal::new(self.neg, add_mag(&a, &b), scale);
        }
        match cmp_mag(&a, &b) {
            Ordering::Equal => Decimal::new(false, Vec::new(), 0),
            Ordering::Greater => Decimal::new(self.neg, sub_mag(&a, &b), scale),
            Ordering::Less => Decimal::new(other.neg, sub_mag(&b, &a), scale),
        }
    }

    #[must_use]
    pub fn sub(&self, other: &Decimal) -> Decimal {
        self.add(&other.negated())
    }

    #[must_use]
    pub fn mul(&self, other: &Decimal) -> Decimal {
        let mag = mul_mag(&self.mag, &other.mag);
        Decimal::new(self.neg != other.neg, mag, self.scale + other.scale)
    }

    /// Numeric comparison: `1.50` equals `1.5` and `-0` equals `0`.
    #[must_use]
    pub fn compare(&self, other: &Decimal) -> Ordering {
        if self.neg != other.neg {
            return if self.neg {
                Ordering::Less
            } else {
                Ordering::Greater
            };
        }
        let scale = self.scale.max(other.scale);
        let ord = cmp_mag(&self.aligned(scale), &other.aligned(scale));
        if self.neg { ord.reverse() } else { ord }
    }
}

impl PartialOrd for Decimal {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Decimal {
    fn cmp(&self, other: &Self) -> Ordering {
        self.compare(other)
    }
}

impl fmt::Display for Decimal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut digits: Vec<u8> = self.mag.clone();
        while digits.len() <= self.scale {
            digits.push(0);
        }
        let text: String = digits.iter().rev().map(|d| char::from(b'0' + d)).collect();
        let split = text.len() - self.scale;
        let sign = if self.neg { "-" } else { "" };
        match (text.get(..split), text.get(split..)) {
            (Some(int), Some(frac)) if self.scale > 0 => write!(f, "{sign}{int}.{frac}"),
            (Some(int), _) => write!(f, "{sign}{int}"),
            (None, _) => Err(fmt::Error),
        }
    }
}

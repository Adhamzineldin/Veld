//! Calendar and clock values as plain integers: RFC 3339 `datetime` text to epoch
//! milliseconds, `date` to days since the epoch, `time` to milliseconds since midnight.
//! Digits below the millisecond are dropped (truncated, never rounded).

pub const MS_PER_DAY: i64 = 86_400_000;

fn is_leap(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

fn days_in_month(y: i64, m: i64) -> i64 {
    match m {
        2 if is_leap(y) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// Days since 1970-01-01 of a proleptic Gregorian date (the month must be 1 to 12).
#[must_use]
pub fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The digits `b[from..to]` as a number; every byte must be an ASCII digit.
fn num(b: &[u8], from: usize, to: usize) -> Option<i64> {
    let part = b.get(from..to)?;
    part.iter().try_fold(0i64, |acc, c| {
        c.is_ascii_digit().then(|| acc * 10 + i64::from(c - b'0'))
    })
}

/// `YYYY-MM-DD` to days since the epoch; `None` for text that is not a real calendar day.
#[must_use]
pub fn parse_date(text: &str) -> Option<i64> {
    let b = text.as_bytes();
    if b.len() != 10 || b.get(4) != Some(&b'-') || b.get(7) != Some(&b'-') {
        return None;
    }
    let (y, m, d) = (num(b, 0, 4)?, num(b, 5, 7)?, num(b, 8, 10)?);
    if !(1..=12).contains(&m) || d < 1 || d > days_in_month(y, m) {
        return None;
    }
    Some(days_from_civil(y, m, d))
}

/// The first three fractional digits as milliseconds, padded with zeros.
fn frac_ms(digits: &[u8]) -> i64 {
    (0..3).fold(0, |acc, i| {
        acc * 10 + digits.get(i).map_or(0, |c| i64::from(c - b'0'))
    })
}

/// `HH:MM:SS[.f+]` at the start of `b`: the milliseconds since midnight and the bytes used.
fn clock(b: &[u8], max_sec: i64) -> Option<(i64, usize)> {
    if b.get(2) != Some(&b':') || b.get(5) != Some(&b':') {
        return None;
    }
    let (h, m, s) = (num(b, 0, 2)?, num(b, 3, 5)?, num(b, 6, 8)?);
    if h > 23 || m > 59 || s > max_sec {
        return None;
    }
    let base = (h * 3600 + m * 60 + s) * 1000;
    if b.get(8) != Some(&b'.') {
        return Some((base, 8));
    }
    let frac = b.get(9..)?;
    let n = frac.iter().take_while(|c| c.is_ascii_digit()).count();
    if n == 0 {
        return None;
    }
    Some((base + frac_ms(frac.get(..n)?), 9 + n))
}

/// `HH:MM:SS[.f+]` to milliseconds since midnight.
#[must_use]
pub fn parse_time(text: &str) -> Option<i64> {
    let b = text.as_bytes();
    let (ms, used) = clock(b, 60)?;
    (used == b.len()).then_some(ms)
}

/// `Z` or `+HH:MM` / `-HH:MM` to the offset in milliseconds.
fn offset_ms(tail: &[u8]) -> Option<i64> {
    if tail == b"Z" {
        return Some(0);
    }
    let sign = match tail.first()? {
        b'+' => 1,
        b'-' => -1,
        _ => return None,
    };
    if tail.len() != 6 || tail.get(3) != Some(&b':') {
        return None;
    }
    let (h, m) = (num(tail, 1, 3)?, num(tail, 4, 6)?);
    (h <= 23 && m <= 59).then_some(sign * (h * 60 + m) * 60_000)
}

/// RFC 3339 `datetime` text to milliseconds since the epoch, UTC. `None` when the text is not
/// `YYYY-MM-DDTHH:MM:SS[.f+](Z|+HH:MM|-HH:MM)` or not a real instant.
#[must_use]
pub fn parse_datetime(text: &str) -> Option<i64> {
    let b = text.as_bytes();
    let days = parse_date(text.get(..10)?)?;
    if b.get(10) != Some(&b'T') {
        return None;
    }
    let rest = b.get(11..)?;
    let (ms, used) = clock(rest, 59)?;
    let offset = offset_ms(rest.get(used..)?)?;
    Some(days * MS_PER_DAY + ms - offset)
}

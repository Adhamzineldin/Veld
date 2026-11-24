//! Direct tests of the helper modules, with the edge cases of `tests/rules/core/*.json`.

use std::cmp::Ordering;

use crate::helpers::decimal::Decimal;
use crate::helpers::instant::{MS_PER_DAY, parse_date, parse_datetime, parse_time};
use crate::helpers::num::{self, EvalError};
use crate::helpers::regex::Regex;
use crate::helpers::text;

fn dec(s: &str) -> Decimal {
    Decimal::parse(s).unwrap()
}

#[test]
fn decimals_compare_by_value_across_scales() {
    assert_eq!(dec("1.50"), dec("1.5"));
    assert_eq!(dec("1.50").compare(&dec("1.5")), Ordering::Equal);
    assert_eq!(dec("-0"), dec("0.00"));
    assert_eq!(dec("-0").compare(&dec("0.00")), Ordering::Equal);
    assert_eq!(
        dec("0.1").compare(&dec("0.10000000000000000001")),
        Ordering::Less
    );
    assert_eq!(dec("-2").compare(&dec("-1.5")), Ordering::Less);
    assert_eq!(dec("-2").compare(&dec("1")), Ordering::Less);
    assert_eq!(dec("10").compare(&dec("9.99")), Ordering::Greater);
}

#[test]
fn decimals_add_exactly() {
    assert_eq!(dec("0.1").add(&dec("0.2")), dec("0.3"));
    let a = dec("123456789012345678901234567890.123456789");
    let c = dec("123456789012345678901234567890.123456790");
    assert_eq!(a.add(&dec("0.000000001")), c);
    assert_eq!(c.sub(&dec("0.000000001")), a);
    assert_eq!(dec("1").sub(&dec("-1")), dec("2"));
    assert_eq!(dec("1").add(&dec("-1")), dec("0"));
    assert_eq!(dec("5").add(&dec("-7.5")), dec("-2.5"));
}

#[test]
fn decimals_multiply_exactly() {
    assert_eq!(dec("19.99").mul(&Decimal::from_i64(3)), dec("59.97"));
    assert_eq!(dec("1.005").mul(&Decimal::from_i64(2)), dec("2.01"));
    assert_ne!(dec("1.005").mul(&Decimal::from_i64(1)), dec("1.01"));
    assert_eq!(dec("2.5").mul(&Decimal::from_i64(-2)), dec("-5"));
    assert_eq!(
        dec("99999999999999999999").mul(&dec("99999999999999999999")),
        dec("9999999999999999999800000000000000000001")
    );
    assert!(dec("0.5").mul(&dec("0")).is_zero());
}

#[test]
fn decimals_reject_other_text() {
    for bad in [
        "", "-", "1.", ".5", "1e3", "+1", "1,5", " 1", "1.5.2", "--1", "abc",
    ] {
        assert!(Decimal::parse(bad).is_none(), "{bad:?}");
    }
    assert_eq!(dec("007.50").to_string(), "7.5");
    assert_eq!(dec("-0.05").to_string(), "-0.05");
    assert_eq!(dec("0").to_string(), "0");
}

#[test]
fn decimals_widen_from_integers() {
    assert_eq!(
        Decimal::from_i64(i64::MIN).to_string(),
        "-9223372036854775808"
    );
    assert_eq!(Decimal::from_i64(0), dec("0"));
}

#[test]
fn int_overflow_at_both_bounds() {
    assert_eq!(num::add_i32(i32::MAX, 1), Err(EvalError::Overflow));
    assert_eq!(num::add_i32(i32::MIN, -1), Err(EvalError::Overflow));
    assert_eq!(num::add_i32(60, 40), Ok(100));
    assert_eq!(num::mul_i32(46340, 46340), Ok(2_147_395_600));
    assert_eq!(num::mul_i32(46341, 46341), Err(EvalError::Overflow));
    assert_eq!(num::neg_i32(i32::MIN), Err(EvalError::Overflow));
    assert_eq!(num::sub_i32(i32::MIN, 1), Err(EvalError::Overflow));
}

#[test]
fn long_overflow_and_precision_beyond_2_pow_53() {
    assert_eq!(
        num::add_i64(9_007_199_254_740_992, 1),
        Ok(9_007_199_254_740_993)
    );
    assert_eq!(num::add_i64(i64::MAX, 1), Err(EvalError::Overflow));
    assert_eq!(num::mul_i64(i64::MAX, 2), Err(EvalError::Overflow));
    assert_eq!(num::neg_i64(i64::MIN), Err(EvalError::Overflow));
    assert_eq!(num::sub_i64(i64::MIN, 1), Err(EvalError::Overflow));
}

#[test]
fn division_truncates_and_the_remainder_follows_the_dividend() {
    assert_eq!(num::div_i32(-7, 2), Ok(-3));
    assert_eq!(num::rem_i32(-7, 2), Ok(-1));
    assert_eq!(num::div_i32(7, -2), Ok(-3));
    assert_eq!(num::rem_i32(7, -2), Ok(1));
    assert_eq!(num::div_i32(7, 2), Ok(3));
    assert_eq!(num::rem_i32(7, 2), Ok(1));
    assert_eq!(num::div_i64(-7, 2), Ok(-3));
    assert_eq!(num::rem_i64(-7, 2), Ok(-1));
}

#[test]
fn division_by_zero_and_the_minimum_over_minus_one() {
    assert_eq!(num::div_i32(1, 0), Err(EvalError::DivisionByZero));
    assert_eq!(num::rem_i32(1, 0), Err(EvalError::DivisionByZero));
    assert_eq!(num::div_i64(1, 0), Err(EvalError::DivisionByZero));
    assert_eq!(num::div_i32(i32::MIN, -1), Err(EvalError::Overflow));
    assert_eq!(num::rem_i32(i32::MIN, -1), Ok(0));
    assert_eq!(num::div_i64(i64::MIN, -1), Err(EvalError::Overflow));
    assert_eq!(num::rem_i64(i64::MIN, -1), Ok(0));
}

#[test]
fn floats_must_stay_finite_and_never_divide_by_zero() {
    assert_eq!(num::mul_f64(1e200, 1e200), Err(EvalError::NonFinite));
    assert_eq!(num::add_f64(f64::MAX, f64::MAX), Err(EvalError::NonFinite));
    assert_eq!(num::div_f64(1.0, 0.0), Err(EvalError::DivisionByZero));
    assert_eq!(num::div_f64(0.0, 0.0), Err(EvalError::DivisionByZero));
    assert_eq!(num::div_f64(1.0, -0.0), Err(EvalError::DivisionByZero));
    assert_eq!(num::rem_f64(1.0, 0.0), Err(EvalError::DivisionByZero));
    assert_eq!(num::div_f64(-1.0, 2.0), Ok(-0.5));
    assert_eq!(num::rem_f64(-7.0, 2.0), Ok(-1.0));
    assert_eq!(num::div_f64(f64::MAX, 0.5), Err(EvalError::NonFinite));
}

#[test]
fn durations_overflow_at_the_documented_boundary() {
    assert_eq!(num::days_ms(106_751_991_167), Ok(9_223_372_036_828_800_000));
    assert_eq!(num::days_ms(106_751_991_168), Err(EvalError::Overflow));
    assert!(num::days_ms(-106_751_991_167).is_ok());
    assert_eq!(num::days_ms(30), Ok(2_592_000_000));
    assert_eq!(num::hours_ms(2), Ok(7_200_000));
    assert_eq!(num::minutes_ms(120), Ok(7_200_000));
    assert_eq!(num::seconds_ms(1), Ok(1000));
    assert_eq!(num::hours_ms(i64::MAX), Err(EvalError::Overflow));
}

#[test]
fn error_names_are_the_wire_names() {
    assert_eq!(EvalError::Overflow.as_str(), "overflow");
    assert_eq!(EvalError::DivisionByZero.as_str(), "division_by_zero");
    assert_eq!(EvalError::NonFinite.as_str(), "non_finite");
}

#[test]
fn datetimes_in_any_offset_are_one_instant() {
    let utc = parse_datetime("2026-01-01T00:00:00Z").unwrap();
    assert_eq!(parse_datetime("2026-01-01T02:00:00+02:00"), Some(utc));
    assert_eq!(parse_datetime("2025-12-31T21:30:00-02:30"), Some(utc));
    assert_eq!(
        parse_datetime("2026-03-01T13:00:00+03:00"),
        parse_datetime("2026-03-01T10:00:00Z")
    );
    assert_eq!(parse_datetime("1970-01-01T00:00:00Z"), Some(0));
    assert_eq!(parse_datetime("1969-12-31T23:59:59.999Z"), Some(-1));
    assert_eq!(
        parse_datetime("2026-01-01T00:00:00Z"),
        Some(1_767_225_600_000)
    );
}

#[test]
fn datetime_digits_below_a_millisecond_are_dropped() {
    let zero = parse_datetime("2026-01-01T00:00:00.000Z").unwrap();
    assert_eq!(parse_datetime("2026-01-01T00:00:00.000999Z"), Some(zero));
    assert_eq!(parse_datetime("2026-01-01T00:00:00.0019Z"), Some(zero + 1));
    assert_eq!(parse_datetime("2026-01-01T00:00:00.5Z"), Some(zero + 500));
    assert_eq!(
        parse_datetime("2026-01-01T00:00:00.12345678901Z"),
        Some(zero + 123)
    );
    assert_eq!(
        parse_datetime("2026-01-31T00:00:00.001Z").unwrap() - zero,
        30 * MS_PER_DAY + 1
    );
}

#[test]
fn datetimes_reject_text_that_is_not_an_instant() {
    for bad in [
        "2026-01-01",
        "2026-01-01T00:00:00",
        "2026-01-01 00:00:00Z",
        "2026-13-01T00:00:00Z",
        "2026-02-29T00:00:00Z",
        "2026-01-01T24:00:00Z",
        "2026-01-01T00:60:00Z",
        "2026-01-01T00:00:60Z",
        "2026-01-01T00:00:00.Z",
        "2026-01-01T00:00:00+0200",
        "2026-01-01T00:00:00+24:00",
        "2026-01-01t00:00:00z",
        "",
        "é026-01-01T00:00:00Z",
    ] {
        assert_eq!(parse_datetime(bad), None, "{bad:?}");
    }
}

#[test]
fn date_differences_know_leap_years() {
    let days = |a: &str, b: &str| parse_date(b).unwrap() - parse_date(a).unwrap();
    assert_eq!(days("2026-02-25", "2026-03-04"), 7);
    assert_eq!(days("2024-02-25", "2024-03-04"), 8);
    assert_eq!(days("2026-03-04", "2026-02-25"), -7);
    assert_eq!(parse_date("1970-01-01"), Some(0));
    assert_eq!(parse_date("1969-12-31"), Some(-1));
    assert!(parse_date("2000-02-29").is_some());
    assert_eq!(parse_date("1900-02-29"), None);
    assert_eq!(parse_date("2026-04-31"), None);
    assert_eq!(parse_date("2026-1-01"), None);
}

#[test]
fn times_truncate_below_a_millisecond() {
    assert_eq!(parse_time("09:00:00.500"), parse_time("09:00:00.5001"));
    assert_eq!(parse_time("09:00:00.500"), Some(9 * 3_600_000 + 500));
    assert_eq!(parse_time("00:00:00"), Some(0));
    assert_eq!(parse_time("23:59:59"), Some(86_399_000));
    assert!(parse_time("23:59:59").unwrap() > parse_time("00:00:00").unwrap());
    assert_eq!(parse_time("24:00:00"), None);
    assert_eq!(parse_time("09:00"), None);
    assert_eq!(parse_time("09:00:00."), None);
    assert_eq!(parse_time("09:00:00Z"), None);
}

#[test]
fn length_counts_code_points() {
    assert_eq!(text::len_code_points("abc"), 3);
    assert_eq!(text::len_code_points("\u{1F600}\u{1F600}\u{1F600}"), 3);
    assert_eq!(text::len_code_points("e\u{301}e\u{301}"), 4);
    assert_eq!(text::len_code_points("\u{e9}\u{e9}"), 2);
    assert_eq!(text::len_code_points(""), 0);
}

#[test]
fn string_tests_do_not_normalise() {
    assert!(text::starts_with("admin1", "admin"));
    assert!(text::ends_with("caf\u{1F600}", "\u{1F600}"));
    assert!(text::contains("caf\u{e9}", "\u{e9}"));
    assert!(!text::contains("cafe\u{301}", "\u{e9}"));
    assert!(!text::ends_with("\u{e9}", "e\u{301}"));
}

#[test]
fn uuids_compare_in_lower_case() {
    let upper = "A1B2C3D4-0000-0000-0000-000000000001";
    let lower = "a1b2c3d4-0000-0000-0000-000000000001";
    assert!(text::uuid_eq(upper, lower));
    assert_eq!(text::normalize_uuid(upper).as_deref(), Some(lower));
    assert_eq!(text::normalize_uuid("not-a-uuid"), None);
    assert_eq!(
        text::normalize_uuid("a1b2c3d4-0000-0000-0000-00000000000g"),
        None
    );
    assert_eq!(
        text::normalize_uuid("a1b2c3d400000000000000000000000001"),
        None
    );
}

#[test]
fn base64_length_counts_bytes_without_decoding() {
    assert_eq!(text::base64_byte_len(""), Some(0));
    assert_eq!(text::base64_byte_len("AQID"), Some(3));
    assert_eq!(text::base64_byte_len("AQIDBA=="), Some(4));
    assert_eq!(text::base64_byte_len("AQIDBAU="), Some(5));
    assert_eq!(text::base64_byte_len("AQI"), None);
    assert_eq!(text::base64_byte_len("AQ=D"), None);
    assert_eq!(text::base64_byte_len("A==="), None);
    assert_eq!(text::base64_byte_len("AQ!D"), None);
}

#[test]
fn the_shared_regex_matcher_is_the_ecmascript_subset() {
    let re = Regex::new(r"^[A-Z]{3}-\d{4}$").unwrap();
    assert!(re.is_match("ABC-1234"));
    assert!(!re.is_match("abc-1234"));
    assert!(Regex::new(r"\d").unwrap().is_match("ABC1"));
    assert!(!Regex::new(r"\d").unwrap().is_match("ABCD"));
    assert!(!Regex::new(r"^\d$").unwrap().is_match("\u{661}"));
}

#[test]
fn ms_is_the_identity_on_milliseconds() {
    assert_eq!(num::ms_ms(500), Ok(500));
    assert_eq!(num::ms_ms(-1), Ok(-1));
}

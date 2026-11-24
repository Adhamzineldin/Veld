//! Checked arithmetic with the three evaluation errors of `spec/rules.md` section 3.7.

/// What can go wrong while a rule is evaluated. The rule then counts as violated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EvalError {
    Overflow,
    DivisionByZero,
    NonFinite,
}

impl EvalError {
    /// The wire name of the error (`"overflow"`, `"division_by_zero"`, `"non_finite"`).
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            EvalError::Overflow => "overflow",
            EvalError::DivisionByZero => "division_by_zero",
            EvalError::NonFinite => "non_finite",
        }
    }
}

pub type Res<T> = Result<T, EvalError>;

macro_rules! int_ops {
    ($t:ty, $add:ident, $sub:ident, $mul:ident, $div:ident, $rem:ident, $neg:ident) => {
        pub fn $add(a: $t, b: $t) -> Res<$t> {
            a.checked_add(b).ok_or(EvalError::Overflow)
        }
        pub fn $sub(a: $t, b: $t) -> Res<$t> {
            a.checked_sub(b).ok_or(EvalError::Overflow)
        }
        pub fn $mul(a: $t, b: $t) -> Res<$t> {
            a.checked_mul(b).ok_or(EvalError::Overflow)
        }
        /// Truncates toward zero. The smallest value divided by -1 overflows.
        pub fn $div(a: $t, b: $t) -> Res<$t> {
            if b == 0 {
                return Err(EvalError::DivisionByZero);
            }
            a.checked_div(b).ok_or(EvalError::Overflow)
        }
        /// Takes the sign of the dividend. It never overflows: the smallest value `% -1` is 0.
        pub fn $rem(a: $t, b: $t) -> Res<$t> {
            if b == 0 {
                return Err(EvalError::DivisionByZero);
            }
            Ok(a.wrapping_rem(b))
        }
        pub fn $neg(a: $t) -> Res<$t> {
            a.checked_neg().ok_or(EvalError::Overflow)
        }
    };
}

int_ops!(i32, add_i32, sub_i32, mul_i32, div_i32, rem_i32, neg_i32);
int_ops!(i64, add_i64, sub_i64, mul_i64, div_i64, rem_i64, neg_i64);

/// A float result must be finite.
pub fn finite(x: f64) -> Res<f64> {
    if x.is_finite() {
        Ok(x)
    } else {
        Err(EvalError::NonFinite)
    }
}

pub fn add_f64(a: f64, b: f64) -> Res<f64> {
    finite(a + b)
}

pub fn sub_f64(a: f64, b: f64) -> Res<f64> {
    finite(a - b)
}

pub fn mul_f64(a: f64, b: f64) -> Res<f64> {
    finite(a * b)
}

/// The zero divisor is checked before dividing.
pub fn div_f64(a: f64, b: f64) -> Res<f64> {
    if b == 0.0 {
        return Err(EvalError::DivisionByZero);
    }
    finite(a / b)
}

/// Takes the sign of the dividend. The zero divisor is checked before dividing.
pub fn rem_f64(a: f64, b: f64) -> Res<f64> {
    if b == 0.0 {
        return Err(EvalError::DivisionByZero);
    }
    finite(a % b)
}

/// `days(n)` in milliseconds.
pub fn days_ms(n: i64) -> Res<i64> {
    mul_i64(n, 86_400_000)
}

/// `hours(n)` in milliseconds.
pub fn hours_ms(n: i64) -> Res<i64> {
    mul_i64(n, 3_600_000)
}

/// `minutes(n)` in milliseconds.
pub fn minutes_ms(n: i64) -> Res<i64> {
    mul_i64(n, 60_000)
}

/// `seconds(n)` in milliseconds.
pub fn seconds_ms(n: i64) -> Res<i64> {
    mul_i64(n, 1_000)
}

/// `ms(n)`: a count of milliseconds is already a duration.
pub fn ms_ms(n: i64) -> Res<i64> {
    Ok(n)
}

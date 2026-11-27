//! The built-in functions of `spec/rules.md` section 3.5, except the comprehensions.

use veld_ir::Builtin;

use crate::error::{EvalResult, InputError};
use crate::helpers::num::{self, EvalError};
use crate::helpers::regex::Regex;
use crate::helpers::text;
use crate::value::Value;

fn arity(f: Builtin, want: usize, got: usize) -> InputError {
    InputError::Ir(format!("`{f:?}` takes {want} arguments, got {got}"))
}

fn one(f: Builtin, args: &[Value]) -> Result<&Value, InputError> {
    match args {
        [a] => Ok(a),
        _ => Err(arity(f, 1, args.len())),
    }
}

fn two_strings(f: Builtin, args: &[Value]) -> Result<(&str, &str), InputError> {
    match args {
        [Value::Str(a), Value::Str(b)] => Ok((a, b)),
        [_, _] => Err(InputError::Ir(format!("`{f:?}` takes two strings"))),
        _ => Err(arity(f, 2, args.len())),
    }
}

fn count(n: usize) -> EvalResult<Value> {
    i32::try_from(n)
        .map(Value::Int)
        .map_err(|_| EvalError::Overflow.into())
}

fn len(f: Builtin, args: &[Value]) -> EvalResult<Value> {
    let arg = one(f, args)?;
    let n = if let Value::Str(s) = arg {
        text::len_code_points(s)
    } else if let Value::List(items) = arg {
        items.len()
    } else if let Value::Map(entries) = arg {
        entries.len()
    } else if let Value::Bytes(b) = arg {
        text::base64_byte_len(b).ok_or_else(|| InputError::ir("bad base64"))?
    } else {
        return Err(InputError::Ir(format!("`len` of {}", arg.kind())).into());
    };
    count(n)
}

/// `matches(s, "re")`: the regex subset of the language, searching anywhere unless anchored.
fn matches(f: Builtin, args: &[Value]) -> EvalResult<Value> {
    let (s, pattern) = two_strings(f, args)?;
    let re =
        Regex::new(pattern).ok_or_else(|| InputError::Unsupported(format!("regex `{pattern}`")))?;
    Ok(Value::Bool(re.is_match(s)))
}

fn milliseconds(f: Builtin, args: &[Value]) -> Result<i64, InputError> {
    let arg = one(f, args)?;
    if let Value::Int(n) = arg {
        Ok(i64::from(*n))
    } else if let Value::Long(n) = arg {
        Ok(*n)
    } else {
        Err(InputError::Ir(format!("`{f:?}` of {}", arg.kind())))
    }
}

/// Calls the built-in `f` on evaluated arguments.
pub fn call(f: Builtin, args: &[Value]) -> EvalResult<Value> {
    match f {
        Builtin::Len => len(f, args),
        Builtin::Matches => matches(f, args),
        Builtin::StartsWith => {
            let (s, p) = two_strings(f, args)?;
            Ok(Value::Bool(text::starts_with(s, p)))
        }
        Builtin::EndsWith => {
            let (s, p) = two_strings(f, args)?;
            Ok(Value::Bool(text::ends_with(s, p)))
        }
        Builtin::Contains => {
            let (s, p) = two_strings(f, args)?;
            Ok(Value::Bool(text::contains(s, p)))
        }
        Builtin::Days => Ok(Value::Duration(num::days_ms(milliseconds(f, args)?)?)),
        Builtin::Hours => Ok(Value::Duration(num::hours_ms(milliseconds(f, args)?)?)),
        Builtin::Minutes => Ok(Value::Duration(num::minutes_ms(milliseconds(f, args)?)?)),
        Builtin::Seconds => Ok(Value::Duration(num::seconds_ms(milliseconds(f, args)?)?)),
        Builtin::Ms => Ok(Value::Duration(num::ms_ms(milliseconds(f, args)?)?)),
    }
}

//! Values back to their wire form, and the calendar text of the clock.

use serde_json::Value as Json;

use crate::error::InputError;
use crate::helpers::instant::MS_PER_DAY;
use crate::value::Value;

/// Days since the epoch to `(year, month, day)`.
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

/// `YYYY-MM-DD`.
#[must_use]
pub fn format_date(days: i64) -> String {
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02}")
}

/// `HH:MM:SS`, with `.mmm` when the milliseconds are not zero.
#[must_use]
pub fn format_time(ms_of_day: i64) -> String {
    let ms = ms_of_day.rem_euclid(MS_PER_DAY);
    let (h, m, s, f) = (ms / 3_600_000, ms / 60_000 % 60, ms / 1000 % 60, ms % 1000);
    if f == 0 {
        format!("{h:02}:{m:02}:{s:02}")
    } else {
        format!("{h:02}:{m:02}:{s:02}.{f:03}")
    }
}

/// `YYYY-MM-DDTHH:MM:SS[.mmm]Z` for milliseconds since the epoch.
#[must_use]
pub fn format_datetime(ms: i64) -> String {
    let days = ms.div_euclid(MS_PER_DAY);
    format!("{}T{}Z", format_date(days), format_time(ms))
}

impl Value {
    /// The wire form of the value.
    ///
    /// # Errors
    /// [`InputError`] for a `duration` (it has no wire form) or a float that is not finite.
    pub fn to_json(&self) -> Result<Json, InputError> {
        Ok(match self {
            Value::Bool(b) => Json::Bool(*b),
            Value::Int(n) => Json::from(*n),
            Value::Long(n) => Json::String(n.to_string()),
            Value::Float(x) => serde_json::Number::from_f64(*x)
                .map(Json::Number)
                .ok_or_else(|| InputError::ir("a float that is not finite has no wire form"))?,
            Value::Decimal(d) => Json::String(d.to_string()),
            Value::Str(s) | Value::Uuid(s) | Value::Bytes(s) | Value::Enum(s) => {
                Json::String(s.clone())
            }
            Value::Date(days) => Json::String(format_date(*days)),
            Value::Datetime(ms) => Json::String(format_datetime(*ms)),
            Value::Time(ms) => Json::String(format_time(*ms)),
            Value::Duration(_) => {
                return Err(InputError::Unsupported(
                    "a duration has no wire form".to_owned(),
                ));
            }
            Value::List(items) => {
                let out: Result<Vec<Json>, InputError> = items.iter().map(Value::to_json).collect();
                Json::Array(out?)
            }
            Value::Map(entries) => {
                let mut out = serde_json::Map::new();
                for (k, v) in entries {
                    out.insert(k.clone(), v.to_json()?);
                }
                Json::Object(out)
            }
            Value::Raw(j) => j.clone(),
        })
    }
}

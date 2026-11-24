//! Errors of the evaluator. An [`InputError`] means the value, the IR or the context is
//! wrong; it is never a rule verdict. Verdicts and evaluation errors (`overflow`, ...) are
//! [`crate::EvalError`].

use crate::helpers::num::EvalError;

/// Malformed input: a value that does not match its declared type, a missing required field,
/// something this evaluator cannot do yet, or an IR that is not well formed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum InputError {
    #[error("{}missing required field `{field}`", at(.path))]
    MissingField { path: String, field: String },
    #[error("{}expected {expected}, found {found}", at(.path))]
    Mismatch {
        path: String,
        expected: String,
        found: String,
    },
    #[error("{}`{text}` is not a valid {ty}", at(.path))]
    InvalidScalar {
        path: String,
        ty: &'static str,
        text: String,
    },
    #[error("`{0}` is not available here")]
    NotAvailable(&'static str),
    #[error("not supported: {0}")]
    Unsupported(String),
    #[error("unknown declaration `{0}`")]
    UnknownDecl(String),
    #[error("malformed IR: {0}")]
    Ir(String),
}

fn at(path: &str) -> String {
    if path.is_empty() {
        String::new()
    } else {
        format!("{path}: ")
    }
}

impl InputError {
    pub(crate) fn mismatch(expected: &str, found: &str) -> InputError {
        InputError::Mismatch {
            path: String::new(),
            expected: expected.to_owned(),
            found: found.to_owned(),
        }
    }

    pub(crate) fn ir(what: &str) -> InputError {
        InputError::Ir(what.to_owned())
    }

    /// The same error with the path of the value it is about.
    #[must_use]
    pub fn at_path(self, to: &str) -> InputError {
        match self {
            InputError::MissingField { path, field } if path.is_empty() => {
                InputError::MissingField {
                    path: to.to_owned(),
                    field,
                }
            }
            InputError::Mismatch {
                path,
                expected,
                found,
            } if path.is_empty() => InputError::Mismatch {
                path: to.to_owned(),
                expected,
                found,
            },
            InputError::InvalidScalar { path, ty, text } if path.is_empty() => {
                InputError::InvalidScalar {
                    path: to.to_owned(),
                    ty,
                    text,
                }
            }
            other @ (InputError::MissingField { .. }
            | InputError::Mismatch { .. }
            | InputError::InvalidScalar { .. }
            | InputError::NotAvailable(_)
            | InputError::Unsupported(_)
            | InputError::UnknownDecl(_)
            | InputError::Ir(_)) => other,
        }
    }
}

/// Why evaluating an expression did not produce a value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    /// Overflow, division by zero or a non-finite float: the rule fails closed.
    Eval(EvalError),
    /// The input is malformed or outside what the evaluator supports.
    Input(InputError),
}

impl From<EvalError> for Failure {
    fn from(e: EvalError) -> Failure {
        Failure::Eval(e)
    }
}

impl From<InputError> for Failure {
    fn from(e: InputError) -> Failure {
        Failure::Input(e)
    }
}

pub(crate) type EvalResult<T> = Result<T, Failure>;

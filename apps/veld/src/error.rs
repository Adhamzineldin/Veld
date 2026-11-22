//! Exit codes and error classification: 0 ok, 1 user or contract error, 2 internal or usage error.

use crate::ui::Ui;

/// How a command ended when it did not fail with an error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit {
    Ok,
    /// The user's contracts, config or files are at fault.
    User,
    /// Not implemented, or an internal failure.
    Internal,
    /// The user cancelled a prompt (Ctrl-C): 130, like a shell does.
    Cancelled,
}

impl From<Exit> for std::process::ExitCode {
    fn from(e: Exit) -> Self {
        std::process::ExitCode::from(match e {
            Exit::Ok => 0,
            Exit::User => 1,
            Exit::Internal => 2,
            Exit::Cancelled => 130,
        })
    }
}

/// An error caused by the user's input. It exits with code 1 and prints without a prefix.
#[derive(Debug)]
pub struct UserError(pub String);

impl std::fmt::Display for UserError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for UserError {}

/// Shorthand for an `anyhow::Error` that exits with code 1.
pub fn user(message: impl Into<String>) -> anyhow::Error {
    anyhow::Error::new(UserError(message.into()))
}

/// Prints `err` and returns the exit code it maps to.
pub fn report(ui: &Ui, err: &anyhow::Error) -> Exit {
    if err.downcast_ref::<UserError>().is_some() {
        ui.error(&format!("{err}"));
        return Exit::User;
    }
    if err
        .downcast_ref::<veld_project::config::ConfigError>()
        .is_some()
    {
        ui.error(&format!("{err}"));
        return Exit::User;
    }
    ui.error(&format!("internal error: {err:#}"));
    Exit::Internal
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_errors_map_to_exit_one() {
        let ui = Ui::plain();
        assert_eq!(report(&ui, &user("bad contract")), Exit::User);
        assert_eq!(report(&ui, &anyhow::anyhow!("boom")), Exit::Internal);
    }
}

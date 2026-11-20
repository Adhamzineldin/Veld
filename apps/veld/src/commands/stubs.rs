//! Commands that are reserved but not built yet. They exit with code 2.

use crate::error::Exit;
use crate::ui::Ui;

pub fn coming_soon(ui: &Ui, name: &str) -> Exit {
    ui.error(&format!("`veld {name}` is coming soon"));
    Exit::Internal
}

pub fn not_yet(ui: &Ui, name: &str, what: &str) -> Exit {
    ui.error(&format!("`veld {name}` is not yet implemented ({what} comes later)"));
    Exit::Internal
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stubs_exit_with_two() {
        let ui = Ui::plain();
        assert_eq!(coming_soon(&ui, "fmt"), Exit::Internal);
        assert_eq!(not_yet(&ui, "lsp", "the language server"), Exit::Internal);
    }
}

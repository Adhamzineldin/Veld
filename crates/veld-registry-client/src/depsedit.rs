//! Format-preserving edits of `veld.toml` (comments and layout survive).

use toml_edit::{DocumentMut, Item, Table, value};

use crate::error::{RegistryError, Result};

fn invalid(e: impl std::fmt::Display) -> RegistryError {
    RegistryError::Other(format!("cannot edit veld.toml: {e}"))
}

/// Sets `[dependencies] "<name>" = "<range>"`. When `registry` is given and the file has no
/// `[registry]` section, one is added so the pin and the config agree.
pub fn set_dependency(
    text: &str,
    name: &str,
    range: &str,
    registry: Option<&str>,
) -> Result<String> {
    let mut doc: DocumentMut = text.parse().map_err(invalid)?;
    let deps = doc
        .entry("dependencies")
        .or_insert_with(|| Item::Table(Table::new()));
    let table = deps
        .as_table_mut()
        .ok_or_else(|| invalid("`dependencies` is not a table"))?;
    if let Some(existing) = table.get_mut(name) {
        *existing = value(range);
    } else {
        table.insert(name, value(range));
    }
    if let Some(url) = registry
        && !doc.contains_key("registry")
    {
        let mut t = Table::new();
        t.insert("url", value(url));
        doc.insert("registry", Item::Table(t));
    }
    Ok(doc.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: &str = "# my project\n[service]\nname = \"shop\" # the name\n\n[client]\nemitter = \"typescript\"\nout = \"gen\"\n";

    #[test]
    fn adds_a_dependencies_table_and_preserves_everything_else() {
        let out =
            set_dependency(BASE, "@acme/billing", "^1.2.0", Some("https://r.example")).unwrap();
        assert!(
            out.starts_with(BASE.trim_end()),
            "existing text is untouched:\n{out}"
        );
        assert!(out.contains("# the name"));
        assert!(out.contains("[dependencies]"));
        assert!(out.contains("\"@acme/billing\" = \"^1.2.0\""));
        assert!(out.contains("[registry]") && out.contains("url = \"https://r.example\""));
        let parsed: toml::Table = out.parse().unwrap();
        assert_eq!(
            parsed["dependencies"]["@acme/billing"].as_str(),
            Some("^1.2.0")
        );
    }

    #[test]
    fn updates_in_place_and_keeps_an_existing_registry() {
        let with = format!(
            "{BASE}\n[registry]\nurl = \"https://mine\"\n\n[dependencies]\n# billing\n\"@acme/billing\" = \"^1.0\"\n\"@acme/other\" = \"*\"\n"
        );
        let out = set_dependency(&with, "@acme/billing", "^1.5.0", Some("https://other")).unwrap();
        assert!(out.contains("# billing"));
        assert!(out.contains("\"@acme/other\" = \"*\""));
        assert!(out.contains("\"@acme/billing\" = \"^1.5.0\""));
        assert!(out.contains("url = \"https://mine\"") && !out.contains("https://other"));
        assert_eq!(out.matches("@acme/billing").count(), 1);
    }

    #[test]
    fn rejects_unparseable_input() {
        assert!(set_dependency("= = =", "@a/b", "*", None).is_err());
        assert!(set_dependency("dependencies = 3\n", "@a/b", "*", None).is_err());
    }
}

//! `E0709`: a `veld.toml` in the layout of earlier betas (`spec/workspace.md`, section 12).

use toml::{Table, Value};

use super::error::ConfigError;

/// The tables of the earlier layout, and what replaces each.
const EARLIER: &[&str] = &["project", "services", "libraries", "targets"];

/// Fails with `E0709` when `root` holds a table of the earlier layout.
pub(super) fn check(root: &Table) -> Result<(), ConfigError> {
    let found: Vec<String> = EARLIER
        .iter()
        .filter(|k| root.contains_key(**k))
        .flat_map(|k| found_tables(root, k))
        .collect();
    if found.is_empty() {
        return Ok(());
    }
    Err(ConfigError::EarlierLayout {
        found: found.join(", "),
        help: help(root),
    })
}

/// `[project]`, `[services.orders]`, `[[targets]]`, as the file writes them.
fn found_tables(root: &Table, key: &str) -> Vec<String> {
    match (key, root.get(key)) {
        ("targets", _) => vec!["`[[targets]]`".into()],
        ("services" | "libraries", Some(Value::Table(t))) if !t.is_empty() => {
            t.keys().map(|n| format!("`[{key}.{n}]`")).collect()
        }
        _ => vec![format!("`[{key}]`")],
    }
}

fn help(root: &Table) -> String {
    let mut lines = vec![
        "put a `[workspace]` table with `name` and `members` in the root `veld.toml`, and one \
         `veld.toml` per member in the member's own directory"
            .to_string(),
    ];
    let mut row = |has: bool, old: &str, new: &str| {
        if has {
            lines.push(format!("{old} becomes {new}"));
        }
    };
    row(
        root.contains_key("project"),
        "`[project] name`",
        "`[workspace] name`, or the `name` of the only member",
    );
    row(
        root.contains_key("project"),
        "`[project] contracts` and `prefix`",
        "`[service] contracts` and `[workspace] prefix`",
    );
    row(
        root.contains_key("services"),
        "`[services.x]`",
        "`[service]` in the `veld.toml` of member `x`",
    );
    row(
        root.contains_key("libraries"),
        "`[libraries.x]`",
        "`[library]` in the `veld.toml` of member `x`",
    );
    row(
        root.contains_key("targets"),
        "`[[targets]]` with `role = \"server\"` and `service = \"x\"`",
        "`[server]` in the `veld.toml` of member `x`",
    );
    row(
        root.contains_key("targets"),
        "`[[targets]]` with `role = \"client\"` and `services = [...]`",
        "`[client]` in an `[app]` member with `uses = [...]`, or in the service's own file",
    );
    row(
        root.contains_key("targets"),
        "`[targets.options]`",
        "`[server.options]` or `[client.options]`",
    );
    lines.push("see docs/guides/workspaces.md for a worked migration".into());
    lines.join("; ")
}

#[cfg(test)]
mod tests {
    use super::super::raw::FileConfig;

    #[test]
    fn every_earlier_table_is_e0709_and_named() {
        for (text, named) in [
            ("[project]\nname = \"x\"\n", "`[project]`"),
            (
                "[services.orders]\ncontracts = \"c\"\n",
                "`[services.orders]`",
            ),
            (
                "[libraries.common]\ncontracts = \"c\"\n",
                "`[libraries.common]`",
            ),
            (
                "[service]\nname = \"x\"\n[[targets]]\nemitter = \"node\"\n",
                "`[[targets]]`",
            ),
        ] {
            let err = FileConfig::parse(text).unwrap_err();
            assert_eq!(err.code(), Some("E0709"), "{text}");
            let msg = err.to_string();
            assert!(msg.contains(named), "{msg}");
            assert!(msg.contains("becomes"), "{msg}");
        }
    }

    #[test]
    fn the_message_says_what_to_write_instead() {
        let err = FileConfig::parse(
            "[project]\nname = \"x\"\n[services.a]\ncontracts = \"c\"\n[[targets]]\nrole = \"server\"\n",
        )
        .unwrap_err()
        .to_string();
        assert!(
            err.contains("`[project]`") && err.contains("`[services.a]`"),
            "{err}"
        );
        assert!(err.contains("`[workspace] name`"), "{err}");
        assert!(
            err.contains("`[service]` in the `veld.toml` of member `x`"),
            "{err}"
        );
        assert!(
            err.contains("`[server]` in the `veld.toml` of member `x`"),
            "{err}"
        );
    }
}

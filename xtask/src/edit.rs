//! Minimal, idempotent text edits of files the scaffolder must touch. Every function returns
//! the new text, and returns the input unchanged when the edit is already there.

use anyhow::{Result, bail};

/// Adds `"<member>",` to `[workspace] members`, after the last emitter (or before `apps/veld`).
pub fn insert_member(cargo_toml: &str, member: &str) -> Result<String> {
    let quoted = format!("\"{member}\"");
    let lines: Vec<&str> = cargo_toml.lines().collect();
    let Some(start) = lines
        .iter()
        .position(|l| l.trim_start().starts_with("members"))
    else {
        bail!("the root Cargo.toml has no `members = [` list");
    };
    let end = lines[start..]
        .iter()
        .position(|l| l.trim() == "]")
        .map(|i| start + i);
    let Some(end) = end else {
        bail!("the `members` list is not closed")
    };
    if lines[start..end].iter().any(|l| l.contains(&quoted)) {
        return Ok(cargo_toml.to_string());
    }
    let after_emitters = lines[start..end]
        .iter()
        .rposition(|l| l.contains("\"crates/emitters/"))
        .map(|i| start + i + 1);
    let before_app = lines[start..end]
        .iter()
        .position(|l| l.contains("\"apps/"))
        .map(|i| start + i);
    let at = after_emitters.or(before_app).unwrap_or(end);
    Ok(insert_line(&lines, at, &format!("  {quoted},"), cargo_toml))
}

/// Adds `<name> = { path = "<path>" }` to `[workspace.dependencies]`.
pub fn insert_workspace_dep(cargo_toml: &str, name: &str, path: &str) -> Result<String> {
    let lines: Vec<&str> = cargo_toml.lines().collect();
    let Some(start) = lines
        .iter()
        .position(|l| l.trim() == "[workspace.dependencies]")
    else {
        bail!("the root Cargo.toml has no `[workspace.dependencies]`");
    };
    let end = section_end(&lines, start);
    if lines[start..end]
        .iter()
        .any(|l| l.starts_with(&format!("{name} ")))
    {
        return Ok(cargo_toml.to_string());
    }
    let at = lines[start..end]
        .iter()
        .rposition(|l| l.starts_with("veld-emit-"))
        .or_else(|| {
            lines[start..end]
                .iter()
                .rposition(|l| l.starts_with("veld-"))
        })
        .map_or(start + 1, |i| start + i + 1);
    Ok(insert_line(
        &lines,
        at,
        &format!("{name} = {{ path = \"{path}\" }}"),
        cargo_toml,
    ))
}

/// Adds `<name>.workspace = true` to `[dependencies]`, after the last emitter dependency.
pub fn insert_app_dep(cargo_toml: &str, name: &str) -> Result<String> {
    let lines: Vec<&str> = cargo_toml.lines().collect();
    let Some(start) = lines.iter().position(|l| l.trim() == "[dependencies]") else {
        bail!("apps/veld/Cargo.toml has no `[dependencies]`");
    };
    let end = section_end(&lines, start);
    if lines[start..end]
        .iter()
        .any(|l| l.starts_with(&format!("{name}.")) || l.starts_with(&format!("{name} ")))
    {
        return Ok(cargo_toml.to_string());
    }
    let at = lines[start..end]
        .iter()
        .rposition(|l| l.starts_with("veld-emit-"))
        .map_or(end, |i| start + i + 1);
    Ok(insert_line(
        &lines,
        at,
        &format!("{name}.workspace = true"),
        cargo_toml,
    ))
}

/// The line index where the section that starts at `start` ends (next `[` header or the end).
fn section_end(lines: &[&str], start: usize) -> usize {
    lines[start + 1..]
        .iter()
        .position(|l| l.trim_start().starts_with('['))
        .map_or(lines.len(), |i| start + 1 + i)
}

fn insert_line(lines: &[&str], at: usize, new: &str, original: &str) -> String {
    let mut out: Vec<&str> = lines.to_vec();
    out.insert(at, new);
    let mut text = out.join("\n");
    if original.ends_with('\n') {
        text.push('\n');
    }
    text
}

/// Registers the generator in `apps/veld/src/registry.rs`: one sorted `use` line and one
/// line in the `generators()` slice, tagged with a marker comment.
pub fn insert_registry(registry_rs: &str, name: &str, struct_name: &str) -> Result<String> {
    let marker = format!("// xtask:new-emitter {name}");
    if registry_rs.contains(&marker) {
        return Ok(registry_rs.to_string());
    }
    check_not_mid_edit(registry_rs)?;
    let with_use = insert_use(registry_rs, name, struct_name)?;
    insert_in_slice(&with_use, struct_name, &marker)
}

fn check_not_mid_edit(text: &str) -> Result<()> {
    let open = text.matches('{').count();
    let close = text.matches('}').count();
    if open != close || !text.contains("fn generators()") {
        bail!(
            "apps/veld/src/registry.rs looks like it is being edited right now (unbalanced braces); retry in a moment"
        );
    }
    Ok(())
}

fn insert_use(text: &str, name: &str, struct_name: &str) -> Result<String> {
    let new = format!("use veld_emit_{name}::{struct_name};");
    if text.lines().any(|l| l.trim() == new) {
        return Ok(text.to_string());
    }
    let lines: Vec<&str> = text.lines().collect();
    let emit_uses: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.starts_with("use veld_emit_"))
        .map(|(i, _)| i)
        .collect();
    let at = match (
        emit_uses.iter().find(|i| lines[**i] > new.as_str()),
        emit_uses.last(),
    ) {
        (Some(i), _) => *i,
        (None, Some(last)) => last + 1,
        (None, None) => match lines.iter().rposition(|l| l.starts_with("use ")) {
            Some(i) => i + 1,
            None => bail!("registry.rs has no `use` lines to put the new one next to"),
        },
    };
    Ok(insert_line(&lines, at, &new, text))
}

/// Appends `&<Struct>, <marker>` to the slice returned by `generators()`. A one-line slice
/// becomes a multi-line one; an existing multi-line slice keeps its lines untouched.
fn insert_in_slice(text: &str, struct_name: &str, marker: &str) -> Result<String> {
    let Some(fn_at) = text.find("fn generators()") else {
        bail!("registry.rs has no `fn generators()`")
    };
    let Some(open) = text[fn_at..].find("&[").map(|i| fn_at + i + 2) else {
        bail!("`generators()` has no slice")
    };
    let Some(close) = text[open..].find(']').map(|i| open + i) else {
        bail!("the slice in `generators()` is not closed")
    };
    let body = &text[open..close];
    if body
        .split(',')
        .any(|i| i.trim() == format!("&{struct_name}"))
    {
        return Ok(text.to_string());
    }
    let new_line = format!("        &{struct_name}, {marker}\n    ");
    let rebuilt = if body.contains('\n') {
        let trimmed = body.trim_end();
        let last_code = trimmed
            .lines()
            .last()
            .and_then(|l| l.split("//").next())
            .unwrap_or_default()
            .trim_end();
        if last_code.is_empty() || last_code.ends_with(',') || last_code.ends_with('[') {
            format!("{trimmed}\n{new_line}")
        } else {
            // The last item has no trailing comma: add it before any trailing comment.
            let (head, last) = trimmed.rsplit_once('\n').unwrap_or(("", trimmed));
            let (code, comment) = last
                .split_once("//")
                .map_or((last, ""), |(c, rest)| (c, rest));
            let comment = if comment.is_empty() {
                String::new()
            } else {
                format!(" //{comment}")
            };
            format!("{head}\n{},{comment}\n{new_line}", code.trim_end())
        }
    } else {
        let items: Vec<&str> = body
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect();
        let lines: String = items.iter().map(|i| format!("        {i},\n")).collect();
        format!("\n{lines}{new_line}")
    };
    Ok(format!("{}{}{}", &text[..open], rebuilt, &text[close..]))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROOT: &str = "[workspace]\nmembers = [\n  \"crates/veld-ir\",\n  \"crates/emitters/typescript\",\n  \"crates/emitters/node\",\n  \"xtask\",\n  \"apps/veld\",\n]\n\n[workspace.package]\nversion = \"1\"\n\n[workspace.dependencies]\nveld-ir = { path = \"crates/veld-ir\" }\nveld-emit-node = { path = \"crates/emitters/node\" }\nserde = \"1\"\n";

    #[test]
    fn member_goes_after_the_last_emitter_and_is_idempotent() {
        let once = insert_member(ROOT, "crates/emitters/kotlin").unwrap();
        assert!(
            once.contains(
                "  \"crates/emitters/node\",\n  \"crates/emitters/kotlin\",\n  \"xtask\","
            ),
            "{once}"
        );
        assert_eq!(
            insert_member(&once, "crates/emitters/kotlin").unwrap(),
            once
        );
    }

    #[test]
    fn workspace_dep_goes_after_the_last_emitter_dep() {
        let once =
            insert_workspace_dep(ROOT, "veld-emit-kotlin", "crates/emitters/kotlin").unwrap();
        assert!(
            once.contains(
                "veld-emit-node = { path = \"crates/emitters/node\" }\nveld-emit-kotlin = "
            ),
            "{once}"
        );
        assert_eq!(
            insert_workspace_dep(&once, "veld-emit-kotlin", "x").unwrap(),
            once
        );
    }

    #[test]
    fn app_dep_is_added_once() {
        let app = "[package]\nname = \"veld\"\n\n[dependencies]\nveld-ir.workspace = true\nveld-emit-node.workspace = true\nclap.workspace = true\n\n[dev-dependencies]\ntempfile.workspace = true\n";
        let once = insert_app_dep(app, "veld-emit-kotlin").unwrap();
        assert!(
            once.contains(
                "veld-emit-node.workspace = true\nveld-emit-kotlin.workspace = true\nclap"
            ),
            "{once}"
        );
        assert_eq!(insert_app_dep(&once, "veld-emit-kotlin").unwrap(), once);
    }

    const REGISTRY: &str = "use veld_codegen::DynGenerator;\nuse veld_emit_node::Node;\nuse veld_emit_typescript::TypeScript;\n\npub fn generators() -> &'static [&'static dyn DynGenerator] {\n    &[&Node, &TypeScript]\n}\n";

    #[test]
    fn registry_gets_a_sorted_use_and_a_marked_slice_entry() {
        let once = insert_registry(REGISTRY, "kotlin", "Kotlin").unwrap();
        let expected_use = "use veld_codegen::DynGenerator;\nuse veld_emit_kotlin::Kotlin;\nuse veld_emit_node::Node;";
        assert!(once.contains(expected_use), "{once}");
        assert!(once.contains("        &Node,\n        &TypeScript,\n        &Kotlin, // xtask:new-emitter kotlin\n    ]"), "{once}");
        assert_eq!(insert_registry(&once, "kotlin", "Kotlin").unwrap(), once);
        let twice = insert_registry(&once, "swift", "Swift").unwrap();
        assert!(twice.contains("&Kotlin, // xtask:new-emitter kotlin\n        &Swift, // xtask:new-emitter swift\n    ]"), "{twice}");
        assert!(
            twice.contains("use veld_emit_swift::Swift;\nuse veld_emit_typescript"),
            "{twice}"
        );
    }

    #[test]
    fn registry_edit_is_refused_when_the_file_looks_half_written() {
        let broken = REGISTRY.replace("}\n", "");
        assert!(
            insert_registry(&broken, "kotlin", "Kotlin")
                .unwrap_err()
                .to_string()
                .contains("being edited")
        );
    }
}

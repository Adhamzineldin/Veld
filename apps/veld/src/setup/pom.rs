//! Maven: registers a generated source directory with `build-helper-maven-plugin`
//! (`add-source`), editing `pom.xml` in place.
//!
//! The edit is one `<execution>` with the id `veld-<target>`. Re-running finds that id: the same
//! source is a no-op, a different one is updated. A structure this editor does not understand
//! (a self-closing `<plugins/>`, a malformed file) is an [`XmlError`], and the caller prints
//! [`snippet`] instead.

use super::json_edit::Change;
use super::xml::{Elem, XmlError, line_indent, parse};

const PLUGIN: &str = "build-helper-maven-plugin";
/// Pinned so a build never floats to a newer plugin.
pub const PLUGIN_VERSION: &str = "3.6.0";

fn unsafe_edit<T>(msg: &str) -> Result<T, XmlError> {
    Err(XmlError(msg.to_string()))
}

/// Layout used while rendering: indentation step and newline.
struct Style<'a> {
    unit: &'a str,
    nl: &'a str,
}

fn execution(st: &Style<'_>, ind: &str, id: &str, source: &str) -> String {
    let (u, nl) = (st.unit, st.nl);
    let lines: [(usize, String); 12] = [
        (
            0,
            format!("<!-- veld setup: generated sources of `{id}` -->"),
        ),
        (0, "<execution>".to_string()),
        (1, format!("<id>{id}</id>")),
        (1, "<phase>generate-sources</phase>".to_string()),
        (1, "<goals>".to_string()),
        (2, "<goal>add-source</goal>".to_string()),
        (1, "</goals>".to_string()),
        (1, "<configuration>".to_string()),
        (2, "<sources>".to_string()),
        (3, format!("<source>{source}</source>")),
        (2, "</sources>".to_string()),
        (1, "</configuration>".to_string()),
    ];
    let mut out = String::new();
    for (depth, line) in lines {
        out.push_str(ind);
        out.push_str(&u.repeat(depth));
        out.push_str(&line);
        out.push_str(nl);
    }
    out.push_str(ind);
    out.push_str("</execution>");
    out
}

fn wrap(st: &Style<'_>, ind: &str, tag: &str, inner: &str) -> String {
    format!("{ind}<{tag}>{nl}{inner}{nl}{ind}</{tag}>", nl = st.nl)
}

fn plugin(st: &Style<'_>, ind: &str, id: &str, source: &str) -> String {
    let (u, nl) = (st.unit, st.nl);
    let child = format!("{ind}{u}");
    let exec = execution(st, &format!("{child}{u}"), id, source);
    let executions = wrap(st, &child, "executions", &exec);
    format!(
        "{ind}<plugin>{nl}{child}<groupId>org.codehaus.mojo</groupId>{nl}{child}<artifactId>{PLUGIN}</artifactId>{nl}{child}<version>{PLUGIN_VERSION}</version>{nl}{executions}{nl}{ind}</plugin>"
    )
}

/// The complete `<plugin>` block to paste into `<build><plugins>` by hand.
#[must_use]
pub fn snippet(id: &str, source: &str) -> String {
    let st = Style {
        unit: "  ",
        nl: "\n",
    };
    plugin(&st, "", id, source)
}

fn indent_of(src: &str, el: &Elem) -> Result<String, XmlError> {
    match line_indent(src, el.open_start) {
        Some(i) => Ok(i),
        None => unsafe_edit(&format!("`<{}>` does not start its own line", el.name)),
    }
}

/// Inserts `text` (already indented as a child) as the last child of `parent`.
fn insert_child(src: &str, parent: &Elem, text: &str, nl: &str) -> Result<String, XmlError> {
    if parent.content_start == parent.end {
        return unsafe_edit(&format!("`<{}/>` is self-closing", parent.name));
    }
    let mut out = src.to_string();
    if let Some(last) = parent.children.last() {
        out.insert_str(last.end, &format!("{nl}{text}"));
    } else if src[parent.content_start..parent.content_end]
        .trim()
        .is_empty()
    {
        let closing = indent_of(src, parent)?;
        out.replace_range(
            parent.content_start..parent.content_end,
            &format!("{nl}{text}{nl}{closing}"),
        );
    } else {
        return unsafe_edit(&format!("`<{}>` holds text", parent.name));
    }
    Ok(out)
}

/// Adds (or updates) the `add-source` execution `id` for `source`.
pub fn add_source(src: &str, id: &str, source: &str) -> Result<(String, Change), XmlError> {
    let root = parse(src)?;
    if root.name != "project" {
        return unsafe_edit("the root element is not `<project>`");
    }
    let nl = if src.contains("\r\n") { "\r\n" } else { "\n" };
    let unit = root
        .children
        .first()
        .and_then(|c| line_indent(src, c.open_start))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "    ".to_string());
    let st = Style { unit: &unit, nl };
    let child_ind =
        |p: &Elem| -> Result<String, XmlError> { Ok(format!("{}{unit}", indent_of(src, p)?)) };

    let Some(build) = root.child("build") else {
        let ci = unit.clone();
        let plugin_text = plugin(&st, &format!("{ci}{unit}{unit}"), id, source);
        let plugins = wrap(&st, &format!("{ci}{unit}"), "plugins", &plugin_text);
        return Ok((
            insert_child(src, &root, &wrap(&st, &ci, "build", &plugins), nl)?,
            Change::Added,
        ));
    };
    let Some(plugins) = build.child("plugins") else {
        let ci = child_ind(build)?;
        let text = wrap(
            &st,
            &ci,
            "plugins",
            &plugin(&st, &format!("{ci}{unit}"), id, source),
        );
        return Ok((insert_child(src, build, &text, nl)?, Change::Added));
    };
    let helper = plugins.children.iter().find(|p| {
        p.name == "plugin" && p.child("artifactId").is_some_and(|a| a.text(src) == PLUGIN)
    });
    let Some(helper) = helper else {
        let text = plugin(&st, &child_ind(plugins)?, id, source);
        return Ok((insert_child(src, plugins, &text, nl)?, Change::Added));
    };
    let Some(executions) = helper.child("executions") else {
        let ci = child_ind(helper)?;
        let text = wrap(
            &st,
            &ci,
            "executions",
            &execution(&st, &format!("{ci}{unit}"), id, source),
        );
        return Ok((insert_child(src, helper, &text, nl)?, Change::Added));
    };
    let ours = executions
        .children
        .iter()
        .find(|e| e.name == "execution" && e.child("id").is_some_and(|i| i.text(src) == id));
    match ours {
        None => {
            let text = execution(&st, &child_ind(executions)?, id, source);
            Ok((insert_child(src, executions, &text, nl)?, Change::Added))
        }
        Some(exec) => update_source(src, exec, source),
    }
}

fn update_source(src: &str, exec: &Elem, source: &str) -> Result<(String, Change), XmlError> {
    let target = exec
        .child("configuration")
        .and_then(|c| c.child("sources"))
        .filter(|s| s.children.len() == 1)
        .and_then(|s| s.child("source"))
        .filter(|s| !s.children.is_empty() || s.content_start != s.end);
    let Some(target) = target else {
        return unsafe_edit("the existing `veld-` execution has an unexpected shape");
    };
    let current = target.text(src);
    if current == source {
        return Ok((src.to_string(), Change::Unchanged));
    }
    let mut out = src.to_string();
    out.replace_range(target.content_start..target.content_end, source);
    Ok((
        out,
        Change::Updated {
            from: current.to_string(),
        },
    ))
}

/// Whether the pom already registers `source` under `id`.
#[must_use]
#[cfg(test)]
pub fn is_wired(src: &str, id: &str, source: &str) -> bool {
    matches!(add_source(src, id, source), Ok((_, Change::Unchanged)))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "veld-orders-api";
    const SRC: &str = "gen/java";

    const BASIC: &str = "<?xml version=\"1.0\"?>\n<project xmlns=\"http://maven.apache.org/POM/4.0.0\">\n    <modelVersion>4.0.0</modelVersion>\n    <artifactId>svc</artifactId>\n</project>\n";

    fn assert_well_formed(text: &str) {
        assert!(parse(text).is_ok(), "{text}");
    }

    #[test]
    fn adds_build_plugins_plugin_when_there_is_no_build() {
        let (out, change) = add_source(BASIC, ID, SRC).unwrap();
        assert_eq!(change, Change::Added);
        assert_well_formed(&out);
        let root = parse(&out).unwrap();
        let plugin = &root
            .child("build")
            .unwrap()
            .child("plugins")
            .unwrap()
            .children[0];
        assert_eq!(plugin.child("artifactId").unwrap().text(&out), PLUGIN);
        assert!(out.contains("<source>gen/java</source>"));
        assert!(out.contains("<id>veld-orders-api</id>"));
        assert!(out.contains("<!-- veld setup:"));
        // Every original line is still there, unchanged.
        for line in BASIC.lines() {
            assert!(out.contains(line), "{line}");
        }
        assert!(out.contains("    <build>\n        <plugins>"), "{out}");
    }

    #[test]
    fn is_idempotent() {
        let (once, _) = add_source(BASIC, ID, SRC).unwrap();
        let (twice, change) = add_source(&once, ID, SRC).unwrap();
        assert_eq!(change, Change::Unchanged);
        assert_eq!(once, twice);
        assert!(is_wired(&once, ID, SRC));
        assert!(!is_wired(BASIC, ID, SRC));
    }

    #[test]
    fn updates_the_source_of_an_existing_execution() {
        let (once, _) = add_source(BASIC, ID, SRC).unwrap();
        let (out, change) = add_source(&once, ID, "other/dir").unwrap();
        assert_eq!(change, Change::Updated { from: SRC.into() });
        assert!(out.contains("<source>other/dir</source>") && !out.contains("gen/java"));
    }

    #[test]
    fn two_targets_share_one_plugin() {
        let (a, _) = add_source(BASIC, "veld-a", "gen/a").unwrap();
        let (b, change) = add_source(&a, "veld-b", "gen/b").unwrap();
        assert_eq!(change, Change::Added);
        assert_well_formed(&b);
        assert_eq!(b.matches(PLUGIN).count(), 1, "{b}");
        assert_eq!(b.matches("<execution>").count(), 2);
    }

    #[test]
    fn extends_an_existing_build_and_other_plugins() {
        let pom = "<project>\n  <build>\n    <plugins>\n      <plugin>\n        <artifactId>maven-compiler-plugin</artifactId>\n      </plugin>\n    </plugins>\n  </build>\n</project>\n";
        let (out, _) = add_source(pom, ID, SRC).unwrap();
        assert_well_formed(&out);
        assert!(out.contains("<artifactId>maven-compiler-plugin</artifactId>"));
        let root = parse(&out).unwrap();
        let plugins = root.child("build").unwrap().child("plugins").unwrap();
        assert_eq!(plugins.children.len(), 2);
        // Two-space file: the new plugin uses the same indentation.
        assert!(
            out.contains("\n      <plugin>\n        <groupId>org.codehaus.mojo</groupId>"),
            "{out}"
        );
    }

    #[test]
    fn adds_an_execution_to_an_existing_build_helper() {
        let pom = "<project>\n  <build>\n    <plugins>\n      <plugin>\n        <groupId>org.codehaus.mojo</groupId>\n        <artifactId>build-helper-maven-plugin</artifactId>\n        <version>3.5.0</version>\n      </plugin>\n    </plugins>\n  </build>\n</project>\n";
        let (out, _) = add_source(pom, ID, SRC).unwrap();
        assert_well_formed(&out);
        assert_eq!(out.matches("build-helper-maven-plugin").count(), 1);
        assert!(out.contains("<version>3.5.0</version>"));
        assert!(out.contains("<executions>"));
        let (again, change) = add_source(&out, ID, SRC).unwrap();
        assert_eq!((again, change), (out, Change::Unchanged));
    }

    #[test]
    fn ignores_plugin_management_and_comments() {
        let pom = "<project>\n  <build>\n    <pluginManagement>\n      <plugins/>\n    </pluginManagement>\n    <!-- <plugins> -->\n  </build>\n</project>\n";
        let (out, _) = add_source(pom, ID, SRC).unwrap();
        assert_well_formed(&out);
        let build = parse(&out).unwrap();
        assert!(build.child("build").unwrap().child("plugins").is_some());
    }

    #[test]
    fn refuses_what_it_cannot_edit_safely() {
        assert!(add_source("<project><build><plugins/></build></project>", ID, SRC).is_err());
        assert!(add_source("<project>", ID, SRC).is_err());
        assert!(add_source("<other/>", ID, SRC).is_err());
    }

    #[test]
    fn the_snippet_is_a_complete_plugin() {
        let s = snippet(ID, SRC);
        assert!(s.starts_with("<plugin>") && s.ends_with("</plugin>"));
        assert!(parse(&s).is_ok());
        assert!(s.contains(PLUGIN_VERSION));
    }
}

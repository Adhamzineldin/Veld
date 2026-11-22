//! Gradle: registers a generated source directory in `sourceSets.main.java`, by appending a
//! marked block to `build.gradle` or `build.gradle.kts`.
//!
//! The block is delimited by `// veld:begin <id>` and `// veld:end <id>`, so a re-run finds and
//! replaces it. A build file that does not apply a JVM plugin is not edited (see [`safe`]).

use super::json_edit::Change;

/// The two lines of a block, for the Groovy or the Kotlin DSL.
fn block(id: &str, dir: &str, kts: bool, nl: &str) -> String {
    let call = if kts {
        format!("sourceSets[\"main\"].java.srcDir(\"{dir}\")")
    } else {
        format!("sourceSets.main.java.srcDir(\"{dir}\")")
    };
    format!("// veld:begin {id}{nl}{call}{nl}// veld:end {id}")
}

/// The block to paste by hand.
#[must_use]
pub fn snippet(id: &str, dir: &str, kts: bool) -> String {
    block(id, dir, kts, "\n")
}

/// Whether the file plausibly applies a JVM plugin, so `sourceSets.main` exists.
#[must_use]
pub fn safe(text: &str) -> bool {
    ["java", "application", "kotlin", "springframework"]
        .iter()
        .any(|w| text.contains(w))
}

/// Adds or updates the block for `id`.
#[must_use]
pub fn add_source(text: &str, id: &str, dir: &str, kts: bool) -> (String, Change) {
    let nl = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let want = block(id, dir, kts, nl);
    let begin = format!("// veld:begin {id}");
    let end = format!("// veld:end {id}");
    let span = text
        .find(&begin)
        .and_then(|b| text[b..].find(&end).map(|n| (b, b + n + end.len())));
    if let Some((b, e)) = span {
        let current = &text[b..e];
        if current == want {
            return (text.to_string(), Change::Unchanged);
        }
        let mut out = text.to_string();
        out.replace_range(b..e, &want);
        let from = current
            .lines()
            .nth(1)
            .unwrap_or_default()
            .trim()
            .to_string();
        return (out, Change::Updated { from });
    }
    let mut out = text.to_string();
    if !out.is_empty() && !out.ends_with('\n') {
        out.push_str(nl);
    }
    if !out.is_empty() {
        out.push_str(nl);
    }
    out.push_str(&want);
    out.push_str(nl);
    (out, Change::Added)
}

#[cfg(test)]
mod tests {
    use super::*;

    const BUILD: &str = "plugins {\n    id 'java'\n}\n\nrepositories {\n    mavenCentral()\n}\n";

    #[test]
    fn appends_a_marked_block_and_keeps_the_file() {
        let (out, change) = add_source(BUILD, "veld-api", "gen/java", false);
        assert_eq!(change, Change::Added);
        assert!(out.starts_with(BUILD));
        assert!(out.ends_with(
            "// veld:begin veld-api\nsourceSets.main.java.srcDir(\"gen/java\")\n// veld:end veld-api\n"
        ));
    }

    #[test]
    fn is_idempotent_and_updates_a_changed_dir() {
        let (once, _) = add_source(BUILD, "veld-api", "gen/java", false);
        let (twice, change) = add_source(&once, "veld-api", "gen/java", false);
        assert_eq!((twice.as_str(), change), (once.as_str(), Change::Unchanged));
        let (moved, change) = add_source(&once, "veld-api", "other", false);
        assert!(matches!(change, Change::Updated { .. }));
        assert!(moved.contains("srcDir(\"other\")") && !moved.contains("gen/java"));
        assert_eq!(moved.matches("veld:begin").count(), 1);
    }

    #[test]
    fn kotlin_dsl_uses_the_indexed_accessor() {
        let (out, _) = add_source("plugins { java }", "veld-api", "g", true);
        assert!(out.contains("sourceSets[\"main\"].java.srcDir(\"g\")"));
        assert!(out.starts_with("plugins { java }\n\n"), "{out:?}");
    }

    #[test]
    fn two_targets_get_two_blocks() {
        let (a, _) = add_source(BUILD, "veld-a", "a", false);
        let (b, change) = add_source(&a, "veld-b", "b", false);
        assert_eq!(change, Change::Added);
        assert_eq!(b.matches("veld:begin").count(), 2);
    }

    #[test]
    fn crlf_files_stay_crlf() {
        let (out, _) = add_source("plugins { id 'java' }\r\n", "v", "g", false);
        assert!(!out.replace("\r\n", "").contains('\n'), "{out:?}");
    }

    #[test]
    fn only_jvm_builds_are_safe_to_edit() {
        assert!(safe(BUILD));
        assert!(!safe("rootProject.name = 'x'\n"));
    }
}

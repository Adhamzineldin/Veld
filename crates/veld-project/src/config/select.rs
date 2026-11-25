//! Member selection and `--affected` (`spec/workspace.md`, sections 9 and 10). Pure: the caller
//! supplies the changed paths and the contract imports, so the rules are tested without git.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use super::error::ConfigError;
use super::{MemberKind, MemberSpec, Project, TargetSpec};

/// How a run asks for members.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectBy<'a> {
    /// The current member when the run started inside one, else every member.
    Default,
    /// `-p <name>` (repeatable).
    Packages(&'a [String]),
    /// `--workspace`.
    Workspace,
}

/// The members a run works on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    /// Names, in member order.
    pub members: Vec<String>,
    /// Whether every member of the workspace is selected (a full run).
    pub full: bool,
}

impl Selection {
    fn of(project: &Project, names: &BTreeSet<String>) -> Self {
        let members: Vec<String> = project
            .members
            .iter()
            .filter(|m| names.contains(&m.name))
            .map(|m| m.name.clone())
            .collect();
        Self {
            full: members.len() == project.members.len(),
            members,
        }
    }

    /// Every member of `project`.
    #[must_use]
    pub fn all(project: &Project) -> Self {
        Self {
            members: project.members.iter().map(|m| m.name.clone()).collect(),
            full: true,
        }
    }

    #[must_use]
    pub fn has(&self, member: &str) -> bool {
        self.members.iter().any(|m| m == member)
    }

    /// The targets of the selected members, in project order.
    #[must_use]
    pub fn targets<'a>(&self, project: &'a Project) -> Vec<&'a TargetSpec> {
        project
            .targets
            .iter()
            .filter(|t| self.has(&t.member))
            .collect()
    }
}

/// Resolves a selection request (section 9). An unknown `-p` name is `E0803` and lists the
/// members.
pub fn select(project: &Project, by: SelectBy<'_>) -> Result<Selection, ConfigError> {
    match by {
        SelectBy::Workspace => Ok(Selection::all(project)),
        SelectBy::Default => Ok(match &project.current {
            Some(name) => Selection::of(project, &BTreeSet::from([name.clone()])),
            None => Selection::all(project),
        }),
        SelectBy::Packages(names) => {
            let mut wanted = BTreeSet::new();
            for name in names {
                if project.member(name).is_none() {
                    return Err(ConfigError::UnknownMember {
                        name: name.clone(),
                        members: project
                            .members
                            .iter()
                            .map(|m| m.name.as_str())
                            .collect::<Vec<_>>()
                            .join(", "),
                    });
                }
                wanted.insert(name.clone());
            }
            Ok(Selection::of(project, &wanted))
        }
    }
}

/// One member that a change can have affected, with the reason it is listed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Affected {
    pub member: String,
    pub reason: String,
}

fn fold(p: &Path) -> PathBuf {
    super::resolve::fold(p)
}

fn under(path: &Path, dir: &Path) -> bool {
    fold(path).starts_with(fold(dir))
}

/// The reason a member is changed by itself, section 10.2, in the order of section 10.4.
fn own_reason(project: &Project, m: &MemberSpec, changed: &[PathBuf]) -> Option<&'static str> {
    let any = |f: &dyn Fn(&Path) -> bool| changed.iter().any(|p| f(p));
    let root_files = [project.file.clone(), project.root.join("veld.lock")];
    if any(&|p| root_files.iter().any(|r| fold(r) == fold(p))) {
        return Some("veld.toml or veld.lock of the workspace changed");
    }
    if any(&|p| fold(&m.file) == fold(p)) {
        return Some("veld.toml changed");
    }
    if let Some(contracts) = &m.contracts
        && any(&|p| under(p, contracts))
    {
        return Some("contracts changed");
    }
    let outs: Vec<&PathBuf> = project
        .targets
        .iter()
        .filter(|t| t.member == m.name)
        .map(|t| &t.out)
        .collect();
    if any(&|p| outs.iter().any(|o| under(p, o))) {
        return Some("output changed");
    }
    None
}

/// What a member depends on, with the verb that names the relation (section 10.3).
fn dependencies(
    m: &MemberSpec,
    imports: &BTreeMap<String, BTreeSet<String>>,
) -> Vec<(&'static str, String)> {
    let verb = if m.kind == MemberKind::App { "uses" } else { "consumes" };
    let mut out: Vec<(&'static str, String)> =
        m.depends_on.iter().map(|d| (verb, d.clone())).collect();
    for dep in imports.get(&m.name).into_iter().flatten() {
        if !out.iter().any(|(_, d)| d == dep) {
            out.push(("imports", dep.clone()));
        }
    }
    out
}

/// The members affected by a change (section 10.3): changed members, then every member that
/// depends on an affected one, until nothing is added. `changed` holds absolute paths;
/// `imports` maps a member to the members whose packages its contracts import. In member order.
#[must_use]
pub fn affected(
    project: &Project,
    changed: &[PathBuf],
    imports: &BTreeMap<String, BTreeSet<String>>,
) -> Vec<Affected> {
    let mut reasons: BTreeMap<String, String> = BTreeMap::new();
    for m in &project.members {
        if let Some(why) = own_reason(project, m, changed) {
            reasons.insert(m.name.clone(), why.to_string());
        }
    }
    loop {
        let mut added = Vec::new();
        for m in &project.members {
            if reasons.contains_key(&m.name) {
                continue;
            }
            let hit = dependencies(m, imports)
                .into_iter()
                .find(|(_, dep)| reasons.contains_key(dep));
            if let Some((verb, dep)) = hit {
                added.push((m.name.clone(), format!("{verb} {dep}")));
            }
        }
        if added.is_empty() {
            break;
        }
        reasons.extend(added);
    }
    project
        .members
        .iter()
        .filter_map(|m| {
            reasons.remove(&m.name).map(|reason| Affected {
                member: m.name.clone(),
                reason,
            })
        })
        .collect()
}

/// The selection that an `--affected` result stands for.
#[must_use]
pub fn selection_of(project: &Project, affected: &[Affected]) -> Selection {
    let names: BTreeSet<String> = affected.iter().map(|a| a.member.clone()).collect();
    Selection::of(project, &names)
}

/// The members that depend directly on `name` (consumes, uses or imports), in member order.
#[must_use]
pub fn dependents<'a>(
    project: &'a Project,
    name: &str,
    imports: &BTreeMap<String, BTreeSet<String>>,
) -> Vec<&'a MemberSpec> {
    project
        .members
        .iter()
        .filter(|m| m.name != name)
        .filter(|m| dependencies(m, imports).iter().any(|(_, d)| d == name))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::load_virtual;

    fn project() -> Project {
        let files = [
            ("veld.toml", "[workspace]\nname = \"s\"\nmembers = [\"services/*\", \"apps/*\", \"libs/*\"]\n"),
            ("services/payments/veld.toml", "[service]\n[server]\nemitter = \"node\"\n"),
            ("services/bookings/veld.toml", "[service]\nconsumes = [\"payments\"]\n"),
            ("services/audit/veld.toml", "[service]\n"),
            ("apps/web/veld.toml", "[app]\nuses = [\"bookings\"]\n"),
            ("libs/money/veld.toml", "[library]\n"),
        ];
        let files: Vec<(String, String)> = files
            .iter()
            .map(|(a, b)| ((*a).to_string(), (*b).to_string()))
            .collect();
        load_virtual(&std::env::temp_dir().join("veld-select"), &files).unwrap()
    }

    fn path(p: &Project, rel: &str) -> PathBuf {
        p.root.join(rel)
    }

    fn imports() -> BTreeMap<String, BTreeSet<String>> {
        BTreeMap::from([("payments".to_string(), BTreeSet::from(["money".to_string()]))])
    }

    fn reasons(p: &Project, changed: &[&str]) -> Vec<(String, String)> {
        let changed: Vec<PathBuf> = changed.iter().map(|c| path(p, c)).collect();
        affected(p, &changed, &imports())
            .into_iter()
            .map(|a| (a.member, a.reason))
            .collect()
    }

    #[test]
    fn selection_defaults_to_the_current_member_else_everything() {
        let mut p = project();
        assert!(select(&p, SelectBy::Default).unwrap().full);
        p.current = Some("bookings".into());
        let s = select(&p, SelectBy::Default).unwrap();
        assert_eq!(s.members, ["bookings"]);
        assert!(!s.full);
        assert!(select(&p, SelectBy::Workspace).unwrap().full);
        let names = vec!["web".to_string(), "audit".to_string()];
        let s = select(&p, SelectBy::Packages(&names)).unwrap();
        assert_eq!(s.members, ["web", "audit"]);
    }

    #[test]
    fn an_unknown_package_is_e0803_and_lists_the_members() {
        let p = project();
        let names = vec!["nope".to_string()];
        let err = select(&p, SelectBy::Packages(&names)).unwrap_err();
        assert_eq!(err.code(), Some("E0803"));
        assert!(err.to_string().contains("audit") && err.to_string().contains("money"), "{err}");
    }

    #[test]
    fn targets_follow_the_selection() {
        let p = project();
        let all = Selection::all(&p);
        assert_eq!(all.targets(&p).len(), 1);
        let names = vec!["audit".to_string()];
        let s = select(&p, SelectBy::Packages(&names)).unwrap();
        assert!(s.targets(&p).is_empty());
    }

    #[test]
    fn a_contract_change_reaches_every_dependent() {
        let p = project();
        let got = reasons(&p, &["services/payments/contracts/a.veld"]);
        assert_eq!(
            got,
            [
                ("web".to_string(), "uses bookings".to_string()),
                ("bookings".to_string(), "consumes payments".to_string()),
                ("payments".to_string(), "contracts changed".to_string()),
            ]
        );
    }

    #[test]
    fn imports_count_as_dependencies() {
        let p = project();
        let got = reasons(&p, &["libs/money/contracts/m.veld"]);
        let names: Vec<_> = got.iter().map(|(m, _)| m.as_str()).collect();
        assert_eq!(names, ["web", "money", "bookings", "payments"]);
        assert_eq!(got[3].1, "imports money");
    }

    #[test]
    fn the_root_file_or_the_lock_changes_everything() {
        let p = project();
        for file in ["veld.toml", "veld.lock"] {
            assert_eq!(reasons(&p, &[file]).len(), p.members.len(), "{file}");
        }
    }

    #[test]
    fn own_file_and_output_are_changes_and_unrelated_paths_are_not() {
        let p = project();
        assert_eq!(
            reasons(&p, &["services/audit/veld.toml"]),
            [("audit".to_string(), "veld.toml changed".to_string())]
        );
        let got = reasons(&p, &["services/audit/README.md", "docs/x.md"]);
        assert!(got.is_empty(), "{got:?}");
        let got = reasons(&p, &["services/payments/src/generated/a.ts"]);
        assert_eq!(got.last().unwrap().1, "output changed");
    }

    #[test]
    fn cycles_end_the_walk() {
        let mut p = project();
        let at = |p: &mut Project, n: &str| p.members.iter().position(|m| m.name == n).unwrap();
        let (b, y) = (at(&mut p, "bookings"), at(&mut p, "payments"));
        p.members[b].depends_on = vec!["payments".into()];
        p.members[y].depends_on = vec!["bookings".into()];
        let got = reasons(&p, &["services/payments/contracts/a.veld"]);
        assert!(got.iter().any(|(m, _)| m == "bookings"));
    }

    #[test]
    fn dependents_are_direct() {
        let p = project();
        let names: Vec<_> = dependents(&p, "payments", &imports())
            .iter()
            .map(|m| m.name.clone())
            .collect();
        assert_eq!(names, ["bookings"]);
        let names: Vec<_> = dependents(&p, "money", &imports())
            .iter()
            .map(|m| m.name.clone())
            .collect();
        assert_eq!(names, ["payments"]);
    }
}

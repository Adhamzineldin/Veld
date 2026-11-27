//! Platform phase P4 through the real binary (`spec/platform.md` sections 22 to 24 and
//! `spec/rules.md` section 19.3): message catalogs, retention, `@pii` and `@secret`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use std::fs;

use common::*;
use predicates::prelude::*;
use tempfile::tempdir;

const PERSONAL: &str = r#"
@retain(days(30), since: seenAt, then: anonymize)
model ErasableGuest {
  id: uuid @readonly
  name: string @pii
  password: string @secret
  seenAt?: datetime
}

model OpenGuest {
  nick: string @pii
}

@auth(bearer)
service P4Guests at "/p4" {
  get "/{id}" getErasable(id: uuid) -> ErasableGuest | NotFound
  get "/open" getOpen() -> OpenGuest
}
"#;

fn project(contract: &str) -> tempfile::TempDir {
    let tmp = tempdir().unwrap();
    veld(tmp.path())
        .args(["init", "--name", "demo"])
        .assert()
        .success();
    write(tmp.path(), "contracts/p4.veld", contract);
    tmp
}

// ---- veld export retention -----------------------------------------------------------------

#[test]
fn export_retention_prints_the_inventory() {
    let tmp = project(PERSONAL);
    veld(tmp.path())
        .args(["export", "retention"])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("# Data inventory"))
        .stdout(predicate::str::contains("### `ErasableGuest`"))
        .stdout(predicate::str::contains(
            "- Kept: 30 days after `seenAt`, then anonymized (`name`)",
        ))
        .stdout(predicate::str::contains("- Kept: kept without limit"))
        .stdout(predicate::str::contains(
            "`GET /p4/{id}` (`P4Guests.getErasable`)",
        ));
}

#[test]
fn export_retention_writes_the_file_the_user_owns() {
    let tmp = project(PERSONAL);
    veld(tmp.path())
        .args(["export", "retention", "-o", "docs/inventory.md"])
        .assert()
        .success();
    let first = fs::read_to_string(tmp.path().join("docs/inventory.md")).unwrap();
    assert!(first.contains("### `OpenGuest`"));
    veld(tmp.path())
        .args(["export", "retention", "-o", "docs/inventory.md"])
        .assert()
        .success();
    let again = fs::read_to_string(tmp.path().join("docs/inventory.md")).unwrap();
    assert_eq!(first, again);
}

// ---- the lints of @pii, @secret and @retain ------------------------------------------------

#[test]
fn check_warns_about_pii_without_retention_and_secrets_in_output() {
    let tmp = project(PERSONAL);
    veld(tmp.path())
        .arg("check")
        .assert()
        .success()
        .stderr(predicate::str::contains("L0017"))
        .stderr(predicate::str::contains("OpenGuest"))
        .stderr(predicate::str::contains("L0016"))
        .stderr(predicate::str::contains("ErasableGuest.password"));
}

#[test]
fn lint_levels_apply_to_the_new_rules() {
    let tmp = project(PERSONAL);
    let toml = fs::read_to_string(tmp.path().join("veld.toml")).unwrap();
    write(
        tmp.path(),
        "veld.toml",
        &format!("{toml}\n[lint]\nsecret-in-output = \"error\"\npii-without-retention = \"off\"\n"),
    );
    veld(tmp.path())
        .arg("check")
        .assert()
        .failure()
        .stderr(predicate::str::contains("L0016"))
        .stderr(predicate::str::contains("L0017").not());
}

#[test]
fn a_wrong_retain_is_e1310() {
    let tmp = project(
        "@retain(days(0), since: nope, then: delete)\nmodel P4Bad {\n  seen: datetime\n}\n",
    );
    veld(tmp.path())
        .arg("check")
        .assert()
        .failure()
        .stderr(predicate::str::contains("E1310"));
}

// ---- message catalogs ----------------------------------------------------------------------

const RULES: &str = r#"
model P4Stay {
  nights: int
  rule atLeastOne: nights >= 1
    message "A stay is at least one night"
}

@auth(none)
service P4Stays at "/p4stays" {
  post "/" book(body: P4Stay) -> 201 P4Stay
}
"#;

fn with_catalog(file: &str, text: &str) -> tempfile::TempDir {
    let tmp = project(RULES);
    write(tmp.path(), &format!("messages/{file}"), text);
    tmp
}

#[test]
fn a_catalog_with_a_key_no_rule_has_is_w1301_and_does_not_stop_check() {
    let tmp = with_catalog(
        "ar.toml",
        "\"demo/P4Stay.atLeastOne\" = \"x\"\n\"demo/P4Stay.nope\" = \"y\"\n",
    );
    veld(tmp.path())
        .arg("check")
        .assert()
        .success()
        .stderr(predicate::str::contains("W1301"))
        .stderr(predicate::str::contains("ar.toml"));
}

#[test]
fn a_catalog_that_lacks_a_message_is_w1302() {
    let tmp = with_catalog("ar.toml", "\"veld/not-permitted\" = \"x\"\n");
    veld(tmp.path())
        .arg("check")
        .assert()
        .success()
        .stderr(predicate::str::contains("W1302"));
}

#[test]
fn a_catalog_that_is_not_a_flat_table_is_e1301() {
    let tmp = with_catalog("ar.toml", "[nested]\nk = \"v\"\n");
    veld(tmp.path())
        .arg("check")
        .assert()
        .failure()
        .stderr(predicate::str::contains("E1301"));
    let tmp = with_catalog("AR.toml", "\"k\" = \"v\"\n");
    veld(tmp.path())
        .arg("check")
        .assert()
        .failure()
        .stderr(predicate::str::contains("E1301"));
}

#[test]
fn a_named_messages_directory_that_is_missing_is_an_error() {
    let tmp = project(RULES);
    let toml = fs::read_to_string(tmp.path().join("veld.toml")).unwrap();
    write(
        tmp.path(),
        "veld.toml",
        &toml.replacen("[service]", "[service]\nmessages = \"texts\"", 1),
    );
    veld(tmp.path())
        .arg("check")
        .assert()
        .failure()
        .stderr(predicate::str::contains("texts"));
}

/// The default targets (`node` and `typescript`) implement catalogs (`spec/targets/typescript.md`
/// section 5l): the member's catalogs are written as data next to the generated code.
#[test]
fn the_default_targets_generate_the_catalogs() {
    let tmp = with_catalog("ar.toml", "\"demo/P4Stay.atLeastOne\" = \"x\"\n");
    veld(tmp.path()).arg("generate").assert().success();
    let server = fs::read_to_string(tmp.path().join("generated/server/messages.ts")).unwrap();
    assert!(server.contains("'demo/P4Stay.atLeastOne': 'x'"), "{server}");
    let client = fs::read_to_string(tmp.path().join("generated/client/demo/messages.ts")).unwrap();
    assert!(client.contains("'demo/P4Stay.atLeastOne': 'x'"), "{client}");
}

/// The default server (`node`) implements retention (`spec/targets/typescript.md` section 5l): the
/// plan and the runner are written next to the generated code, and the client has none of it.
#[test]
fn the_default_server_generates_retention() {
    let tmp = project(PERSONAL);
    veld(tmp.path()).arg("generate").assert().success();
    let file = fs::read_to_string(tmp.path().join("generated/server/retention.ts")).unwrap();
    assert!(file.contains("export const retentionPlan"), "{file}");
    assert!(
        file.contains("export async function runApiRetention("),
        "{file}"
    );
    assert!(file.contains("fields: ['name'],"), "{file}");
    assert!(!tmp.path().join("generated/client/retention.ts").exists());
}

#[test]
fn a_project_that_uses_none_of_it_still_checks_without_a_warning() {
    let tmp = tempdir().unwrap();
    veld(tmp.path())
        .args(["init", "--name", "demo"])
        .assert()
        .success();
    veld(tmp.path())
        .arg("check")
        .assert()
        .success()
        .stderr(predicate::str::contains("warning").not());
}

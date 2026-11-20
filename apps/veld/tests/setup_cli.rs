//! `veld init` (flag paths) and `veld setup`: wiring generated packages into apps.
//!
//! The npm round trip (`tsc` resolving `@veld/api` after `init`, `generate`, `setup`) needs node,
//! npm and network access; without them it is skipped, unless `VELD_REQUIRE_TOOLCHAINS=1`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};

use common::*;
use predicates::prelude::*;
use serde_json::Value;
use tempfile::tempdir;

fn read(dir: &Path, rel: &str) -> String {
    fs::read_to_string(dir.join(rel)).unwrap()
}

fn stdout(cmd: &mut assert_cmd::Command) -> String {
    let out = cmd.assert().success().get_output().stdout.clone();
    String::from_utf8(out).unwrap()
}

// ---------------------------------------------------------------------------
// init: flags
// ---------------------------------------------------------------------------

#[test]
fn init_flags_describe_a_workspace_without_a_tty() {
    let tmp = tempdir().unwrap();
    veld(tmp.path())
        .args([
            "init",
            "--name",
            "shop",
            "--workspace",
            "users,billing",
            "--server",
            "node:fastify",
            "--server",
            "billing=java:spring",
            "--client",
            "typescript:react",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("veld setup"));
    let toml = read(tmp.path(), "veld.toml");
    assert!(toml.contains("framework = \"fastify\"") && toml.contains("framework = \"spring\""));
    assert!(toml.contains("framework = \"react\"") && toml.contains("consumes = [\"users\"]"));
    assert!(tmp.path().join("contracts/billing/billing.veld").is_file());
    assert!(tmp.path().join("services/users/package.json").is_file());
    assert!(tmp.path().join("web/package.json").is_file());
    veld(tmp.path())
        .arg("check")
        .assert()
        .success()
        .stdout(predicate::str::contains("2 services"));
    veld(tmp.path()).arg("generate").assert().success();
    assert!(
        tmp.path()
            .join("services/users/generated/package.json")
            .is_file()
    );
    assert!(tmp.path().join("web/generated/package.json").is_file());
    assert!(tmp.path().join("services/billing/src/main/java").is_dir());
}

#[test]
fn init_yes_and_defaults_match_and_bad_values_are_errors() {
    let (a, b) = (tempdir().unwrap(), tempdir().unwrap());
    veld(a.path())
        .args(["init", "--name", "demo", "--yes"])
        .assert()
        .success();
    veld(b.path())
        .args(["init", "--name", "demo"])
        .assert()
        .success();
    assert_eq!(read(a.path(), "veld.toml"), read(b.path(), "veld.toml"));

    let tmp = tempdir().unwrap();
    for bad in [
        ["--server", "cobol"],
        ["--client", "typescript:svelte"],
        ["--server", "node:koa"],
    ] {
        veld(tmp.path()).arg("init").args(bad).assert().code(1);
        assert!(!tmp.path().join("veld.toml").exists(), "{bad:?}");
    }
    veld(tmp.path())
        .args(["init", "--workspace", "a,a"])
        .assert()
        .code(1);
    veld(tmp.path())
        .args(["init", "--server", "none", "--client", "none"])
        .assert()
        .success();
    assert!(!read(tmp.path(), "veld.toml").contains("[[targets]]"));
}

// ---------------------------------------------------------------------------
// setup: npm
// ---------------------------------------------------------------------------

fn init_single(dir: &Path) {
    veld(dir)
        .args(["init", "--name", "demo", "--yes"])
        .assert()
        .success();
    veld(dir).arg("generate").assert().success();
}

#[test]
fn generate_hints_at_setup_until_it_is_done() {
    let tmp = tempdir().unwrap();
    veld(tmp.path())
        .args(["init", "--name", "demo"])
        .assert()
        .success();
    veld(tmp.path())
        .arg("generate")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "hint: 2 target(s) are not wired into your app yet; run `veld setup`",
        ));
    veld(tmp.path())
        .arg("setup")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "dependencies[\"@veld/api\"] = \"file:generated/server\"",
        ))
        .stdout(predicate::str::contains("npm install"));
    veld(tmp.path())
        .arg("generate")
        .assert()
        .success()
        .stdout(predicate::str::contains("hint:").not());
}

#[test]
fn setup_edits_package_json_in_place_and_is_idempotent() {
    let tmp = tempdir().unwrap();
    init_single(tmp.path());
    let original = "{\n    \"name\": \"mine\",\n    \"scripts\": { \"start\": \"node x.js\" },\n    \"dependencies\": {\n        \"express\": \"^4.0.0\"\n    }\n}\n";
    write(tmp.path(), "package.json", original);

    let dry = stdout(veld(tmp.path()).args(["setup", "--dry-run"]));
    assert!(
        dry.contains("would change package.json") && dry.contains("dry run"),
        "{dry}"
    );
    assert_eq!(
        read(tmp.path(), "package.json"),
        original,
        "a dry run writes nothing"
    );

    veld(tmp.path()).arg("setup").assert().success();
    let after = read(tmp.path(), "package.json");
    assert_eq!(
        after,
        "{\n    \"name\": \"mine\",\n    \"scripts\": { \"start\": \"node x.js\" },\n    \"dependencies\": {\n        \"@veld/api\": \"file:generated/server\",\n        \"@veld/web\": \"file:generated/client\",\n        \"express\": \"^4.0.0\"\n    }\n}\n"
    );
    veld(tmp.path())
        .arg("setup")
        .assert()
        .success()
        .stdout(predicate::str::contains("already wired"));
    assert_eq!(read(tmp.path(), "package.json"), after);

    // One target by name; an unknown one is an error.
    veld(tmp.path()).args(["setup", "nope"]).assert().code(1);
}

#[test]
fn the_app_key_picks_the_package_json() {
    let tmp = tempdir().unwrap();
    init_single(tmp.path());
    fs::remove_file(tmp.path().join("package.json")).unwrap();
    write(tmp.path(), "apps/site/package.json", "{}\n");
    let toml = read(tmp.path(), "veld.toml").replace(
        "out = \"generated/client\"",
        "out = \"generated/client\"\napp = \"apps/site\"",
    );
    write(tmp.path(), "veld.toml", &toml);
    let out = stdout(veld(tmp.path()).arg("setup"));
    assert!(
        out.contains("no package.json found"),
        "api has no app: {out}"
    );
    let pkg: Value = serde_json::from_str(&read(tmp.path(), "apps/site/package.json")).unwrap();
    assert_eq!(
        pkg["dependencies"]["@veld/web"],
        "file:../../generated/client"
    );
}

// ---------------------------------------------------------------------------
// setup: Maven and Gradle
// ---------------------------------------------------------------------------

const JAVA_TOML: &str = "[project]\nname = \"svc\"\ncontracts = \"contracts\"\n\n[[targets]]\nname = \"api\"\nrole = \"server\"\nemitter = \"java\"\nservice = \"svc\"\nout = \"gen/java\"\n\n[targets.options]\npackage = \"com.acme.svc\"\n";
const CONTRACT: &str =
    "model Ping {\n  ok: bool\n}\n\nservice Svc at \"/svc\" {\n  get \"/ping\" ping() -> Ping\n}\n";

fn java_project(dir: &Path) {
    write(dir, "veld.toml", JAVA_TOML);
    write(dir, "contracts/svc.veld", CONTRACT);
}

#[test]
fn maven_gets_one_add_source_execution() {
    let tmp = tempdir().unwrap();
    java_project(tmp.path());
    let pom = "<?xml version=\"1.0\"?>\n<project>\n  <modelVersion>4.0.0</modelVersion>\n  <build>\n    <plugins>\n      <plugin>\n        <artifactId>maven-compiler-plugin</artifactId>\n      </plugin>\n    </plugins>\n  </build>\n</project>\n";
    write(tmp.path(), "pom.xml", pom);

    let dry = stdout(veld(tmp.path()).args(["setup", "--dry-run"]));
    assert!(dry.contains("would change pom.xml"), "{dry}");
    assert_eq!(read(tmp.path(), "pom.xml"), pom);

    veld(tmp.path()).arg("setup").assert().success();
    let edited = read(tmp.path(), "pom.xml");
    assert!(edited.starts_with("<?xml version=\"1.0\"?>\n<project>"));
    assert!(edited.contains("<artifactId>maven-compiler-plugin</artifactId>"));
    for needle in [
        "<artifactId>build-helper-maven-plugin</artifactId>",
        "<id>veld-api</id>",
        "<goal>add-source</goal>",
        "<phase>generate-sources</phase>",
        "<source>gen/java</source>",
        "<!-- veld setup:",
    ] {
        assert_eq!(edited.matches(needle).count(), 1, "{needle} in\n{edited}");
    }
    assert_eq!(edited.matches("<plugin>").count(), 2);
    veld(tmp.path()).arg("setup").assert().success();
    assert_eq!(read(tmp.path(), "pom.xml"), edited, "idempotent");
}

#[test]
fn maven_with_a_standard_source_root_needs_nothing() {
    let tmp = tempdir().unwrap();
    java_project(tmp.path());
    write(
        tmp.path(),
        "veld.toml",
        &JAVA_TOML.replace("gen/java", "src/main/java"),
    );
    write(tmp.path(), "pom.xml", "<project/>\n");
    veld(tmp.path())
        .arg("setup")
        .assert()
        .success()
        .stdout(predicate::str::contains("standard source root"));
    assert_eq!(read(tmp.path(), "pom.xml"), "<project/>\n");
}

#[test]
fn an_unsafe_pom_gets_a_snippet_and_is_left_alone() {
    let tmp = tempdir().unwrap();
    java_project(tmp.path());
    write(
        tmp.path(),
        "pom.xml",
        "<project><build><plugins/></build></project>\n",
    );
    veld(tmp.path())
        .arg("setup")
        .assert()
        .success()
        .stderr(predicate::str::contains("cannot edit `pom.xml` safely"))
        .stdout(predicate::str::contains(
            "<artifactId>build-helper-maven-plugin</artifactId>",
        ));
    assert_eq!(
        read(tmp.path(), "pom.xml"),
        "<project><build><plugins/></build></project>\n"
    );
}

#[test]
fn gradle_gets_a_marked_source_set_line() {
    let tmp = tempdir().unwrap();
    java_project(tmp.path());
    write(tmp.path(), "build.gradle", "plugins {\n    id 'java'\n}\n");
    veld(tmp.path()).arg("setup").assert().success();
    let edited = read(tmp.path(), "build.gradle");
    assert_eq!(
        edited,
        "plugins {\n    id 'java'\n}\n\n// veld:begin veld-api\nsourceSets.main.java.srcDir(\"gen/java\")\n// veld:end veld-api\n"
    );
    veld(tmp.path()).arg("setup").assert().success();
    assert_eq!(read(tmp.path(), "build.gradle"), edited);

    write(tmp.path(), "build.gradle", "rootProject.name = 'x'\n");
    veld(tmp.path())
        .arg("setup")
        .assert()
        .success()
        .stdout(predicate::str::contains("sourceSets.main.java.srcDir"));
    assert_eq!(read(tmp.path(), "build.gradle"), "rootProject.name = 'x'\n");
}

// ---------------------------------------------------------------------------
// setup: tsc resolves @veld/api
// ---------------------------------------------------------------------------

fn npm_cli() -> Option<std::path::PathBuf> {
    let probe = Command::new("node")
        .args(["-p", "process.execPath"])
        .output()
        .ok()?;
    let exe = std::path::PathBuf::from(String::from_utf8_lossy(&probe.stdout).trim());
    let dir = exe.parent()?.to_path_buf();
    [
        dir.join("node_modules/npm/bin/npm-cli.js"),
        dir.join("../lib/node_modules/npm/bin/npm-cli.js"),
    ]
    .into_iter()
    .find(|p| p.is_file())
}

fn run_ok(cmd: &mut Command, what: &str) {
    let out = cmd
        .stdin(Stdio::null())
        .output()
        .unwrap_or_else(|e| panic!("{what}: {e}"));
    assert!(
        out.status.success(),
        "{what} failed\n{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn init_generate_setup_then_tsc_resolves_the_generated_packages() {
    let Some(cli) = npm_cli() else {
        assert!(
            std::env::var("VELD_REQUIRE_TOOLCHAINS").as_deref() != Ok("1"),
            "toolchain required but node/npm are unavailable"
        );
        eprintln!("SKIPPED: node/npm not found");
        return;
    };
    let tmp = tempdir().unwrap();
    let dir = tmp.path();
    init_single(dir);
    veld(dir).arg("setup").assert().success();
    // Add the toolchain next to the wired dependencies (pinned like tests/toolchain/ts).
    let mut pkg: Value = serde_json::from_str(&read(dir, "package.json")).unwrap();
    pkg["devDependencies"] = serde_json::json!({ "typescript": "5.9.3", "@types/node": "22.20.5" });
    write(
        dir,
        "package.json",
        &serde_json::to_string_pretty(&pkg).unwrap(),
    );
    write(
        dir,
        "tsconfig.json",
        "{\"compilerOptions\":{\"strict\":true,\"noEmit\":true,\"target\":\"ES2022\",\"module\":\"ESNext\",\"moduleResolution\":\"bundler\",\"types\":[\"node\"],\"skipLibCheck\":true},\"include\":[\"src\"]}",
    );
    write(
        dir,
        "src/main.ts",
        "import { createHelloHandler, toNodeHttp } from '@veld/api';\nimport type { HelloService } from '@veld/api';\nimport { HelloClient } from '@veld/web';\n\nconst impl: HelloService = { async greet({ name }) { return { message: `hi ${name}` }; } };\nexport const listener = toNodeHttp(createHelloHandler(impl));\nexport const client = new HelloClient({ baseUrl: 'http://localhost' });\n",
    );
    run_ok(
        Command::new("node")
            .arg(&cli)
            .args(["install", "--no-audit", "--no-fund"])
            .current_dir(dir),
        "npm install",
    );
    assert!(
        dir.join("node_modules/@veld/api/index.ts").is_file(),
        "the package is linked"
    );
    run_ok(
        Command::new("node")
            .args(["node_modules/typescript/bin/tsc", "--noEmit"])
            .current_dir(dir),
        "tsc --noEmit",
    );
    // A path that is not a generated package does not resolve: the check above is real.
    write(
        dir,
        "src/bad.ts",
        "import { nope } from '@veld/missing';\nexport { nope };\n",
    );
    let bad = Command::new("node")
        .args(["node_modules/typescript/bin/tsc", "--noEmit"])
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(!bad.status.success());
}

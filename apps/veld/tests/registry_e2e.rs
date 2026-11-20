//! End to end against the REAL Go registry: a throwaway PostgreSQL cluster, the registry built
//! from `registry/`, and the real `veld` binary doing login, publish, add, check, generate,
//! and the tamper checks.
//!
//! Needs `go` and the PostgreSQL server binaries (`VELD_PG_BIN`, else the default install path).
//! Without them the test is skipped with a message, unless `VELD_REQUIRE_TOOLCHAINS=1`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use std::fs;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use common::*;
use predicates::prelude::*;
use serde_json::{Value, json};
use tempfile::tempdir;

const EXE: &str = std::env::consts::EXE_SUFFIX;

fn require() -> bool {
    std::env::var("VELD_REQUIRE_TOOLCHAINS").is_ok_and(|v| v == "1")
}

fn pg_bin() -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(p) = std::env::var_os("VELD_PG_BIN").filter(|v| !v.is_empty()) {
        candidates.push(PathBuf::from(p));
    }
    candidates.push(PathBuf::from(r"C:\Program Files\PostgreSQL\17\bin"));
    if let Ok(dirs) = fs::read_dir("/usr/lib/postgresql") {
        candidates.extend(dirs.flatten().map(|d| d.path().join("bin")));
    }
    candidates.into_iter().find(|d| {
        d.join(format!("initdb{EXE}")).is_file() && d.join(format!("pg_ctl{EXE}")).is_file()
    })
}

fn have_go() -> bool {
    Command::new("go")
        .arg("version")
        .output()
        .is_ok_and(|o| o.status.success())
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

/// Everything that must be torn down, whatever happens in the test.
struct Env {
    pg_bin: PathBuf,
    pg_data: PathBuf,
    pg_started: bool,
    registry: Option<Child>,
}

impl Drop for Env {
    fn drop(&mut self) {
        if let Some(mut child) = self.registry.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        if self.pg_started {
            let _ = Command::new(self.pg_bin.join(format!("pg_ctl{EXE}")))
                .args(["stop", "-m", "immediate", "-D"])
                .arg(&self.pg_data)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
    }
}

fn run_ok(cmd: &mut Command, what: &str) {
    let out = cmd.output().unwrap_or_else(|e| panic!("{what}: {e}"));
    assert!(
        out.status.success(),
        "{what} failed\n{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

fn start_postgres(env: &mut Env, work: &Path, port: u16) {
    let initdb = env.pg_bin.join(format!("initdb{EXE}"));
    run_ok(
        Command::new(initdb)
            .args(["-U", "veld", "--auth=trust", "-E", "UTF8", "-D"])
            .arg(&env.pg_data)
            .stdin(Stdio::null()),
        "initdb",
    );
    let mut opts = format!("-p {port} -c listen_addresses=127.0.0.1 -c fsync=off");
    if !cfg!(windows) {
        opts.push_str(&format!(" -c unix_socket_directories={}", work.display()));
    }
    // pg_ctl must not inherit our pipes, or it would never return.
    let status = Command::new(env.pg_bin.join(format!("pg_ctl{EXE}")))
        .args(["start", "-w", "-t", "60", "-D"])
        .arg(&env.pg_data)
        .arg("-l")
        .arg(work.join("postgres.log"))
        .arg("-o")
        .arg(opts)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap();
    assert!(
        status.success(),
        "pg_ctl start failed:\n{}",
        fs::read_to_string(work.join("postgres.log")).unwrap_or_default()
    );
    env.pg_started = true;
    run_ok(
        Command::new(env.pg_bin.join(format!("createdb{EXE}")))
            .args([
                "-h",
                "127.0.0.1",
                "-p",
                &port.to_string(),
                "-U",
                "veld",
                "veldtest",
            ])
            .stdin(Stdio::null()),
        "createdb",
    );
}

fn api(method: &str, url: &str, bearer: Option<&str>, body: Option<Value>) -> Value {
    let mut req = ureq::request(method, url);
    if let Some(t) = bearer {
        req = req.set("Authorization", &format!("Bearer {t}"));
    }
    let resp = match body {
        Some(b) => req
            .set("Content-Type", "application/json")
            .send_bytes(b.to_string().as_bytes()),
        None => req.call(),
    };
    match resp {
        Ok(r) => serde_json::from_reader(r.into_reader()).unwrap_or(Value::Null),
        Err(e) => panic!("{method} {url}: {e}"),
    }
}

fn wait_for_registry(base: &str, child: &mut Child) {
    let deadline = Instant::now() + Duration::from_secs(60);
    while Instant::now() < deadline {
        if let Some(status) = child.try_wait().unwrap() {
            panic!("the registry exited early: {status}");
        }
        if ureq::get(&format!("{base}/api/v1/stats")).call().is_ok() {
            return;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    panic!("the registry did not become ready");
}

const INVOICES: &str =
    "/// An invoice from the billing package.\nmodel Invoice {\n  id: uuid\n  total: decimal\n}\n";

const ORDERS: &str = r#"use @acme/billing:invoices.{Invoice}

model Order {
  id: uuid
  invoice?: Invoice
}

service Orders at "/orders" {
  get "/{id}" getOrder(id: uuid) -> Order | NotFound
}
"#;

fn lib_toml(base: &str) -> String {
    format!(
        "[project]\nname = \"libs\"\n\n[libraries.billing]\ncontracts = \"billing\"\npackage = \"@acme/billing\"\nversion = \"1.2.0\"\ndescription = \"Invoices\"\n\n[registry]\nurl = \"{base}\"\n"
    )
}

const APP_TOML: &str = r#"[project]
name = "app"
contracts = "contracts"

[[targets]]
name = "api"
role = "server"
emitter = "node"
service = "app"
out = "generated/server"
"#;

fn cached(app: &Path, file: &str) -> PathBuf {
    app.join(".veld/packages/@acme/billing/1.2.0").join(file)
}

#[test]
fn publish_add_check_generate_and_tamper_against_the_real_registry() {
    let pg = pg_bin();
    if !have_go() || pg.is_none() {
        let why = format!(
            "go on PATH: {}, PostgreSQL binaries (VELD_PG_BIN): {}",
            have_go(),
            pg.is_some()
        );
        assert!(!require(), "VELD_REQUIRE_TOOLCHAINS=1 but {why}");
        eprintln!("SKIPPED registry e2e ({why})");
        return;
    }
    let work = tempdir().unwrap();
    let work_path = work.path().to_path_buf();
    let mut env = Env {
        pg_bin: pg.unwrap(),
        pg_data: work_path.join("pgdata"),
        pg_started: false,
        registry: None,
    };

    // 1. Build the registry.
    let registry_bin = work_path.join(format!("veld-registry{EXE}"));
    run_ok(
        Command::new("go")
            .args(["build", "-o"])
            .arg(&registry_bin)
            .arg("./cmd/veld-registry")
            .current_dir(repo_root().join("registry")),
        "go build",
    );

    // 2. A throwaway PostgreSQL cluster on a free port, trust auth on localhost only.
    let pg_port = free_port();
    start_postgres(&mut env, &work_path, pg_port);

    // 3. The registry on a free port.
    let port = free_port();
    let base = format!("http://127.0.0.1:{port}");
    let dsn = format!("postgres://veld@127.0.0.1:{pg_port}/veldtest?sslmode=disable");
    let mut child = Command::new(&registry_bin)
        .args(["--addr", &format!("127.0.0.1:{port}"), "--dsn", &dsn])
        .args(["--secret", "e2e-test-secret-0123456789abcdef", "--storage"])
        .arg(work_path.join("storage"))
        .current_dir(&work_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    wait_for_registry(&base, &mut child);
    env.registry = Some(child);

    // 4. A user, an org and a write token, through the API.
    let reg = api(
        "POST",
        &format!("{base}/api/v1/auth/register"),
        None,
        Some(
            json!({"email": "e2e@example.com", "username": "e2e", "password": "correct horse battery"}),
        ),
    );
    let jwt = reg["token"].as_str().unwrap().to_string();
    api(
        "POST",
        &format!("{base}/api/v1/orgs"),
        Some(&jwt),
        Some(json!({"name": "acme"})),
    );
    let tok = api(
        "POST",
        &format!("{base}/api/v1/tokens"),
        Some(&jwt),
        Some(json!({"name": "ci", "scopes": ["read", "write"]})),
    );
    let token = tok["token"].as_str().unwrap().to_string();
    assert!(token.starts_with("vtk_"), "unexpected token shape");

    let home = work_path.join("home");
    let lib = work_path.join("lib");
    let app = work_path.join("app");
    let veld_in = |dir: &Path| {
        let mut cmd = veld(dir);
        cmd.env("VELD_HOME", &home);
        cmd
    };

    // 5a. Login: the token is validated, stored, and never printed.
    fs::create_dir_all(&lib).unwrap();
    veld_in(&lib)
        .args([
            "login",
            "--registry",
            &base,
            "--token",
            "vtk_not_a_real_token",
        ])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("rejected the token"));
    veld_in(&lib)
        .args(["login", "--registry", &base])
        .write_stdin(format!("{token}\n"))
        .assert()
        .success()
        .stdout(predicate::str::contains("logged in to").and(predicate::str::contains("as e2e")))
        .stdout(predicate::str::contains(&token).not())
        .stderr(predicate::str::contains(&token).not());
    let creds = fs::read_to_string(home.join("credentials.toml")).unwrap();
    assert!(creds.contains(&token) && creds.contains(&base));

    // 5b. Publish a library package.
    write(&lib, "veld.toml", &lib_toml(&base));
    write(&lib, "billing/invoices.veld", INVOICES);
    veld_in(&lib)
        .args(["publish", "--dry-run"])
        .assert()
        .success()
        .stdout(predicate::str::contains("@acme/billing@1.2.0"))
        .stdout(predicate::str::contains("nothing was uploaded"));
    veld_in(&lib)
        .arg("publish")
        .assert()
        .success()
        .stdout(predicate::str::contains("published @acme/billing@1.2.0"));
    veld_in(&lib)
        .arg("publish")
        .assert()
        .code(1)
        .stderr(predicate::str::contains("already published"));

    // 5c. In another project: add, check, generate.
    fs::create_dir_all(&app).unwrap();
    write(
        &app,
        "veld.toml",
        &format!("{APP_TOML}\n[registry]\nurl = \"{base}\"\n"),
    );
    write(&app, "contracts/orders.veld", ORDERS);
    veld_in(&app).arg("check").assert().code(1);
    veld_in(&app)
        .args(["add", "@acme/billing"])
        .assert()
        .success()
        .stdout(predicate::str::contains("added @acme/billing@1.2.0"));
    let toml = fs::read_to_string(app.join("veld.toml")).unwrap();
    assert!(
        toml.contains("[dependencies]") && toml.contains("\"@acme/billing\" = \"^1.2.0\""),
        "{toml}"
    );
    assert!(
        toml.contains("[[targets]]"),
        "the rest of veld.toml is preserved"
    );
    let lock = fs::read_to_string(app.join("veld.lock")).unwrap();
    assert!(
        lock.contains("name = \"@acme/billing\"") && lock.contains("version = \"1.2.0\""),
        "{lock}"
    );
    assert!(cached(&app, "contracts/invoices.veld").is_file());
    assert!(!lock.contains(&token), "the lock never holds the token");

    veld_in(&app)
        .arg("check")
        .assert()
        .success()
        .stdout(predicate::str::contains("1 service"));
    veld_in(&app).arg("generate").assert().success();
    let generated = tree(&app.join("generated/server"));
    let has_invoice = generated
        .values()
        .any(|b| String::from_utf8_lossy(b).contains("Invoice"));
    assert!(
        has_invoice,
        "the dependency's types are generated: {:?}",
        generated.keys()
    );

    // A missing cache is reinstalled from the lock by `check` (and by `install`).
    fs::remove_dir_all(app.join(".veld")).unwrap();
    veld_in(&app)
        .arg("check")
        .assert()
        .success()
        .stdout(predicate::str::contains("installed @acme/billing@1.2.0"));
    fs::remove_dir_all(app.join(".veld")).unwrap();
    veld_in(&app)
        .arg("install")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "1 package installed from veld.lock",
        ));

    // 6. Tampering fails loudly.
    let original = fs::read_to_string(cached(&app, "contracts/invoices.veld")).unwrap();
    fs::write(
        cached(&app, "contracts/invoices.veld"),
        "model Invoice { id: uuid }\n",
    )
    .unwrap();
    veld_in(&app)
        .arg("check")
        .assert()
        .code(1)
        .stderr(predicate::str::contains("checksum mismatch"));
    fs::write(cached(&app, "contracts/invoices.veld"), original).unwrap();
    veld_in(&app).arg("check").assert().success();

    let good_lock = fs::read_to_string(app.join("veld.lock")).unwrap();
    let sha_line = good_lock
        .lines()
        .find(|l| l.starts_with("sha256 = "))
        .unwrap()
        .to_string();
    let bad_lock = good_lock.replace(&sha_line, &format!("sha256 = \"{}\"", "0".repeat(64)));
    fs::write(app.join("veld.lock"), &bad_lock).unwrap();
    veld_in(&app)
        .arg("check")
        .assert()
        .code(1)
        .stderr(predicate::str::contains("checksum mismatch"));
    fs::remove_dir_all(app.join(".veld")).unwrap();
    veld_in(&app)
        .arg("install")
        .assert()
        .code(1)
        .stderr(predicate::str::contains("checksum mismatch"));
    assert!(
        !cached(&app, "contracts/invoices.veld").exists(),
        "a rejected package is never installed"
    );
    fs::write(app.join("veld.lock"), good_lock).unwrap();
    veld_in(&app).arg("install").assert().success();

    // Logout forgets the token; publishing then fails without touching the network.
    veld_in(&lib)
        .arg("logout")
        .assert()
        .success()
        .stdout(predicate::str::contains("logged out"));
    veld_in(&lib)
        .arg("publish")
        .assert()
        .code(1)
        .stderr(predicate::str::contains("not logged in"));

    // 7. Teardown happens in `Env::drop`.
}

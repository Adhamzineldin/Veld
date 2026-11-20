//! The examples run for real: each one is copied to a temporary directory, regenerated (nothing
//! may change), installed with `npm ci`, type-checked with `tsc`, booted on free ports and driven
//! over HTTP, and its React app is rendered with react-test-renderer against the live servers.
//!
//! Needs `node` with npm and, for `microservices`, a JDK. Without them a test is skipped with a
//! message, unless `VELD_REQUIRE_TOOLCHAINS=1`, in which case it fails.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use common::*;
use serde_json::{Value, json};
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Toolchain
// ---------------------------------------------------------------------------

struct Npm(Option<PathBuf>);

impl Npm {
    /// Runs `npm <args>` in `dir`. npm is started through `node npm-cli.js` when that file is found.
    fn run(&self, dir: &Path, args: &[&str]) {
        let mut cmd = match &self.0 {
            Some(cli) => {
                let mut c = Command::new("node");
                c.arg(cli);
                c
            }
            None => Command::new(if cfg!(windows) { "npm.cmd" } else { "npm" }),
        };
        let out = cmd
            .args(args)
            .current_dir(dir)
            .env("npm_config_audit", "false")
            .env("npm_config_fund", "false")
            .env("npm_config_update_notifier", "false")
            .stdin(Stdio::null())
            .output()
            .unwrap_or_else(|e| panic!("cannot start npm: {e}"));
        assert!(
            out.status.success(),
            "npm {args:?} failed in {}\n{}\n{}",
            dir.display(),
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

fn ok(cmd: &mut Command) -> bool {
    cmd.stdin(Stdio::null())
        .output()
        .is_ok_and(|o| o.status.success())
}

fn find_npm() -> Result<Npm, String> {
    if !ok(Command::new("node").arg("--version")) {
        return Err("`node` is not on PATH".into());
    }
    let probe = Command::new("node")
        .args(["-p", "process.execPath"])
        .output()
        .map_err(|e| e.to_string())?;
    let exe = PathBuf::from(String::from_utf8_lossy(&probe.stdout).trim());
    let dir = exe.parent().map(Path::to_path_buf).unwrap_or_default();
    let candidates = [
        dir.join("node_modules/npm/bin/npm-cli.js"),
        dir.join("../lib/node_modules/npm/bin/npm-cli.js"),
    ];
    if let Some(cli) = candidates.iter().find(|c| c.is_file()) {
        return Ok(Npm(Some(cli.clone())));
    }
    let npm = if cfg!(windows) { "npm.cmd" } else { "npm" };
    if ok(Command::new(npm).arg("--version")) {
        return Ok(Npm(None));
    }
    Err("npm was not found next to node or on PATH".into())
}

/// The tools, or `None` after printing why the test is skipped (or panicking when required).
fn tools(need_java: bool) -> Option<Npm> {
    let mut missing = None;
    let npm = match find_npm() {
        Ok(n) => Some(n),
        Err(why) => {
            missing = Some(why);
            None
        }
    };
    if missing.is_none()
        && need_java
        && !(ok(Command::new("javac").arg("-version")) && ok(Command::new("java").arg("-version")))
    {
        missing = Some("a JDK (`javac` and `java`) is not on PATH".into());
    }
    match missing {
        None => npm,
        Some(why) => {
            assert!(
                std::env::var("VELD_REQUIRE_TOOLCHAINS").as_deref() != Ok("1"),
                "toolchain required but unavailable: {why}"
            );
            eprintln!("SKIPPED examples e2e: {why}");
            None
        }
    }
}

// ---------------------------------------------------------------------------
// Workspace, processes, HTTP
// ---------------------------------------------------------------------------

fn example(name: &str) -> PathBuf {
    repo_root().join("examples").join(name)
}

fn skipped(name: &str) -> bool {
    matches!(name, "node_modules" | "dist" | "target" | ".veld")
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap().flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if skipped(&name) {
            continue;
        }
        let (src, dst) = (entry.path(), to.join(&name));
        if src.is_dir() {
            copy_dir(&src, &dst);
        } else {
            fs::copy(&src, &dst).unwrap();
        }
    }
}

/// Copies an example without `node_modules` and build output.
fn copy_example(name: &str) -> (TempDir, PathBuf) {
    let guard = tempfile::Builder::new()
        .prefix(&format!("veld-example-{name}-"))
        .tempdir()
        .unwrap();
    let root = guard.path().join(name);
    copy_dir(&example(name), &root);
    (guard, root)
}

fn free_port() -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    l.local_addr().unwrap().port()
}

/// A child process that is killed when dropped.
struct Svc {
    name: String,
    child: Child,
    log: PathBuf,
    port: u16,
}

impl Svc {
    fn start(name: &str, mut cmd: Command, port: u16, logs: &Path) -> Self {
        let log = logs.join(format!("{name}.log"));
        let file = fs::File::create(&log).unwrap();
        let child = cmd
            .env("PORT", port.to_string())
            .env("HOST", "127.0.0.1")
            .stdin(Stdio::null())
            .stdout(Stdio::from(file.try_clone().unwrap()))
            .stderr(Stdio::from(file))
            .spawn()
            .unwrap_or_else(|e| panic!("cannot start {name}: {e}"));
        let mut svc = Self {
            name: name.to_string(),
            child,
            log,
            port,
        };
        svc.wait_ready();
        svc
    }

    fn logs(&self) -> String {
        fs::read_to_string(&self.log).unwrap_or_default()
    }

    fn wait_ready(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(90);
        while Instant::now() < deadline {
            if TcpStream::connect(("127.0.0.1", self.port)).is_ok() {
                return;
            }
            if let Ok(Some(status)) = self.child.try_wait() {
                panic!("{} exited early ({status})\n{}", self.name, self.logs());
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        panic!(
            "{} did not listen on {}\n{}",
            self.name,
            self.port,
            self.logs()
        );
    }
}

impl Drop for Svc {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// `node --import tsx <script>` in `app` (one process, so killing it frees the port).
fn node_ts(app: &Path, script: &str) -> Command {
    let mut c = Command::new("node");
    c.args(["--import", "tsx", script]).current_dir(app);
    c
}

fn url(port: u16) -> String {
    format!("http://127.0.0.1:{port}")
}

/// One HTTP call; returns the status and the JSON body (`Null` when there is none).
fn call(method: &str, url: &str, token: Option<&str>, body: Option<&Value>) -> (u16, Value) {
    let mut req = ureq::request(method, url);
    if let Some(t) = token {
        req = req.set("authorization", &format!("Bearer {t}"));
    }
    let result = match body {
        Some(b) => req
            .set("content-type", "application/json")
            .send_string(&b.to_string()),
        None => req.call(),
    };
    let resp = match result {
        Ok(r) => r,
        Err(ureq::Error::Status(_, r)) => r,
        Err(e) => panic!("{method} {url}: {e}"),
    };
    let status = resp.status();
    let mut text = String::new();
    resp.into_reader().read_to_string(&mut text).unwrap();
    (status, serde_json::from_str(&text).unwrap_or(Value::Null))
}

/// An error answer: `application/problem+json` with the error's title and typed data.
fn assert_error(got: &(u16, Value), status: u16, title: &str) {
    assert_eq!(got.0, status, "{:?}", got.1);
    assert_eq!(got.1["title"], title, "{:?}", got.1);
    assert!(
        got.1["type"]
            .as_str()
            .unwrap_or_default()
            .starts_with("veld:"),
        "an error carries its signature: {:?}",
        got.1
    );
}

fn num(v: &Value) -> f64 {
    v.as_str().unwrap_or_default().parse().unwrap_or(f64::NAN)
}

/// Step 1 of every example: regenerating changes nothing, and `veld setup` finds it all wired.
fn assert_up_to_date(root: &Path) {
    let before = tree(root);
    veld(root).arg("generate").assert().success();
    assert_eq!(
        before,
        tree(root),
        "regenerating changed the committed example"
    );
    veld(root).arg("setup").assert().success();
    assert_eq!(
        before,
        tree(root),
        "`veld setup` changed the committed example"
    );
}

fn install_and_typecheck(npm: &Npm, apps: &[PathBuf]) {
    for app in apps {
        npm.run(app, &["ci"]);
    }
    for app in apps {
        let tsc = Command::new("node")
            .args(["node_modules/typescript/bin/tsc", "--noEmit"])
            .current_dir(app)
            .output()
            .unwrap();
        assert!(
            tsc.status.success(),
            "tsc --noEmit failed in {}\n{}{}",
            app.display(),
            String::from_utf8_lossy(&tsc.stdout),
            String::from_utf8_lossy(&tsc.stderr)
        );
    }
}

fn run_render_test(app: &Path, envs: &[(&str, String)]) {
    let mut cmd = node_ts(app, "test/render.tsx");
    for (k, v) in envs {
        cmd.env(k, v);
    }
    let out = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success() && stdout.contains("render ok"),
        "the React render test failed\n{stdout}\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

// ---------------------------------------------------------------------------
// hello
// ---------------------------------------------------------------------------

#[test]
fn hello_server_and_react_app_run() {
    let Some(npm) = tools(false) else { return };
    let (guard, root) = copy_example("hello");
    assert_up_to_date(&root);
    let (server, web) = (root.join("server"), root.join("web"));
    install_and_typecheck(&npm, &[server.clone(), web.clone()]);

    let logs = guard.path();
    let port = free_port();
    let _api = Svc::start("hello", node_ts(&server, "src/main.ts"), port, logs);
    let base = url(port);

    let greeting = call("GET", &format!("{base}/hello/Ada"), None, None);
    assert_eq!(
        (greeting.0, greeting.1["message"].clone()),
        (200, json!("Hello, Ada!"))
    );
    let missing = call("GET", &format!("{base}/hello/nobody"), None, None);
    assert_error(&missing, 404, "NotFound");
    assert_eq!(missing.1["data"]["resource"], "nobody");

    run_render_test(&web, &[("VELD_HELLO_URL", base)]);
}

// ---------------------------------------------------------------------------
// library-package
// ---------------------------------------------------------------------------

#[test]
fn library_package_services_share_types_and_errors() {
    let Some(npm) = tools(false) else { return };
    let (guard, root) = copy_example("library-package");
    assert_up_to_date(&root);
    let apps: Vec<PathBuf> = ["orders", "invoices", "web"]
        .iter()
        .map(|a| root.join(a))
        .collect();
    install_and_typecheck(&npm, &apps);

    let (orders_port, invoices_port) = (free_port(), free_port());
    let _orders = Svc::start(
        "orders",
        node_ts(&apps[0], "src/main.ts"),
        orders_port,
        guard.path(),
    );
    let _invoices = Svc::start(
        "invoices",
        node_ts(&apps[1], "src/main.ts"),
        invoices_port,
        guard.path(),
    );
    let (orders, invoices) = (url(orders_port), url(invoices_port));
    let token = Some("demo-token");

    assert_error(
        &call("GET", &format!("{orders}/orders"), None, None),
        401,
        "Unauthorized",
    );
    let money = json!({ "amount": "10.00", "currency": "USD" });
    let mut ids = Vec::new();
    for _ in 0..3 {
        let (status, order) = call(
            "POST",
            &format!("{orders}/orders"),
            token,
            Some(&json!({ "total": money })),
        );
        assert_eq!(status, 201, "{order:?}");
        assert_eq!(order["total"], money, "the shared Money type round-trips");
        ids.push(order["id"].clone());
    }
    // The fourth open order hits the plan limit: an error declared in the shared library.
    let limit = call(
        "POST",
        &format!("{orders}/orders"),
        token,
        Some(&json!({ "total": money })),
    );
    assert_error(&limit, 402, "PlanLimitReached");
    assert!(
        limit.1["type"]
            .as_str()
            .unwrap()
            .starts_with("veld:common/PlanLimitReached@")
    );
    let (status, page) = call(
        "GET",
        &format!("{orders}/orders?page=1&size=2"),
        token,
        None,
    );
    assert_eq!(
        (
            status,
            page["items"].as_array().map(Vec::len),
            page["total"].clone()
        ),
        (200, Some(2), json!("3"))
    );

    // The big invoice is flaky once: the shared `Unavailable` (declared in `common`), then success.
    let (_, list) = call("GET", &format!("{invoices}/invoices"), token, None);
    let big = list["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| num(&i["due"]["amount"]) > 5000.0)
        .unwrap()
        .clone();
    let pay = format!("{invoices}/invoices/{}/pay", big["id"].as_str().unwrap());
    let first = call("POST", &pay, token, None);
    assert_error(&first, 503, "Unavailable");
    assert!(
        first.1["type"]
            .as_str()
            .unwrap()
            .starts_with("veld:common/Unavailable@")
    );
    let second = call("POST", &pay, token, None);
    assert_eq!((second.0, second.1["paid"].clone()), (200, json!(true)));

    // The typed client script (`web`) does the same through `@veld/web`.
    let out = node_ts(&apps[2], "src/report.ts")
        .env("VELD_ORDERS_URL", &orders)
        .env("VELD_INVOICES_URL", &invoices)
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "{text}\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(text.contains("plan limit reached"), "{text}");
}

// ---------------------------------------------------------------------------
// microservices
// ---------------------------------------------------------------------------

struct Stack {
    iam: String,
    accounts: String,
    transactions: String,
    cards: String,
    notifications: String,
    _svcs: Vec<Svc>,
}

/// Compiles the Java service with `javac` (no build tool needed: it has no dependencies).
fn compile_java(service: &Path, out: &Path) {
    let mut sources = Vec::new();
    fn walk(dir: &Path, acc: &mut Vec<String>) {
        for e in fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, acc);
            } else if p.extension().is_some_and(|x| x == "java") {
                acc.push(p.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    walk(&service.join("src/main/java"), &mut sources);
    assert!(sources.len() > 50);
    let args = out.join("sources.args");
    fs::write(&args, sources.join("\n")).unwrap();
    let classes = out.join("classes");
    let res = Command::new("javac")
        .args(["--release", "17", "-d"])
        .arg(&classes)
        .arg(format!("@{}", args.display()))
        .output()
        .unwrap();
    assert!(
        res.status.success(),
        "javac failed\n{}{}",
        String::from_utf8_lossy(&res.stdout),
        String::from_utf8_lossy(&res.stderr)
    );
}

fn boot_stack(root: &Path, logs: &Path) -> Stack {
    let ports: BTreeMap<&str, u16> = ["iam", "accounts", "transactions", "cards", "notifications"]
        .into_iter()
        .map(|n| (n, free_port()))
        .collect();
    let envs: Vec<(String, String)> = ports
        .iter()
        .map(|(n, p)| (format!("VELD_{}_URL", n.to_uppercase()), url(*p)))
        .collect();
    let node_service = |dir: &str, name: &str| {
        let mut c = node_ts(&root.join("backend").join(dir), "src/main.ts");
        c.envs(envs.iter().cloned());
        Svc::start(name, c, ports[name], logs)
    };
    let mut svcs = vec![
        node_service("iam-service", "iam"),
        node_service("account-service", "accounts"),
        node_service("card-service", "cards"),
        node_service("notification-service", "notifications"),
    ];
    let classes = logs.join("classes");
    let mut java = Command::new("java");
    java.arg("-cp")
        .arg(&classes)
        .arg("com.nexusbank.transactions.app.TransactionServiceMain")
        .envs(envs.iter().cloned());
    svcs.push(Svc::start(
        "transactions",
        java,
        ports["transactions"],
        logs,
    ));
    Stack {
        iam: url(ports["iam"]),
        accounts: url(ports["accounts"]),
        transactions: url(ports["transactions"]),
        cards: url(ports["cards"]),
        notifications: url(ports["notifications"]),
        _svcs: svcs,
    }
}

fn sign_up(s: &Stack, email: &str) -> String {
    let user = json!({ "email": email, "password": "correct-horse-battery", "firstName": "Ada", "lastName": "Lovelace" });
    let (status, body) = call(
        "POST",
        &format!("{}/api/iam/register", s.iam),
        None,
        Some(&user),
    );
    assert_eq!(status, 201, "{body:?}");
    let dup = call(
        "POST",
        &format!("{}/api/iam/register", s.iam),
        None,
        Some(&user),
    );
    assert_error(&dup, 409, "EmailTaken");
    assert_eq!(dup.1["data"]["email"], email);
    let bad = call(
        "POST",
        &format!("{}/api/iam/login", s.iam),
        None,
        Some(&json!({ "email": email, "password": "wrong" })),
    );
    assert_error(&bad, 401, "Unauthorized");
    let (status, tokens) = call(
        "POST",
        &format!("{}/api/iam/login", s.iam),
        None,
        Some(&json!({ "email": email, "password": "correct-horse-battery" })),
    );
    assert_eq!(status, 200, "{tokens:?}");
    tokens["accessToken"].as_str().unwrap().to_string()
}

#[test]
fn microservices_run_together_across_node_and_java() {
    let Some(npm) = tools(true) else { return };
    let (guard, root) = copy_example("microservices");
    assert_up_to_date(&root);
    let mut apps: Vec<PathBuf> = [
        "iam-service",
        "account-service",
        "card-service",
        "notification-service",
    ]
    .iter()
    .map(|d| root.join("backend").join(d))
    .collect();
    apps.push(root.join("web"));
    install_and_typecheck(&npm, &apps);
    compile_java(&root.join("backend/transaction-service"), guard.path());

    let s = boot_stack(&root, guard.path());
    let token = sign_up(&s, "ada@example.com");
    let t = Some(token.as_str());
    scripted_flow(&s, t);
    cards_and_notifications(&s, t);

    // The React app, against the same live services (it signs up its own user).
    run_render_test(
        &root.join("web"),
        &[
            ("VELD_IAM_URL", s.iam.clone()),
            ("VELD_ACCOUNTS_URL", s.accounts.clone()),
            ("VELD_TRANSACTIONS_URL", s.transactions.clone()),
        ],
    );
}

/// accounts (Node) and transactions (Java) call each other; typed errors cross both ways.
fn scripted_flow(s: &Stack, t: Option<&str>) {
    let unauth = call("GET", &format!("{}/api/accounts", s.accounts), None, None);
    assert_error(&unauth, 401, "Unauthorized");

    let new = json!({ "type": "checking", "name": "Everyday", "currency": "EUR" });
    let (status, account) = call(
        "POST",
        &format!("{}/api/accounts", s.accounts),
        t,
        Some(&new),
    );
    assert_eq!(status, 201, "{account:?}");
    let id = account["id"].as_str().unwrap().to_string();
    assert_eq!(num(&account["balance"]), 1000.0);

    // Node accounts -> Java transactions (which calls Node accounts and iam back) -> Node accounts.
    let (status, statement) = call(
        "GET",
        &format!("{}/api/accounts/{id}/statement", s.accounts),
        t,
        None,
    );
    assert_eq!(status, 200, "{statement:?}");
    assert_eq!(statement["account"]["id"], id);
    assert_eq!(statement["recent"][0]["type"], "credit");

    // A transfer: Java transactions asks Node accounts to check the source account.
    let transfer = |amount: &str, from: &str| {
        let body = json!({ "fromAccountId": from, "toIban": "DE89370400440532013000", "amount": amount, "currency": "EUR" });
        call(
            "POST",
            &format!("{}/api/transactions/transfer", s.transactions),
            t,
            Some(&body),
        )
    };
    let (status, receipt) = transfer("250.00", &id);
    assert_eq!(status, 201, "{receipt:?}");
    assert_eq!(num(&receipt["source"]["balance"]), 750.0);
    assert_eq!(receipt["transaction"]["type"], "transfer");

    // A typed error raised in Java, with typed data.
    let broke = transfer("9999.00", &id);
    assert_error(&broke, 422, "InsufficientFunds");
    assert!(
        broke.1["type"]
            .as_str()
            .unwrap()
            .starts_with("veld:transactions/InsufficientFunds@")
    );
    assert_eq!(
        (
            num(&broke.1["data"]["available"]),
            num(&broke.1["data"]["required"])
        ),
        (750.0, 9999.0)
    );

    // A typed error raised in Node (accounts), carried through Java, back to the caller.
    let ghost = "00000000-0000-4000-8000-000000000000";
    let lost = transfer("1.00", ghost);
    assert_error(&lost, 404, "NotFound");
    assert!(
        lost.1["operation"]
            .as_str()
            .unwrap()
            .contains("Transactions.transfer"),
        "{:?}",
        lost.1
    );
    let history = call(
        "GET",
        &format!(
            "{}/api/transactions/account/{ghost}?page=1&size=5",
            s.transactions
        ),
        t,
        None,
    );
    assert_error(&history, 404, "NotFound");

    // The statement now shows the transfer (Node -> Java -> Node again).
    let (_, statement) = call(
        "GET",
        &format!("{}/api/accounts/{id}/statement", s.accounts),
        t,
        None,
    );
    let kinds: Vec<&str> = statement["recent"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|x| x["type"].as_str())
        .collect();
    assert!(
        kinds.contains(&"transfer") && kinds.contains(&"credit"),
        "{kinds:?}"
    );
    let (status, page) = call(
        "GET",
        &format!(
            "{}/api/transactions/account/{id}?page=1&size=1",
            s.transactions
        ),
        t,
        None,
    );
    assert_eq!(
        (
            status,
            page["items"].as_array().map(Vec::len),
            page["total"].clone()
        ),
        (200, Some(1), json!("2"))
    );

    // A typed error raised in accounts itself.
    let own = call(
        "GET",
        &format!("{}/api/accounts/{ghost}/statement", s.accounts),
        t,
        None,
    );
    assert_error(&own, 404, "NotFound");
    let frozen = call(
        "POST",
        &format!("{}/api/accounts/{id}/freeze", s.accounts),
        t,
        None,
    );
    assert_eq!(
        (frozen.0, frozen.1["status"].clone()),
        (200, json!("frozen"))
    );
    assert_error(
        &call(
            "POST",
            &format!("{}/api/accounts/{id}/freeze", s.accounts),
            t,
            None,
        ),
        409,
        "AccountNotActive",
    );
    assert_error(
        &call(
            "POST",
            &format!("{}/api/accounts/{id}/ban", s.accounts),
            t,
            None,
        ),
        403,
        "Forbidden",
    );
}

fn cards_and_notifications(s: &Stack, t: Option<&str>) {
    let (_, account) = call(
        "POST",
        &format!("{}/api/accounts", s.accounts),
        t,
        Some(&json!({ "type": "savings", "name": "Rainy day", "currency": "EUR" })),
    );
    let body = |id: &str| json!({ "accountId": id, "type": "debit", "network": "visa" });
    // cards (Fastify) -> accounts (Express): the account is checked by a generated client.
    let (status, card) = call(
        "POST",
        &format!("{}/api/cards", s.cards),
        t,
        Some(&body(account["id"].as_str().unwrap())),
    );
    assert_eq!(status, 201, "{card:?}");
    assert_eq!(card["last4"].as_str().map(str::len), Some(4));
    let ghost = "00000000-0000-4000-8000-000000000000";
    assert_error(
        &call(
            "POST",
            &format!("{}/api/cards", s.cards),
            t,
            Some(&body(ghost)),
        ),
        404,
        "NotFound",
    );
    let id = card["id"].as_str().unwrap();
    let frozen = call(
        "POST",
        &format!("{}/api/cards/{id}/freeze", s.cards),
        t,
        None,
    );
    assert_eq!(
        (frozen.0, frozen.1["status"].clone()),
        (200, json!("frozen"))
    );
    assert_eq!(
        call("DELETE", &format!("{}/api/cards/{id}", s.cards), t, None).0,
        204
    );
    assert_error(
        &call(
            "POST",
            &format!("{}/api/cards/{id}/freeze", s.cards),
            t,
            None,
        ),
        409,
        "CardCancelled",
    );

    // notifications (Hono) authenticates with iam.
    assert_error(
        &call(
            "GET",
            &format!("{}/api/notifications", s.notifications),
            None,
            None,
        ),
        401,
        "Unauthorized",
    );
    let (_, list) = call(
        "GET",
        &format!("{}/api/notifications", s.notifications),
        t,
        None,
    );
    let first = list[0]["id"].as_str().unwrap();
    let read = call(
        "PATCH",
        &format!("{}/api/notifications/{first}/read", s.notifications),
        t,
        None,
    );
    assert_eq!((read.0, read.1["read"].clone()), (200, json!(true)));
    let prefs = call(
        "PATCH",
        &format!("{}/api/notifications/preferences", s.notifications),
        t,
        Some(&json!({ "marketing": true })),
    );
    assert_eq!(
        (
            prefs.0,
            prefs.1["marketing"].clone(),
            prefs.1["security"].clone()
        ),
        (200, json!(true), json!(true))
    );

    // Signing out invalidates the token for every service.
    assert_eq!(
        call("POST", &format!("{}/api/iam/logout", s.iam), t, None).0,
        204
    );
    assert_error(
        &call("GET", &format!("{}/api/accounts", s.accounts), t, None),
        401,
        "Unauthorized",
    );
}

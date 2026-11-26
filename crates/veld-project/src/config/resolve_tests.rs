use super::*;

fn root() -> PathBuf {
    std::env::temp_dir().join("veld-resolve-proj")
}

fn files(items: &[(&str, &str)]) -> Vec<(String, String)> {
    items
        .iter()
        .map(|(d, t)| ((*d).to_string(), (*t).to_string()))
        .collect()
}

fn load(root_text: &str, members: &[(&str, &str)]) -> Result<Project, ConfigError> {
    resolve(&root(), root_text, &files(members))
}

const SINGLE: &str = r#"
[service]
name = "shop"

[server]
emitter = "node"
framework = "express"
out = "../api/src/generated"

[client]
emitter = "typescript"
out = "../web/src/generated"
"#;

const BANK: &str = r#"
[workspace]
name = "bank"
members = ["libs/*", "services/*", "apps/*"]
"#;

const COMMON: (&str, &str) = ("libs/common", "[library]\n");
const IAM: (&str, &str) = (
    "services/iam",
    "[service]\nbase-url = \"http://iam:3001\"\n",
);
const ORDERS: (&str, &str) = (
    "services/orders",
    "[service]\nconsumes = [\"iam\", \"payments\"]\n[server]\nemitter = \"node\"\n",
);
const PAYMENTS: (&str, &str) = ("services/payments", "[service]\nconsumes = [\"orders\"]\n");
const WEB: (&str, &str) = (
    "apps/web",
    "[app]\nuses = [\"iam\", \"orders\"]\n[client]\nemitter = \"typescript\"\n",
);

fn bank() -> Project {
    load(BANK, &[COMMON, IAM, ORDERS, PAYMENTS, WEB]).unwrap()
}

#[test]
fn single_file_workspace() {
    let p = load(SINGLE, &[]).unwrap();
    assert_eq!(p.name, "shop");
    assert_eq!(p.packages.len(), 1);
    assert_eq!(p.packages[0].name, "shop");
    assert_eq!(p.packages[0].root, normalize(&root().join("contracts")));
    assert_eq!(p.targets[0].service.as_deref(), Some("shop"));
    assert_eq!(p.targets[1].services, ["shop"]);
    assert_eq!(
        p.targets[0].out,
        normalize(&root().join("../api/src/generated"))
    );
    assert_eq!(p.targets[0].name, "shop-server");
    assert_eq!(p.targets[1].name, "shop-client");
    assert!(p.targets[0].clients);
    assert_eq!(p.targets[0].app, None);
    assert_eq!(p.members.len(), 1);
    assert_eq!(p.members[0].targets, ["shop-server", "shop-client"]);
}

#[test]
fn one_target_takes_the_member_name_and_default_out_follows_the_emitter() {
    let p = load(
        "[service]\nname = \"a\"\n[[server]]\nname = \"j\"\nemitter = \"java\"\n[[server]]\nname = \"r\"\nemitter = \"rust\"\n",
        &[],
    )
    .unwrap();
    assert_eq!(p.targets[0].out, normalize(&root().join("src/main/java")));
    assert_eq!(p.targets[1].out, normalize(&root().join("generated")));
    let p = load(
        "[service]\nname = \"a\"\n[client]\nemitter = \"typescript\"\n",
        &[],
    )
    .unwrap();
    assert_eq!(p.targets[0].name, "a");
    assert_eq!(p.targets[0].out, normalize(&root().join("src/generated")));
}

#[test]
fn two_targets_of_a_role_need_names_and_a_plugin_needs_out() {
    let two = "[service]\nname = \"a\"\n[[server]]\nemitter = \"node\"\nout = \"x\"\n[[server]]\nemitter = \"java\"\n";
    assert_eq!(load(two, &[]).unwrap_err().code(), Some("E0710"));
    let plugin =
        "[service]\nname = \"a\"\n[server]\nemitter = \"plugin:k\"\n[plugins.k]\ncommand = \"x\"\n";
    assert_eq!(load(plugin, &[]).unwrap_err().code(), Some("E0700"));
    let undeclared = "[service]\nname = \"a\"\n[server]\nemitter = \"plugin:k\"\nout = \"o\"\n";
    assert_eq!(load(undeclared, &[]).unwrap_err().code(), Some("E0700"));
}

#[test]
fn workspace_members_packages_and_targets_are_ordered() {
    let p = bank();
    let names: Vec<_> = p.members.iter().map(|m| m.name.as_str()).collect();
    assert_eq!(names, ["web", "common", "iam", "orders", "payments"]);
    let pkgs: Vec<_> = p.packages.iter().map(|x| x.name.as_str()).collect();
    assert_eq!(pkgs, ["common", "iam", "orders", "payments"]);
    assert_eq!(p.package("common").unwrap().kind, PackageKind::Library);
    assert_eq!(p.package("orders").unwrap().consumes, ["iam", "payments"]);
    assert_eq!(
        p.package("iam").unwrap().base_url.as_deref(),
        Some("http://iam:3001")
    );
    assert_eq!(
        p.package("orders").unwrap().root,
        normalize(&root().join("services/orders/contracts"))
    );
    let targets: Vec<_> = p.targets.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(targets, ["web", "orders"]);
    assert_eq!(p.targets[0].services, ["iam", "orders"]);
    assert_eq!(p.targets[1].service.as_deref(), Some("orders"));
    assert_eq!(p.targets[1].member, "orders");
    assert!(p.warnings.is_empty());
}

#[test]
fn an_app_uses_every_service_by_default_and_checks_the_names() {
    let web = ("apps/web", "[app]\n[client]\nemitter = \"typescript\"\n");
    let p = load(BANK, &[COMMON, IAM, ORDERS, PAYMENTS, web]).unwrap();
    let client = p.targets.iter().find(|t| t.member == "web").unwrap();
    assert_eq!(client.services, ["iam", "orders", "payments"]);
    assert_eq!(
        p.member("web").unwrap().depends_on,
        ["iam", "orders", "payments"]
    );
    let bad = ("apps/web", "[app]\nuses = [\"iamm\"]\n");
    let err = load(BANK, &[COMMON, IAM, bad]).unwrap_err();
    assert_eq!(err.code(), Some("E0702"));
    assert!(err.to_string().contains("did you mean `iam`"), "{err}");
}

#[test]
fn unknown_consumed_service_is_e0702_with_suggestion() {
    let orders = ("services/orders", "[service]\nconsumes = [\"iamm\"]\n");
    let err = load(BANK, &[IAM, orders]).unwrap_err();
    assert_eq!(err.code(), Some("E0702"));
    assert!(err.to_string().contains("did you mean `iam`"), "{err}");
    let lib = ("services/orders", "[service]\nconsumes = [\"common\"]\n");
    assert_eq!(
        load(BANK, &[COMMON, lib]).unwrap_err().code(),
        Some("E0702")
    );
}

#[test]
fn consumes_wildcard_expands_to_every_other_service() {
    let orders = ("services/orders", "[service]\nconsumes = [\"*\"]\n");
    let p = load(BANK, &[IAM, orders, PAYMENTS]).unwrap();
    assert_eq!(p.package("orders").unwrap().consumes, ["iam", "payments"]);
    let mixed = (
        "services/orders",
        "[service]\nconsumes = [\"payments\", \"*\"]\n",
    );
    let p = load(BANK, &[IAM, mixed, PAYMENTS]).unwrap();
    assert_eq!(p.package("orders").unwrap().consumes, ["payments", "iam"]);
}

#[test]
fn self_consumption_warns() {
    let orders = (
        "services/orders",
        "[service]\nconsumes = [\"orders\", \"iam\"]\n",
    );
    let p = load(BANK, &[IAM, orders]).unwrap();
    assert_eq!(p.warnings.len(), 1);
    assert_eq!(p.warnings[0].code, "W0701");
    assert_eq!(p.package("orders").unwrap().consumes, ["iam"]);
}

#[test]
fn the_messages_directory_of_a_service_defaults_to_messages_next_to_the_member() {
    let p = load("[service]\nname = \"s\"\n", &[]).unwrap();
    let m = p.package("s").unwrap().messages.clone().unwrap();
    assert_eq!(m.name, "messages");
    assert_eq!(m.dir, normalize(&root().join("messages")));
    assert!(!m.explicit);
}

#[test]
fn the_messages_key_names_another_directory_and_a_library_has_none() {
    let p = load(
        "[service]\nname = \"s\"\nmessages = \"./texts/\"\n",
        &[],
    )
    .unwrap();
    let m = p.package("s").unwrap().messages.clone().unwrap();
    assert_eq!((m.name.as_str(), m.explicit), ("texts", true));
    assert_eq!(m.dir, normalize(&root().join("texts")));
    let bank = load(BANK, &[COMMON, IAM]).unwrap();
    assert_eq!(bank.package("common").unwrap().messages, None);
    assert!(bank.package("iam").unwrap().messages.is_some());
}

#[test]
fn a_messages_key_that_is_empty_or_absolute_is_refused() {
    for bad in ["", "/etc/messages"] {
        let text = format!("[service]\nname = \"s\"\nmessages = \"{bad}\"\n");
        assert!(load(&text, &[]).is_err(), "{bad:?}");
    }
}

#[test]
fn prefixes_join_workspace_then_service() {
    let root_text = "[workspace]\nname = \"b\"\nmembers = [\"s/*\"]\nprefix = \"/api/\"\n";
    let iam = ("s/iam", "[service]\nprefix = \"/v1\"\n");
    let orders = ("s/orders", "[service]\n");
    let p = load(root_text, &[iam, orders]).unwrap();
    assert_eq!(p.package("iam").unwrap().prefix, "/api/v1");
    assert_eq!(p.package("orders").unwrap().prefix, "/api");
    assert_eq!(p.prefixes().len(), 2);
    let bad = load("[service]\nname = \"s\"\nprefix = \"api\"\n", &[]).unwrap_err();
    assert!(bad.to_string().contains("must start with"), "{bad}");
}

#[test]
fn defaults_apply_and_the_member_wins_key_by_key() {
    let root_text = "[workspace]\nname = \"w\"\nmembers = [\"s/*\"]\n\
[workspace.defaults.server.java]\nframework = \"spring\"\nframework-version = \"3\"\njava = \"21\"\npackage = \"com.d\"\n";
    let plain = ("s/a", "[service]\n[server]\nemitter = \"java\"\n");
    let own = (
        "s/b",
        "[service]\n[server]\nemitter = \"java\"\nframework = \"jdk\"\n[server.options]\npackage = \"com.b\"\n",
    );
    let other = ("s/c", "[service]\n[server]\nemitter = \"node\"\n");
    let p = load(root_text, &[plain, own, other]).unwrap();
    let a = &p.targets[0];
    assert_eq!(a.framework.as_deref(), Some("spring"));
    assert_eq!(a.options["java"].as_str(), Some("21"));
    assert_eq!(a.options["framework-version"].as_str(), Some("3"));
    assert!(!a.options.contains_key("framework"));
    let b = &p.targets[1];
    assert_eq!(b.framework.as_deref(), Some("jdk"));
    assert!(!b.options.contains_key("framework-version"));
    assert_eq!(b.options["package"].as_str(), Some("com.b"));
    assert_eq!(b.options["java"].as_str(), Some("21"));
    assert_eq!(p.targets[2].framework, None);
}

#[test]
fn defaults_are_kept_for_the_generators_to_check() {
    let root_text = "[workspace]\nname = \"w\"\nmembers = [\"s/*\"]\n\
[workspace.defaults.server.node]\nframework = \"express\"\n\
[workspace.defaults.client.typescript]\ntypescript = \"es2022\"\n";
    let p = load(root_text, &[("s/a", "[service]\n")]).unwrap();
    assert_eq!(p.defaults.len(), 2);
    let client = &p.defaults[0];
    assert_eq!(
        (
            client.role,
            client.emitter.clone(),
            client.framework.clone()
        ),
        (Role::Client, EmitterRef::Builtin("typescript".into()), None)
    );
    assert_eq!(client.options["typescript"].as_str(), Some("es2022"));
    assert_eq!(p.defaults[1].framework.as_deref(), Some("express"));
    assert!(p.defaults[1].options.is_empty());
    let undeclared = "[workspace]\nname = \"w\"\n[workspace.defaults.server.\"plugin:k\"]\nx = 1\n[service]\nname = \"s\"\n";
    assert_eq!(load(undeclared, &[]).unwrap_err().code(), Some("E0700"));
}

#[test]
fn structure_errors_are_e0710() {
    let cases: Vec<(&str, Vec<(&str, &str)>)> = vec![
        ("[registry]\nurl = \"https://r\"\n", vec![]),
        ("[service]\nname = \"a\"\n[app]\n", vec![]),
        ("[service]\n", vec![]),
        (
            "[workspace]\nname = \"w\"\nmembers = [\"s/*\"]\n",
            vec![("s/a", "[registry]\n")],
        ),
        (
            "[workspace]\nname = \"w\"\nmembers = [\"s/*\"]\n",
            vec![("s/a", "[workspace]\nname = \"x\"\n")],
        ),
        (
            "[workspace]\nname = \"w\"\nmembers = [\"s/*\"]\n",
            vec![("s/a b", "[service]\n")],
        ),
        (
            "[workspace]\nname = \"w\"\nmembers = [\"s/*\", \"t/*\"]\n",
            vec![("s/a", "[service]\n"), ("t/a", "[service]\n")],
        ),
    ];
    for (root_text, members) in cases {
        let err = load(root_text, &members).unwrap_err();
        assert_eq!(err.code(), Some("E0710"), "{root_text} {members:?}: {err}");
    }
}

#[test]
fn a_duplicate_member_name_names_both_files() {
    let err = load(
        "[workspace]\nname = \"w\"\nmembers = [\"s/*\", \"t/*\"]\n",
        &[("s/a", "[service]\n"), ("t/a", "[library]\n")],
    )
    .unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("s/a/veld.toml") && msg.contains("t/a/veld.toml"),
        "{msg}"
    );
}

#[test]
fn the_root_member_must_set_a_name_and_the_workspace_a_name() {
    let err = load("[workspace]\nname = \"w\"\n[service]\n", &[]).unwrap_err();
    assert_eq!(err.code(), Some("E0710"));
    let p = load(
        "[workspace]\nname = \"w\"\n[service]\nname = \"root\"\n",
        &[],
    )
    .unwrap();
    assert_eq!(p.members[0].name, "root");
    assert_eq!(p.members[0].dir, root());
    let err = load("[workspace]\nmembers = []\n", &[]).unwrap_err();
    assert_eq!(err.code(), Some("E0700"));
}

#[test]
fn a_name_in_the_table_replaces_the_directory_name() {
    let odd = ("s/a b", "[service]\nname = \"ab\"\n");
    let p = load("[workspace]\nname = \"w\"\nmembers = [\"s/*\"]\n", &[odd]).unwrap();
    assert_eq!(p.members[0].name, "ab");
}

#[test]
fn duplicate_target_names_are_e0710() {
    let a = (
        "s/a",
        "[service]\n[server]\nname = \"api\"\nemitter = \"node\"\n",
    );
    let b = (
        "s/b",
        "[service]\n[server]\nname = \"api\"\nemitter = \"node\"\n",
    );
    let err = load("[workspace]\nname = \"w\"\nmembers = [\"s/*\"]\n", &[a, b]).unwrap_err();
    assert_eq!(err.code(), Some("E0710"));
}

#[test]
fn tables_in_the_wrong_place_are_e0711() {
    let root_text = "[workspace]\nname = \"w\"\nmembers = [\"s/*\"]\n";
    for (member, needle) in [
        (
            "[service]\n[dependencies]\n\"@a/b\" = \"^1\"\n",
            "workspace root",
        ),
        (
            "[app]\n[server]\nemitter = \"node\"\n",
            "an app has no server",
        ),
        (
            "[library]\n[client]\nemitter = \"typescript\"\n",
            "a library has no targets",
        ),
    ] {
        let err = load(root_text, &[("s/a", member)]).unwrap_err();
        assert_eq!(err.code(), Some("E0711"), "{member}");
        assert!(err.to_string().contains(needle), "{err}");
    }
}

#[test]
fn overlapping_outs_are_e0703_and_name_both_targets_and_the_directory() {
    let same = SINGLE.replace("../web/src/generated", "../api/src/generated");
    let err = load(&same, &[]).unwrap_err();
    assert_eq!(err.code(), Some("E0703"));
    let msg = err.to_string();
    assert!(
        msg.contains("shop-server") && msg.contains("shop-client"),
        "{msg}"
    );
    assert!(msg.contains("../api/src/generated"), "{msg}");
    let nested = SINGLE.replace("../web/src/generated", "../api/src/generated/web");
    assert_eq!(load(&nested, &[]).unwrap_err().code(), Some("E0703"));
    let reverse = SINGLE.replace("../web/src/generated", "../api/src");
    assert_eq!(load(&reverse, &[]).unwrap_err().code(), Some("E0703"));
    let dotted = SINGLE.replace("../web/src/generated", "../x/../api/src/generated/");
    assert_eq!(load(&dotted, &[]).unwrap_err().code(), Some("E0703"));
    let defaults = "[service]\nname = \"a\"\n[server]\nemitter = \"node\"\n[client]\nemitter = \"typescript\"\n";
    assert_eq!(load(defaults, &[]).unwrap_err().code(), Some("E0703"));
}

#[test]
fn forbidden_outs_are_e0704() {
    for out in [
        ".",
        "contracts",
        "contracts/sub",
        "sub/../contracts/x",
        "..",
    ] {
        let text = SINGLE.replace("../web/src/generated", out);
        let err = load(&text, &[]).unwrap_err();
        assert_eq!(err.code(), Some("E0704"), "out = {out}: {err}");
    }
}

#[test]
fn app_and_out_resolve_against_the_member_directory() {
    let a = (
        "s/a",
        "[service]\n[server]\nemitter = \"node\"\nout = \"../../web/gen\"\napp = \"../../web\"\n",
    );
    let p = load("[workspace]\nname = \"w\"\nmembers = [\"s/*\"]\n", &[a]).unwrap();
    assert_eq!(p.targets[0].out, normalize(&root().join("web/gen")));
    assert_eq!(p.targets[0].app, Some(normalize(&root().join("web"))));
}

#[test]
fn owners_and_publish_metadata_are_carried() {
    let a = (
        "s/a",
        "[service]\nowners = [\"team-a\"]\npackage = \"@acme/a\"\nversion = \"1.0.0\"\n",
    );
    let p = load("[workspace]\nname = \"w\"\nmembers = [\"s/*\"]\n", &[a]).unwrap();
    let m = p.member("a").unwrap();
    assert_eq!(m.owners, ["team-a"]);
    assert_eq!(m.package.as_deref(), Some("@acme/a"));
    assert_eq!(m.version.as_deref(), Some("1.0.0"));
}

#[test]
fn options_registry_plugins_and_dependencies_carry_through() {
    let text = format!(
        "{SINGLE}\n[client.options]\nvalidate = true\n[registry]\nurl = \"https://r\"\n[dependencies]\n\"@a/b\" = \"^1\"\n[plugins.k]\ncommand = \"c\"\n"
    );
    let p = load(&text, &[]).unwrap();
    assert_eq!(
        p.targets[1].options.get("validate"),
        Some(&toml::Value::Boolean(true))
    );
    assert_eq!(p.registry.as_deref(), Some("https://r"));
    assert_eq!(p.dependencies["@a/b"], "^1");
    assert_eq!(p.plugins[0].name, "k");
}

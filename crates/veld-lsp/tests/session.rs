//! Scripted LSP sessions that drive the server in-process over `Connection::memory()`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fs;
use std::path::Path;
use std::thread::JoinHandle;
use std::time::Duration;

use lsp_server::{Connection, Message, Notification, Request, RequestId};
use serde_json::{Value, json};

const TOML: &str = "[project]\nname = \"shop\"\n\n[services.iam]\ncontracts = \"iam\"\n\n[services.app]\ncontracts = \"app\"\nconsumes = [\"iam\"]\n";
const USERS: &str = "/// A registered user.\nmodel User {\n  id: uuid\n  name: string\n}\n\nenum Role { admin, guest }\n";
const ORDERS: &str = "use iam:users.{User}\n\nmodel Order {\n  owner: User\n  note?: string\n}\n\nservice Orders at \"/orders\" {\n  /// Fetch one order.\n  get \"/{id}\" getOrder(id: uuid) -> Order | NotFound\n}\n";

struct Client {
    conn: Connection,
    next: i32,
    notes: Vec<Notification>,
    server: Option<JoinHandle<()>>,
}

impl Client {
    fn start(root: &Path) -> Client {
        let (server_conn, conn) = Connection::memory();
        let server = std::thread::spawn(move || {
            let _ = veld_lsp::serve(&server_conn);
        });
        let mut c = Client {
            conn,
            next: 0,
            notes: Vec::new(),
            server: Some(server),
        };
        let init = c.request(
            "initialize",
            json!({"processId": null, "capabilities": {}, "rootUri": uri(root),
                   "workspaceFolders": [{"uri": uri(root), "name": "w"}]}),
        );
        assert!(
            init["capabilities"]["renameProvider"]["prepareProvider"]
                .as_bool()
                .unwrap()
        );
        assert_eq!(init["serverInfo"]["name"], "veld");
        c.notify("initialized", json!({}));
        c
    }

    fn send(&self, m: Message) {
        self.conn.sender.send(m).unwrap();
    }

    fn notify(&self, method: &str, params: Value) {
        self.send(Message::Notification(Notification::new(
            method.into(),
            params,
        )));
    }

    fn recv(&self) -> Message {
        self.conn
            .receiver
            .recv_timeout(Duration::from_secs(20))
            .expect("the server answered in time")
    }

    fn request_raw(&mut self, method: &str, params: Value) -> lsp_server::Response {
        self.next += 1;
        let id = RequestId::from(self.next);
        self.send(Message::Request(Request::new(
            id.clone(),
            method.into(),
            params,
        )));
        loop {
            match self.recv() {
                Message::Response(r) if r.id == id => return r,
                Message::Notification(n) => self.notes.push(n),
                Message::Request(_) | Message::Response(_) => {}
            }
        }
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        let r = self.request_raw(method, params);
        assert!(r.error.is_none(), "{method}: {:?}", r.error);
        r.result.unwrap_or(Value::Null)
    }

    /// The next `publishDiagnostics` for `uri` that satisfies `pred`.
    fn diagnostics(&mut self, uri: &str, pred: impl Fn(&[Value]) -> bool) -> Vec<Value> {
        loop {
            let found = self.notes.iter().position(|n| {
                n.method == "textDocument/publishDiagnostics"
                    && n.params["uri"] == uri
                    && pred(n.params["diagnostics"].as_array().unwrap())
            });
            if let Some(i) = found {
                let n = self.notes.remove(i);
                return n.params["diagnostics"].as_array().unwrap().clone();
            }
            match self.recv() {
                Message::Notification(n) => self.notes.push(n),
                other @ (Message::Request(_) | Message::Response(_)) => {
                    panic!("unexpected message {other:?}")
                }
            }
        }
    }

    fn open(&self, uri: &str, text: &str) {
        self.notify(
            "textDocument/didOpen",
            json!({"textDocument": {"uri": uri, "languageId": "veld", "version": 1, "text": text}}),
        );
    }

    fn change(&self, uri: &str, version: i32, text: &str) {
        self.notify(
            "textDocument/didChange",
            json!({"textDocument": {"uri": uri, "version": version}, "contentChanges": [{"text": text}]}),
        );
    }

    fn at(&mut self, method: &str, uri: &str, line: u32, character: u32) -> Value {
        self.request(
            method,
            json!({"textDocument": {"uri": uri}, "position": {"line": line, "character": character}}),
        )
    }

    fn shutdown(mut self) {
        assert_eq!(self.request("shutdown", Value::Null), Value::Null);
        self.notify("exit", Value::Null);
        self.server.take().unwrap().join().unwrap();
    }
}

fn uri(p: &Path) -> String {
    veld_lsp::uri::path_to_uri(p).unwrap().as_str().to_string()
}

fn project() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    for (p, t) in [
        ("veld.toml", TOML),
        ("iam/users.veld", USERS),
        ("app/orders.veld", ORDERS),
    ] {
        let path = tmp.path().join(p);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, t).unwrap();
    }
    tmp
}

fn code(d: &Value) -> &str {
    d["code"].as_str().unwrap()
}

fn is_empty(d: &[Value]) -> bool {
    d.is_empty()
}

fn not_empty(d: &[Value]) -> bool {
    !d.is_empty()
}

#[test]
fn diagnostics_appear_and_clear_while_editing() {
    let tmp = project();
    let mut c = Client::start(tmp.path());
    let orders = uri(&tmp.path().join("app/orders.veld"));
    let users = uri(&tmp.path().join("iam/users.veld"));
    // The workspace starts clean.
    c.open(&orders, ORDERS);
    assert!(c.diagnostics(&orders, is_empty).is_empty());

    let broken = ORDERS.replace("owner: User", "owner: Usr");
    c.change(&orders, 2, &broken);
    let d = c.diagnostics(&orders, not_empty);
    // The unknown name is an error; the import it no longer uses is a warning.
    let unknown = d.iter().find(|x| code(x) == "E0205").expect("E0205");
    assert_eq!(unknown["severity"], 1);
    assert_eq!(unknown["source"], "veld");
    assert_eq!(unknown["range"]["start"]["line"], 3);
    let unused = d.iter().find(|x| code(x) == "W0201").expect("W0201");
    assert_eq!(unused["severity"], 2);

    // An error in a second file is reported for that file, and clears when it is fixed.
    c.open(&users, &USERS.replace("name: string", "name: Missing"));
    let d = c.diagnostics(&users, not_empty);
    assert_eq!(code(&d[0]), "E0205");
    c.change(&users, 2, USERS);
    c.change(&orders, 3, ORDERS);
    assert!(c.diagnostics(&users, is_empty).is_empty());
    assert!(c.diagnostics(&orders, is_empty).is_empty());
    c.shutdown();
}

#[test]
fn hover_definition_references_and_symbols() {
    let tmp = project();
    let mut c = Client::start(tmp.path());
    let orders = uri(&tmp.path().join("app/orders.veld"));
    let users = uri(&tmp.path().join("iam/users.veld"));
    c.open(&orders, ORDERS);

    let h = c.at("textDocument/hover", &orders, 3, 10);
    let md = h["contents"]["value"].as_str().unwrap();
    assert!(
        md.contains("model User") && md.contains("A registered user.") && md.contains("sig: `"),
        "{md}"
    );

    let h = c.at("textDocument/hover", &orders, 9, 22);
    let md = h["contents"]["value"].as_str().unwrap();
    assert!(
        md.contains("`GET /orders/{id}`")
            && md.contains("404 NotFound")
            && md.contains("Fetch one order."),
        "{md}"
    );

    // Across packages: the type use, the import and the module path all lead to the user file.
    for (line, col) in [(3, 10), (0, 18), (0, 10)] {
        let d = c.at("textDocument/definition", &orders, line, col);
        assert_eq!(d["uri"], users, "{line}:{col}");
    }
    assert_eq!(
        c.at("textDocument/definition", &orders, 0, 18)["range"]["start"]["line"],
        1
    );

    let refs = c.request(
        "textDocument/references",
        json!({"textDocument": {"uri": orders}, "position": {"line": 3, "character": 10},
               "context": {"includeDeclaration": true}}),
    );
    assert_eq!(refs.as_array().unwrap().len(), 3);

    let syms = c.request(
        "textDocument/documentSymbol",
        json!({"textDocument": {"uri": orders}}),
    );
    let names: Vec<_> = syms
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["Order", "Orders"]);
    let toks = c.request(
        "textDocument/semanticTokens/full",
        json!({"textDocument": {"uri": orders}}),
    );
    assert!(!toks["data"].as_array().unwrap().is_empty());
    c.shutdown();
}

#[test]
fn completion_offers_auto_import_edits() {
    let tmp = project();
    let mut c = Client::start(tmp.path());
    let uri = uri(&tmp.path().join("app/extra.veld"));
    c.open(&uri, "model Extra {\n  who: Us\n}\n");
    let items = c.at("textDocument/completion", &uri, 1, 9);
    let user = items
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["label"] == "User")
        .expect("User is offered");
    let edit = &user["additionalTextEdits"][0];
    assert_eq!(edit["newText"], "use iam:users.{User}\n\n");
    assert_eq!(edit["range"]["start"], json!({"line": 0, "character": 0}));
    let labels: Vec<_> = items
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["label"].as_str().unwrap())
        .collect();
    assert!(labels.contains(&"string") && labels.contains(&"Role"));
    // Keywords at the top level, annotations after `@`.
    c.change(&uri, 2, "mo");
    let kw = c.at("textDocument/completion", &uri, 0, 2);
    assert!(kw.as_array().unwrap().iter().any(|i| i["label"] == "model"));
    c.change(&uri, 3, "model A {\n  x: int @\n}\n");
    let an = c.at("textDocument/completion", &uri, 1, 10);
    let min = an
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["label"] == "min")
        .unwrap();
    assert_eq!(min["insertText"], "min(${1:n})");
    c.shutdown();
}

#[test]
fn rename_edits_several_files_and_refuses_reserved_words() {
    let tmp = project();
    let mut c = Client::start(tmp.path());
    let users = uri(&tmp.path().join("iam/users.veld"));
    let orders = uri(&tmp.path().join("app/orders.veld"));
    c.open(&users, USERS);
    let prep = c.at("textDocument/prepareRename", &users, 1, 8);
    assert_eq!(prep["placeholder"], "User");
    let edit = c.request(
        "textDocument/rename",
        json!({"textDocument": {"uri": users}, "position": {"line": 1, "character": 8}, "newName": "Account"}),
    );
    let changes = edit["changes"].as_object().unwrap();
    assert_eq!(changes.len(), 2);
    assert_eq!(changes[&orders].as_array().unwrap().len(), 2);
    assert_eq!(changes[&users].as_array().unwrap().len(), 1);
    let bad = c.request_raw(
        "textDocument/rename",
        json!({"textDocument": {"uri": users}, "position": {"line": 1, "character": 8}, "newName": "model"}),
    );
    assert!(bad.error.unwrap().message.contains("reserved"));
    c.shutdown();
}

#[test]
fn formatting_returns_a_full_document_edit() {
    let tmp = project();
    let mut c = Client::start(tmp.path());
    let users = uri(&tmp.path().join("iam/users.veld"));
    c.open(&users, "model   User{id:uuid}\n");
    let opts = json!({"tabSize": 2, "insertSpaces": true});
    let edits = c.request(
        "textDocument/formatting",
        json!({"textDocument": {"uri": users}, "options": opts}),
    );
    let edits = edits.as_array().unwrap();
    assert_eq!(edits.len(), 1);
    assert!(
        edits[0]["newText"]
            .as_str()
            .unwrap()
            .starts_with("model User {\n  id: uuid\n}")
    );
    c.change(&users, 2, "model User {\n  id: uuid\n}\n");
    let edits = c.request(
        "textDocument/formatting",
        json!({"textDocument": {"uri": users}, "options": opts}),
    );
    assert!(edits.as_array().unwrap().is_empty());
    c.shutdown();
}

#[test]
fn unknown_requests_fail_cleanly_and_the_server_keeps_serving() {
    let tmp = project();
    let mut c = Client::start(tmp.path());
    let r = c.request_raw("textDocument/nope", json!({}));
    assert!(r.error.is_some());
    let missing = uri(&tmp.path().join("nowhere/none.veld"));
    assert_eq!(c.at("textDocument/hover", &missing, 0, 0), Value::Null);
    c.shutdown();
}

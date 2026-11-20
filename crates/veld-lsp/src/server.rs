//! The server loop: initialize, document sync, requests and diagnostics publishing.

use std::collections::BTreeSet;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;

use lsp_server::{Connection, Message, Notification, Request, Response};
use lsp_types::notification::{
    DidChangeTextDocument, DidChangeWatchedFiles, DidCloseTextDocument, DidOpenTextDocument,
    DidSaveTextDocument, Exit, Notification as _, PublishDiagnostics,
};
use lsp_types::{
    CompletionOptions, DidChangeTextDocumentParams, DidCloseTextDocumentParams,
    DidOpenTextDocumentParams, DocumentSymbolResponse, GotoDefinitionResponse, Hover,
    HoverContents, HoverProviderCapability, InitializeParams, MarkupContent, MarkupKind, OneOf,
    PrepareRenameResponse, PublishDiagnosticsParams, RenameOptions, SemanticTokens,
    SemanticTokensFullOptions, SemanticTokensOptions, SemanticTokensResult,
    SemanticTokensServerCapabilities, ServerCapabilities, TextDocumentPositionParams,
    TextDocumentSyncCapability, TextDocumentSyncKind, TextDocumentSyncOptions, Uri,
};
use serde::de::DeserializeOwned;
use veld_compiler::FileId;

use crate::uri::uri_to_path;
use crate::workspace::{Analysis, Workspace};
use crate::{complete, diag, hover, nav, outline};

/// Errors that end the server.
#[derive(Debug, thiserror::Error)]
pub enum ServerError {
    #[error("protocol error: {0}")]
    Protocol(#[from] lsp_server::ProtocolError),
    #[error("could not read the initialize request: {0}")]
    Initialize(serde_json::Error),
    #[error("the client closed the connection")]
    Disconnected,
}

const INVALID_PARAMS: i32 = -32602;
const REQUEST_FAILED: i32 = -32803;

/// What this server offers.
#[must_use]
pub fn capabilities() -> ServerCapabilities {
    ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Options(
            TextDocumentSyncOptions {
                open_close: Some(true),
                change: Some(TextDocumentSyncKind::FULL),
                save: Some(lsp_types::TextDocumentSyncSaveOptions::Supported(true)),
                ..TextDocumentSyncOptions::default()
            },
        )),
        hover_provider: Some(HoverProviderCapability::Simple(true)),
        definition_provider: Some(OneOf::Left(true)),
        references_provider: Some(OneOf::Left(true)),
        document_symbol_provider: Some(OneOf::Left(true)),
        document_formatting_provider: Some(OneOf::Left(true)),
        rename_provider: Some(OneOf::Right(RenameOptions {
            prepare_provider: Some(true),
            work_done_progress_options: lsp_types::WorkDoneProgressOptions::default(),
        })),
        completion_provider: Some(CompletionOptions {
            trigger_characters: Some(vec![".".into(), "@".into(), ":".into(), " ".into()]),
            ..CompletionOptions::default()
        }),
        semantic_tokens_provider: Some(SemanticTokensServerCapabilities::SemanticTokensOptions(
            SemanticTokensOptions {
                legend: outline::legend(),
                full: Some(SemanticTokensFullOptions::Bool(true)),
                range: None,
                work_done_progress_options: lsp_types::WorkDoneProgressOptions::default(),
            },
        )),
        ..ServerCapabilities::default()
    }
}

/// Runs the server on stdin and stdout until the client exits.
///
/// # Errors
/// Returns an error when the transport or the handshake fails.
pub fn run_stdio() -> Result<(), ServerError> {
    let (connection, io) = Connection::stdio();
    let result = serve(&connection);
    drop(connection);
    let _ = io.join();
    result
}

/// Serves one connection: the handshake, then the message loop.
///
/// # Errors
/// Returns an error when the transport or the handshake fails.
pub fn serve(connection: &Connection) -> Result<(), ServerError> {
    let (id, params) = connection.initialize_start()?;
    let init: InitializeParams = serde_json::from_value(params).map_err(ServerError::Initialize)?;
    let result = serde_json::json!({
        "capabilities": capabilities(),
        "serverInfo": {"name": "veld", "version": env!("CARGO_PKG_VERSION")},
    });
    connection.initialize_finish(id, result)?;
    let mut server = Server::new(connection, workspace_root(&init));
    server.run()
}

#[allow(deprecated)]
fn workspace_root(init: &InitializeParams) -> Option<PathBuf> {
    let folder = init
        .workspace_folders
        .as_ref()
        .and_then(|f| f.first())
        .and_then(|f| uri_to_path(&f.uri));
    folder.or_else(|| init.root_uri.as_ref().and_then(uri_to_path))
}

struct Server<'c> {
    conn: &'c Connection,
    ws: Workspace,
    analysis: Option<Analysis>,
    dirty: bool,
    pending_publish: bool,
    published: BTreeSet<Uri>,
}

impl<'c> Server<'c> {
    fn new(conn: &'c Connection, root: Option<PathBuf>) -> Self {
        Self {
            conn,
            ws: Workspace::new(root),
            analysis: None,
            dirty: true,
            pending_publish: false,
            published: BTreeSet::new(),
        }
    }

    fn run(&mut self) -> Result<(), ServerError> {
        loop {
            let Ok(first) = self.conn.receiver.recv() else {
                return Err(ServerError::Disconnected);
            };
            let mut next = Some(first);
            while let Some(msg) = next.take() {
                if self.handle(msg)? {
                    return Ok(());
                }
                next = self.conn.receiver.try_recv().ok();
            }
            self.refresh();
            if self.pending_publish {
                self.pending_publish = false;
                self.publish();
            }
        }
    }

    /// Returns true when the server should stop.
    fn handle(&mut self, msg: Message) -> Result<bool, ServerError> {
        match msg {
            Message::Request(req) => {
                if self.conn.handle_shutdown(&req)? {
                    return Ok(true);
                }
                self.request(req);
            }
            Message::Notification(n) => {
                if n.method == Exit::METHOD {
                    return Ok(true);
                }
                self.notification(n);
            }
            Message::Response(_) => {}
        }
        Ok(false)
    }

    // ---- analysis -------------------------------------------------------------------

    /// Re-analyses the workspace when something changed. A panic inside the compiler must not
    /// take the editor session down: the previous analysis stays in place.
    fn refresh(&mut self) {
        if !self.dirty && self.analysis.is_some() {
            return;
        }
        self.dirty = false;
        let prev = self.analysis.as_ref().map(|a| a.sigs.clone());
        let ws = &self.ws;
        if let Ok(a) = catch_unwind(AssertUnwindSafe(|| ws.analyze(prev.as_ref()))) {
            self.analysis = Some(a);
            self.pending_publish = true;
        }
    }

    fn publish(&mut self) {
        let Some(a) = &self.analysis else { return };
        let mut all = diag::all(a);
        let stale: Vec<Uri> = self
            .published
            .iter()
            .filter(|u| !all.contains_key(*u))
            .cloned()
            .collect();
        for uri in stale {
            all.insert(uri, Vec::new());
        }
        let mut now = BTreeSet::new();
        let mut sends = Vec::new();
        for (uri, diagnostics) in all {
            if !diagnostics.is_empty() {
                now.insert(uri.clone());
            }
            let had = self.published.contains(&uri);
            // Skip files that never had a problem; the client has nothing to clear.
            if diagnostics.is_empty() && !had && !self.ws_has_open(&uri) {
                continue;
            }
            sends.push(PublishDiagnosticsParams::new(uri, diagnostics, None));
        }
        for p in sends {
            self.notify(PublishDiagnostics::METHOD, p);
        }
        self.published = now;
    }

    fn ws_has_open(&self, uri: &Uri) -> bool {
        uri_to_path(uri).is_some_and(|p| self.ws.buffer_text(&p).is_some())
    }

    fn notify(&self, method: &str, params: impl serde::Serialize) {
        let _ = self
            .conn
            .sender
            .send(Message::Notification(Notification::new(
                method.to_string(),
                params,
            )));
    }

    // ---- notifications ----------------------------------------------------------------

    fn notification(&mut self, n: Notification) {
        match n.method.as_str() {
            DidOpenTextDocument::METHOD => {
                if let Some(p) = parse::<DidOpenTextDocumentParams>(n.params)
                    && let Some(path) = uri_to_path(&p.text_document.uri)
                {
                    self.ws.open(&path, p.text_document.text);
                    self.dirty = true;
                }
            }
            DidChangeTextDocument::METHOD => {
                if let Some(p) = parse::<DidChangeTextDocumentParams>(n.params)
                    && let Some(path) = uri_to_path(&p.text_document.uri)
                    && let Some(change) = p.content_changes.into_iter().last()
                {
                    self.ws.open(&path, change.text);
                    self.dirty = true;
                }
            }
            DidCloseTextDocument::METHOD => {
                if let Some(p) = parse::<DidCloseTextDocumentParams>(n.params)
                    && let Some(path) = uri_to_path(&p.text_document.uri)
                {
                    self.ws.close(&path);
                    self.dirty = true;
                }
            }
            DidSaveTextDocument::METHOD | DidChangeWatchedFiles::METHOD => self.dirty = true,
            _ => {}
        }
    }

    // ---- requests -----------------------------------------------------------------------

    fn request(&mut self, req: Request) {
        self.refresh();
        let id = req.id.clone();
        let reply = match req.method.as_str() {
            "textDocument/hover" => self.with_position(req, Self::hover),
            "textDocument/definition" => self.with_position(req, Self::definition),
            "textDocument/references" => self.references(req),
            "textDocument/completion" => self.with_position(req, Self::completion),
            "textDocument/documentSymbol" => self.with_document(req, Self::symbols),
            "textDocument/formatting" => self.with_document(req, Self::formatting),
            "textDocument/semanticTokens/full" => self.with_document(req, Self::tokens),
            "textDocument/prepareRename" => self.with_position(req, Self::prepare_rename),
            "textDocument/rename" => self.rename(req),
            other => Err((REQUEST_FAILED, format!("unsupported request `{other}`"))),
        };
        let response = match reply {
            Ok(value) => Response::new_ok(id, value),
            Err((code, message)) => Response::new_err(id, code, message),
        };
        let _ = self.conn.sender.send(Message::Response(response));
    }

    /// The analysis and the file a URI names.
    fn locate(&self, uri: &Uri) -> Option<(&Analysis, FileId)> {
        let a = self.analysis.as_ref()?;
        let file = a.file_for(&uri_to_path(uri)?)?;
        Some((a, file))
    }

    fn with_position(
        &self,
        req: Request,
        f: fn(&Self, &Analysis, FileId, usize) -> Reply,
    ) -> Reply {
        let p: TextDocumentPositionParams = params(req.params)?;
        let Some((a, file)) = self.locate(&p.text_document.uri) else {
            return Ok(serde_json::Value::Null);
        };
        let offset = a.lines(file).map_or(0, |l| l.offset(p.position));
        f(self, a, file, offset)
    }

    fn with_document(&self, req: Request, f: fn(&Self, &Analysis, FileId) -> Reply) -> Reply {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct P {
            text_document: lsp_types::TextDocumentIdentifier,
        }
        let p: P = params(req.params)?;
        match self.locate(&p.text_document.uri) {
            Some((a, file)) => f(self, a, file),
            None => Ok(serde_json::Value::Null),
        }
    }

    fn hover(&self, a: &Analysis, file: FileId, offset: usize) -> Reply {
        let Some(h) = hover::hover(a, file, to_u32(offset)) else {
            return Ok(serde_json::Value::Null);
        };
        let range = a
            .lines(file)
            .map(|l| l.range(h.span.start as usize, h.span.end as usize));
        json(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: h.markdown,
            }),
            range,
        })
    }

    fn definition(&self, a: &Analysis, file: FileId, offset: usize) -> Reply {
        match nav::definition(a, file, to_u32(offset)) {
            Some(loc) => json(GotoDefinitionResponse::Scalar(loc)),
            None => Ok(serde_json::Value::Null),
        }
    }

    fn references(&self, req: Request) -> Reply {
        let p: lsp_types::ReferenceParams = params(req.params)?;
        let uri = &p.text_document_position.text_document.uri;
        let Some((a, file)) = self.locate(uri) else {
            return Ok(serde_json::Value::Null);
        };
        let pos = p.text_document_position.position;
        let offset = a.lines(file).map_or(0, |l| l.offset(pos));
        json(nav::references(
            a,
            file,
            to_u32(offset),
            p.context.include_declaration,
        ))
    }

    fn completion(&self, a: &Analysis, file: FileId, offset: usize) -> Reply {
        json(complete::complete(a, file, offset))
    }

    fn symbols(&self, a: &Analysis, file: FileId) -> Reply {
        json(DocumentSymbolResponse::Nested(outline::document_symbols(
            a, file,
        )))
    }

    fn formatting(&self, a: &Analysis, file: FileId) -> Reply {
        match outline::format_edits(a, file) {
            Some(edits) => json(edits),
            None => Ok(serde_json::Value::Null),
        }
    }

    fn tokens(&self, a: &Analysis, file: FileId) -> Reply {
        json(SemanticTokensResult::Tokens(SemanticTokens {
            result_id: None,
            data: outline::semantic_tokens(a, file),
        }))
    }

    fn prepare_rename(&self, a: &Analysis, file: FileId, offset: usize) -> Reply {
        match nav::prepare_rename(a, file, to_u32(offset)) {
            Ok((range, placeholder)) => {
                json(PrepareRenameResponse::RangeWithPlaceholder { range, placeholder })
            }
            Err(e) => Err((REQUEST_FAILED, e.to_string())),
        }
    }

    fn rename(&self, req: Request) -> Reply {
        let p: lsp_types::RenameParams = params(req.params)?;
        let uri = &p.text_document_position.text_document.uri;
        let Some((a, file)) = self.locate(uri) else {
            return Ok(serde_json::Value::Null);
        };
        let offset = a
            .lines(file)
            .map_or(0, |l| l.offset(p.text_document_position.position));
        match nav::rename(a, file, to_u32(offset), &p.new_name) {
            Ok(edit) => json(edit),
            Err(e) => Err((REQUEST_FAILED, e.to_string())),
        }
    }
}

type Reply = Result<serde_json::Value, (i32, String)>;

fn to_u32(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

fn json(v: impl serde::Serialize) -> Reply {
    serde_json::to_value(v).map_err(|e| (REQUEST_FAILED, e.to_string()))
}

fn parse<T: DeserializeOwned>(v: serde_json::Value) -> Option<T> {
    serde_json::from_value(v).ok()
}

fn params<T: DeserializeOwned>(v: serde_json::Value) -> Result<T, (i32, String)> {
    serde_json::from_value(v).map_err(|e| (INVALID_PARAMS, e.to_string()))
}

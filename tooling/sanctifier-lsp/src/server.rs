//! Request dispatch: the LSP methods this server implements.

use std::collections::HashMap;
use std::io::{BufRead, Write};

use serde_json::{json, Value};

use crate::analysis::{self, Finding};
use crate::protocol::{read_frame, write_frame, FrameError};

/// Open documents, keyed by URI. The client is the source of truth for
/// content between saves, so text is held here rather than re-read from disk —
/// analysing the file on disk would report diagnostics for code the developer
/// has already changed.
#[derive(Default)]
pub struct Server {
    documents: HashMap<String, String>,
    shutdown_requested: bool,
}

impl Server {
    pub fn new() -> Self {
        Self::default()
    }

    /// Capabilities advertised at `initialize`.
    ///
    /// Text sync is Full rather than Incremental: the analyzer takes whole
    /// source text, so applying incremental edits here would only mean
    /// reassembling the document to hand it over intact. Full sync is more
    /// data on the wire and strictly less code that can be wrong.
    fn capabilities() -> Value {
        json!({
            "textDocumentSync": { "openClose": true, "change": 1, "save": { "includeText": true } },
            "hoverProvider": true,
            "diagnosticProvider": { "interFileDependencies": false, "workspaceDiagnostics": false }
        })
    }

    /// Handle one decoded message.
    ///
    /// Returns the response to send, or `None` for notifications — which have
    /// no `id` and must never be answered. Replying to one is a protocol
    /// violation some clients treat as fatal.
    pub fn handle(&mut self, message: &Value) -> Option<Value> {
        let method = message.get("method")?.as_str()?;
        let id = message.get("id").cloned();
        let params = message.get("params").cloned().unwrap_or(Value::Null);

        match method {
            "initialize" => Some(ok(
                id?,
                json!({
                    "capabilities": Self::capabilities(),
                    "serverInfo": { "name": "sanctifier-lsp", "version": env!("CARGO_PKG_VERSION") }
                }),
            )),

            "initialized" => None,

            "textDocument/didOpen" | "textDocument/didChange" | "textDocument/didSave" => {
                self.sync_document(method, &params);
                None
            }

            "textDocument/didClose" => {
                if let Some(uri) = uri_of(&params) {
                    self.documents.remove(&uri);
                }
                None
            }

            "textDocument/hover" => {
                let id = id?;
                Some(ok(id, self.hover(&params)))
            }

            "shutdown" => {
                self.shutdown_requested = true;
                Some(ok(id?, Value::Null))
            }

            // Unknown request: answer with MethodNotFound rather than staying
            // silent, or the client blocks waiting for a reply that never comes.
            _ => id.map(|id| {
                json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": { "code": -32601, "message": format!("method not found: {method}") }
                })
            }),
        }
    }

    pub fn shutdown_requested(&self) -> bool {
        self.shutdown_requested
    }

    fn sync_document(&mut self, method: &str, params: &Value) {
        let Some(uri) = uri_of(params) else { return };

        let text = match method {
            "textDocument/didOpen" => params
                .pointer("/textDocument/text")
                .and_then(Value::as_str)
                .map(str::to_string),
            // Full sync, so the first content change carries the whole document.
            "textDocument/didChange" => params
                .pointer("/contentChanges/0/text")
                .and_then(Value::as_str)
                .map(str::to_string),
            // didSave only carries text when the client honoured includeText;
            // otherwise the last synced content is still current.
            _ => params
                .get("text")
                .and_then(Value::as_str)
                .map(str::to_string)
                .or_else(|| self.documents.get(&uri).cloned()),
        };

        if let Some(text) = text {
            self.documents.insert(uri, text);
        }
    }

    /// Diagnostics to publish for a document, if it is open.
    pub fn diagnostics_for(&self, uri: &str) -> Option<Value> {
        let source = self.documents.get(uri)?;
        let findings = analysis::analyze(source);
        let diagnostics: Vec<Value> = findings
            .iter()
            .map(|f| {
                serde_json::to_value(analysis::to_diagnostic(f, source)).unwrap_or(Value::Null)
            })
            .collect();

        Some(json!({
            "jsonrpc": "2.0",
            "method": "textDocument/publishDiagnostics",
            "params": { "uri": uri, "diagnostics": diagnostics }
        }))
    }

    /// URIs the server currently has content for.
    pub fn open_uris(&self) -> Vec<String> {
        self.documents.keys().cloned().collect()
    }

    fn hover(&self, params: &Value) -> Value {
        let Some(uri) = uri_of(params) else {
            return Value::Null;
        };
        let Some(source) = self.documents.get(&uri) else {
            return Value::Null;
        };
        let line = params
            .pointer("/position/line")
            .and_then(Value::as_u64)
            .unwrap_or(0) as u32;

        let findings: Vec<Finding> = analysis::analyze(source);
        match analysis::hover_markdown(&findings, line) {
            Some(markdown) => json!({
                "contents": { "kind": "markdown", "value": markdown }
            }),
            None => Value::Null,
        }
    }
}

fn uri_of(params: &Value) -> Option<String> {
    params
        .pointer("/textDocument/uri")
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn ok(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

/// Run the server loop over the given streams until the client closes stdin.
///
/// Diagnostics are published after every document sync, which is what makes
/// findings appear on save without the editor having to ask.
pub fn serve<R: BufRead, W: Write>(input: &mut R, output: &mut W) -> std::io::Result<()> {
    let mut server = Server::new();

    loop {
        let body = match read_frame(input) {
            Ok(body) => body,
            Err(FrameError::Eof) => return Ok(()),
            Err(FrameError::Io(e)) => return Err(e),
            Err(FrameError::Protocol(reason)) => {
                // A malformed frame desynchronises the stream — there is no
                // safe place to resume from, so report and stop rather than
                // emitting garbage responses against the wrong ids.
                eprintln!("sanctifier-lsp: protocol error: {reason}");
                return Ok(());
            }
        };

        let Ok(message) = serde_json::from_str::<Value>(&body) else {
            eprintln!("sanctifier-lsp: ignoring unparseable message");
            continue;
        };

        let is_sync = message
            .get("method")
            .and_then(Value::as_str)
            .is_some_and(|m| m.starts_with("textDocument/did") && m != "textDocument/didClose");

        if let Some(response) = server.handle(&message) {
            write_frame(output, &response.to_string())?;
        }

        if is_sync {
            if let Some(uri) = message
                .pointer("/params/textDocument/uri")
                .and_then(Value::as_str)
            {
                if let Some(notification) = server.diagnostics_for(uri) {
                    write_frame(output, &notification.to_string())?;
                }
            }
        }

        if message.get("method").and_then(Value::as_str) == Some("exit") {
            return Ok(());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    const SOURCE: &str = "use soroban_sdk::Env;\npub fn f(env: Env) { let _x: i128 = env.storage().persistent().get(&1u32).unwrap(); }\n";

    fn frame(body: &str) -> String {
        format!("Content-Length: {}\r\n\r\n{}", body.len(), body)
    }

    fn did_open(uri: &str, text: &str) -> Value {
        json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": { "textDocument": { "uri": uri, "languageId": "rust", "version": 1, "text": text } }
        })
    }

    #[test]
    fn initialize_advertises_hover_and_text_sync() {
        let mut server = Server::new();
        let response = server
            .handle(&json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {} }))
            .expect("initialize is a request and must be answered");

        assert_eq!(response["id"], 1);
        assert_eq!(response["result"]["capabilities"]["hoverProvider"], true);
        assert!(response["result"]["capabilities"]["textDocumentSync"].is_object());
        assert_eq!(response["result"]["serverInfo"]["name"], "sanctifier-lsp");
    }

    #[test]
    fn notifications_are_never_answered() {
        // Replying to a notification is a protocol violation; some clients
        // treat the unexpected response as fatal.
        let mut server = Server::new();
        assert!(server
            .handle(&json!({ "jsonrpc": "2.0", "method": "initialized", "params": {} }))
            .is_none());
        assert!(server.handle(&did_open("file:///a.rs", SOURCE)).is_none());
    }

    #[test]
    fn unknown_requests_get_method_not_found_rather_than_silence() {
        // Staying silent leaves the client blocked on a reply forever.
        let mut server = Server::new();
        let response = server
            .handle(&json!({ "jsonrpc": "2.0", "id": 7, "method": "textDocument/codeLens" }))
            .expect("a request must always be answered");
        assert_eq!(response["error"]["code"], -32601);
    }

    #[test]
    fn unknown_notifications_stay_silent() {
        let mut server = Server::new();
        assert!(server
            .handle(&json!({ "jsonrpc": "2.0", "method": "$/setTrace", "params": {} }))
            .is_none());
    }

    #[test]
    fn opening_a_document_makes_diagnostics_available() {
        let mut server = Server::new();
        server.handle(&did_open("file:///vault.rs", SOURCE));

        let notification = server
            .diagnostics_for("file:///vault.rs")
            .expect("an open document should produce a diagnostics notification");

        assert_eq!(notification["method"], "textDocument/publishDiagnostics");
        assert_eq!(notification["params"]["uri"], "file:///vault.rs");
        let diagnostics = notification["params"]["diagnostics"].as_array().unwrap();
        assert!(!diagnostics.is_empty(), "expected at least one diagnostic");
        assert_eq!(diagnostics[0]["source"], "sanctifier");
    }

    #[test]
    fn an_unopened_document_yields_no_diagnostics() {
        let server = Server::new();
        assert!(server.diagnostics_for("file:///never-opened.rs").is_none());
    }

    #[test]
    fn changes_replace_the_document_content() {
        let mut server = Server::new();
        server.handle(&did_open("file:///a.rs", SOURCE));
        server.handle(&json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didChange",
            "params": {
                "textDocument": { "uri": "file:///a.rs", "version": 2 },
                "contentChanges": [ { "text": "// clean\n" } ]
            }
        }));

        let notification = server.diagnostics_for("file:///a.rs").unwrap();
        let diagnostics = notification["params"]["diagnostics"].as_array().unwrap();
        assert!(
            diagnostics.is_empty(),
            "diagnostics should clear once the offending code is gone, got {diagnostics:?}"
        );
    }

    #[test]
    fn save_without_included_text_keeps_the_last_synced_content() {
        // Clients may honour includeText or not; dropping the document on a
        // bare save would silently stop reporting for that file.
        let mut server = Server::new();
        server.handle(&did_open("file:///a.rs", SOURCE));
        server.handle(&json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didSave",
            "params": { "textDocument": { "uri": "file:///a.rs" } }
        }));

        assert!(server.diagnostics_for("file:///a.rs").is_some());
    }

    #[test]
    fn closing_a_document_forgets_it() {
        let mut server = Server::new();
        server.handle(&did_open("file:///a.rs", SOURCE));
        server.handle(&json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didClose",
            "params": { "textDocument": { "uri": "file:///a.rs" } }
        }));

        assert!(server.open_uris().is_empty());
        assert!(server.diagnostics_for("file:///a.rs").is_none());
    }

    #[test]
    fn hover_on_a_line_without_findings_returns_null() {
        let mut server = Server::new();
        server.handle(&did_open("file:///a.rs", SOURCE));

        let response = server
            .handle(&json!({
                "jsonrpc": "2.0",
                "id": 3,
                "method": "textDocument/hover",
                "params": { "textDocument": { "uri": "file:///a.rs" }, "position": { "line": 0, "character": 0 } }
            }))
            .unwrap();

        assert_eq!(response["result"], Value::Null);
    }

    #[test]
    fn hover_on_a_finding_returns_markdown() {
        let mut server = Server::new();
        server.handle(&did_open("file:///a.rs", SOURCE));

        let response = server
            .handle(&json!({
                "jsonrpc": "2.0",
                "id": 4,
                "method": "textDocument/hover",
                "params": { "textDocument": { "uri": "file:///a.rs" }, "position": { "line": 1, "character": 10 } }
            }))
            .unwrap();

        assert_eq!(response["result"]["contents"]["kind"], "markdown");
        let value = response["result"]["contents"]["value"].as_str().unwrap();
        assert!(!value.is_empty());
    }

    #[test]
    fn hover_on_an_unopened_document_returns_null_rather_than_erroring() {
        let mut server = Server::new();
        let response = server
            .handle(&json!({
                "jsonrpc": "2.0",
                "id": 5,
                "method": "textDocument/hover",
                "params": { "textDocument": { "uri": "file:///gone.rs" }, "position": { "line": 0, "character": 0 } }
            }))
            .unwrap();
        assert_eq!(response["result"], Value::Null);
    }

    #[test]
    fn shutdown_is_acknowledged_and_recorded() {
        let mut server = Server::new();
        let response = server
            .handle(&json!({ "jsonrpc": "2.0", "id": 9, "method": "shutdown" }))
            .unwrap();
        assert_eq!(response["result"], Value::Null);
        assert!(server.shutdown_requested());
    }

    #[test]
    fn a_full_session_over_stdio_publishes_diagnostics_then_exits() {
        let session = [
            frame(
                &json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {} })
                    .to_string(),
            ),
            frame(&json!({ "jsonrpc": "2.0", "method": "initialized", "params": {} }).to_string()),
            frame(&did_open("file:///vault.rs", SOURCE).to_string()),
            frame(&json!({ "jsonrpc": "2.0", "id": 2, "method": "shutdown" }).to_string()),
            frame(&json!({ "jsonrpc": "2.0", "method": "exit" }).to_string()),
        ]
        .concat();

        let mut input = Cursor::new(session.into_bytes());
        let mut output = Vec::new();
        serve(&mut input, &mut output).expect("session should complete cleanly");

        let out = String::from_utf8(output).unwrap();
        assert!(
            out.contains("\"capabilities\""),
            "initialize was not answered"
        );
        assert!(
            out.contains("textDocument/publishDiagnostics"),
            "diagnostics were never published"
        );
        assert!(out.contains("\"source\":\"sanctifier\""));
    }

    #[test]
    fn an_unparseable_message_does_not_kill_the_session() {
        let session = [
            frame("{ this is not json"),
            frame(
                &json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {} })
                    .to_string(),
            ),
        ]
        .concat();

        let mut input = Cursor::new(session.into_bytes());
        let mut output = Vec::new();
        serve(&mut input, &mut output).unwrap();

        assert!(
            String::from_utf8(output)
                .unwrap()
                .contains("\"capabilities\""),
            "the server should have recovered and answered the next message"
        );
    }
}

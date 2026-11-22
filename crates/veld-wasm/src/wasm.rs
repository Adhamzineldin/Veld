//! The JavaScript surface: every function takes and returns JSON text, so the module needs no
//! serde bridge. `website/src/wasm/veld.ts` wraps these with typed `JSON.parse` helpers.
#![allow(unsafe_code)]

use serde::Serialize;
use wasm_bindgen::prelude::wasm_bindgen;

use crate::api;

fn json<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value).unwrap_or_else(|e| {
        format!(
            "{{\"ok\":false,\"error\":{}}}",
            serde_json::Value::from(e.to_string())
        )
    })
}

/// `compile(filesJson, projectJson?)` returns a `CompileResult` as JSON.
#[wasm_bindgen(js_name = compile)]
pub fn compile(files: &str, project: Option<String>) -> String {
    json(&api::compile_json(files, project.as_deref()))
}

/// `generate(filesJson, targetJson, projectJson?)` returns a `GenerateResult` as JSON.
#[wasm_bindgen(js_name = generate)]
pub fn generate(files: &str, target: &str, project: Option<String>) -> String {
    json(&api::generate_json(files, target, project.as_deref()))
}

/// `format(text)` returns a `FormatResult` as JSON.
#[wasm_bindgen(js_name = format)]
pub fn format(text: &str) -> String {
    json(&api::format(text))
}

/// `exportOpenapi(filesJson, projectJson?)` returns an `OpenapiResult` as JSON.
#[wasm_bindgen(js_name = exportOpenapi)]
pub fn export_openapi(files: &str, project: Option<String>) -> String {
    json(&api::export_openapi_json(files, project.as_deref()))
}

/// `describe()` lists the generators with their roles, frameworks and the engine version.
#[wasm_bindgen(js_name = describe)]
pub fn describe() -> String {
    json(&api::describe())
}

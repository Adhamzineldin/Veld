//! `tests/wire/streaming/sse.json` through the generated runtime's frame writer and `SseParser`.

use harness::check::{equal, that};
use live_client::veld::stream::{SseParser, sse_frame};
use serde_json::Value;

fn main() {
    let path = std::env::var("WIRE_DIR").map_or_else(|_| "tests/wire".into(), |d| d) + "/streaming/sse.json";
    let doc: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))).unwrap_or_default();
    let mut checked = 0;
    for f in doc["frames"].as_array().cloned().unwrap_or_default() {
        let name = f["name"].as_str().unwrap_or("?");
        let opts = &f["options"];
        let frame = sse_frame(
            f["data"].as_str().unwrap_or_default(),
            opts["id"].as_str(),
            opts["event"].as_str(),
            opts["retry"].as_u64(),
        )
        .unwrap_or_else(|e| panic!("{name}: {e}"));
        equal(f["frame"].as_str().unwrap_or_default(), frame.as_str(), &format!("frame: {name}"));
        checked += 1;
    }
    for p in doc["parse"].as_array().cloned().unwrap_or_default() {
        let name = p["name"].as_str().unwrap_or("?");
        let mut parser = SseParser::new();
        let mut events = Vec::new();
        for chunk in p["chunks"].as_array().cloned().unwrap_or_default() {
            events.extend(parser.push(chunk.as_str().unwrap_or_default().as_bytes()));
        }
        let want = p["events"].as_array().cloned().unwrap_or_default();
        equal(want.len(), events.len(), &format!("parse: {name}: event count"));
        for (w, e) in want.iter().zip(&events) {
            equal(w["event"].as_str().unwrap_or_default(), e.event.as_str(), &format!("parse: {name}: event"));
            equal(w["data"].as_str().unwrap_or_default(), e.data.as_str(), &format!("parse: {name}: data"));
            equal(w["id"].as_str(), e.id.as_deref(), &format!("parse: {name}: id"));
        }
        checked += 1;
    }
    that(checked > 0, "no cases");
    println!("sse wire ok ({checked} cases)");
}

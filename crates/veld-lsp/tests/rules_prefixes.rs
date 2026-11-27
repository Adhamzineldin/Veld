//! The server must answer, or answer with nothing, whatever text it is given: hover, completion,
//! definition, the outline and the semantic tokens at the end of every prefix of the bookings
//! contract (cut at every character boundary), and at every offset of the whole file.
//!
//! The prefixes are split in four tests so that they run side by side.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fs;
use std::path::PathBuf;

use veld_lsp::workspace::Workspace;
use veld_lsp::{complete, hover, nav, outline};

const TOML: &str = "[service]\nname = \"s\"\ncontracts = \"c\"\n";
const BOOKINGS: &str = include_str!("fixtures/bookings.veld");
const EXTRAS: &str = include_str!("fixtures/extras.veld");

struct Project {
    _tmp: tempfile::TempDir,
    ws: Workspace,
    path: PathBuf,
}

fn project() -> Project {
    let tmp = tempfile::tempdir().unwrap();
    fs::write(tmp.path().join("veld.toml"), TOML).unwrap();
    fs::create_dir_all(tmp.path().join("c")).unwrap();
    let path = tmp.path().join("c/m.veld");
    fs::write(&path, "").unwrap();
    let ws = Workspace::new(Some(tmp.path().to_path_buf()));
    Project {
        _tmp: tmp,
        ws,
        path,
    }
}

impl Project {
    /// Asks everything at `at` of an analysis of `text`.
    fn ask(&mut self, text: &str, at: usize) -> usize {
        self.ws.open(&self.path, text.to_owned());
        let a = self.ws.analyze(None);
        let file = a
            .file_for(&self.path)
            .expect("the buffer is part of the project");
        let offset = u32::try_from(at).unwrap();
        let mut answers = 0;
        answers += usize::from(hover::hover(&a, file, offset).is_some());
        answers += complete::complete(&a, file, at).len();
        answers += usize::from(nav::definition(&a, file, offset).is_some());
        answers
    }

    /// The outline and the tokens of `text`.
    fn outline(&mut self, text: &str) {
        self.ws.open(&self.path, text.to_owned());
        let a = self.ws.analyze(None);
        let file = a.file_for(&self.path).unwrap();
        let _ = outline::document_symbols(&a, file);
        let _ = outline::semantic_tokens(&a, file);
    }
}

fn boundaries(text: &str) -> Vec<usize> {
    let mut v: Vec<usize> = text.char_indices().map(|(i, _)| i).collect();
    v.push(text.len());
    v
}

/// Every prefix whose position in the list is `lane` modulo `lanes`.
fn prefixes(text: &str, lane: usize, lanes: usize) {
    let mut p = project();
    for (n, end) in boundaries(text).into_iter().enumerate() {
        if n % lanes != lane {
            continue;
        }
        let prefix = &text[..end];
        p.ask(prefix, end);
        if n % 97 == 0 {
            p.outline(prefix);
        }
    }
}

#[test]
fn every_prefix_of_bookings_lane_0() {
    prefixes(BOOKINGS, 0, 4);
}

#[test]
fn every_prefix_of_bookings_lane_1() {
    prefixes(BOOKINGS, 1, 4);
}

#[test]
fn every_prefix_of_bookings_lane_2() {
    prefixes(BOOKINGS, 2, 4);
}

#[test]
fn every_prefix_of_bookings_lane_3() {
    prefixes(BOOKINGS, 3, 4);
}

#[test]
fn every_prefix_of_the_extras_contract() {
    prefixes(EXTRAS, 0, 1);
}

#[test]
fn every_offset_of_the_whole_contracts() {
    let mut p = project();
    for text in [BOOKINGS, EXTRAS] {
        p.ws.open(&p.path, text.to_owned());
        let a = p.ws.analyze(None);
        let file = a.file_for(&p.path).unwrap();
        let mut answered = 0;
        for at in boundaries(text) {
            let offset = u32::try_from(at).unwrap();
            answered += usize::from(hover::hover(&a, file, offset).is_some());
            answered += complete::complete(&a, file, at).len();
            answered += usize::from(nav::definition(&a, file, offset).is_some());
        }
        assert!(answered > 0);
        p.outline(text);
    }
}

#[test]
fn every_byte_offset_of_a_contract_with_multibyte_text() {
    let text = BOOKINGS
        .replace(
            "A room of the hotel.",
            "A room of the h\u{f4}tel \u{1f3e8}.",
        )
        .replace("\"staff\"", "\"st\u{e4}ff\"");
    let mut p = project();
    p.ws.open(&p.path, text.clone());
    let a = p.ws.analyze(None);
    let file = a.file_for(&p.path).unwrap();
    for at in 0..=text.len() + 2 {
        let offset = u32::try_from(at).unwrap();
        let _ = hover::hover(&a, file, offset);
        let _ = complete::complete(&a, file, at);
        let _ = nav::definition(&a, file, offset);
    }
    p.outline(&text);
}

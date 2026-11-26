//! Sagas in the runner (`spec/platform.md` section 18): a saga example runs offline through the
//! reference orchestrator, a live run skips it and says so, and the fuzzer leaves a saga alone.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;

use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_ir::{Ir, PackageKind};
use veld_rules::Verdict;
use veld_testing::{GenerateError, Live, generate, run_offline};

fn pending() -> Ir {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/rules/endpoints/sagas.veld.pending");
    let text = std::fs::read_to_string(path).unwrap();
    let out = compile(&CompileInput {
        packages: vec![PackageInput {
            name: "t".into(),
            kind: PackageKind::Service,
            files: vec![SourceFile {
                path: "t.veld".into(),
                text,
            }],
            base_url: None,
        }],
        consumes: BTreeMap::new(),
    });
    match out.ir {
        Some(ir) => ir,
        None => panic!("does not compile:\n{}", out.render()),
    }
}

/// A server that answers the probe of the test handler and counts what else it is asked.
fn probe_only() -> (String, std::thread::JoinHandle<usize>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let thread = std::thread::spawn(move || {
        listener.set_nonblocking(false).unwrap();
        let mut asked = 0;
        // The probe, then one more connection that the test makes only to stop this thread.
        for stream in listener.incoming().take(2) {
            let mut stream = stream.unwrap();
            let mut buf = [0u8; 4096];
            let n = stream.read(&mut buf).unwrap_or(0);
            let head = String::from_utf8_lossy(&buf[..n]).into_owned();
            if head.is_empty() {
                break;
            }
            asked += 1;
            let body = "{\"veldTestHandler\":1,\"digests\":{\"Checkout\":null}}";
            let reply = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(reply.as_bytes());
        }
        asked
    });
    (url, thread)
}

#[test]
fn every_saga_example_passes_offline() {
    let ir = pending();
    let runs = run_offline(&ir, None);
    assert_eq!(runs.len(), 11);
    for run in &runs {
        let report = run.result.as_ref().unwrap();
        assert!(report.passed(), "{report}");
    }
}

#[test]
fn a_live_run_skips_a_saga_example_and_sends_nothing() {
    let ir = pending();
    let (url, server) = probe_only();
    let live = Live::connect(&url, false).unwrap();
    for ex in &ir.examples {
        let report = live.run_example(&ir, ex).unwrap();
        let Verdict::Skipped(why) = &report.verdict else {
            panic!("not skipped: {report}");
        };
        assert!(why.contains("saga"), "{why}");
    }
    // Only the probe was ever asked: this connection ends the server.
    let _ = std::net::TcpStream::connect(url.trim_start_matches("http://"));
    assert_eq!(server.join().unwrap(), 1);
}

#[test]
fn the_fuzzer_leaves_a_saga_endpoint_alone() {
    let ir = pending();
    let service = ir.services.iter().find(|s| s.name == "Checkout").unwrap();
    for name in ["checkout", "review", "quick"] {
        let ep = service.endpoints.iter().find(|e| e.name == name).unwrap();
        match generate(&ir, service, ep, 1, 10) {
            Err(GenerateError::Skipped(why)) => assert!(why.contains("saga"), "{why}"),
            other => panic!("{name}: {other:?}"),
        }
    }
}

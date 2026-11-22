//! A 2k-line workspace must re-analyse quickly. The product budget is 50 ms in release; the
//! assertion is generous so that debug builds and loaded CI machines do not flake.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fmt::Write as _;
use std::fs;
use std::time::Instant;

use veld_lsp::workspace::Workspace;

fn module(i: usize) -> String {
    let mut s = String::new();
    if i > 0 {
        let _ = writeln!(s, "use m{}.{{T{}_0}}\n", i - 1, i - 1);
    }
    for t in 0..14 {
        let _ = writeln!(s, "/// Model {i}.{t}.\nmodel T{i}_{t} {{");
        for f in 0..10 {
            let _ = writeln!(s, "  f{f}: string @length(max: {})", 10 + f);
        }
        if i > 0 && t == 0 {
            let _ = writeln!(s, "  prev: T{}_0", i - 1);
        }
        let _ = writeln!(s, "}}\n");
    }
    let _ = writeln!(
        s,
        "service S{i} at \"/s{i}\" {{\n  get \"/\" list{i}() -> T{i}_0[]\n}}"
    );
    s
}

#[test]
fn a_two_thousand_line_workspace_analyses_within_budget() {
    let tmp = tempfile::tempdir().unwrap();
    fs::write(tmp.path().join("veld.toml"), "[project]\nname = \"big\"\n").unwrap();
    fs::create_dir_all(tmp.path().join("contracts")).unwrap();
    let mut lines = 0;
    for i in 0..10 {
        let text = module(i);
        lines += text.lines().count();
        fs::write(tmp.path().join(format!("contracts/m{i}.veld")), text).unwrap();
    }
    assert!(lines >= 2000, "{lines} lines");
    let ws = Workspace::new(Some(tmp.path().to_path_buf()));
    let _warm = ws.analyze(None);
    let start = Instant::now();
    let a = ws.analyze(None);
    let elapsed = start.elapsed();
    assert_eq!(
        a.index.error_count(),
        0,
        "{}",
        a.index.sources.render_all(&a.index.diagnostics)
    );
    assert!(
        elapsed.as_millis() < 200,
        "analysis took {elapsed:?} for {lines} lines"
    );
}

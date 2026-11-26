//! The examples of `tests/rules/endpoints/derived.veld.pending` run through `derived-serve`
//! (see `crates/emitters/rust/tests/derived.rs`); this program only checks that the generated
//! test handler of a contract with derived fields answers its probe. Prints `derived harness ok`.

fn main() {
    let probe = derived_server::derived::shop_test_server::probe();
    assert!(probe.contains("\"veldTestHandler\":1"), "{probe}");
    println!("derived harness ok");
}

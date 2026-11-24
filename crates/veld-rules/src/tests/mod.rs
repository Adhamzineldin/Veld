mod behaviour;
mod helpers;
mod kit;
mod models;
mod vector_files;

#[test]
fn the_crate_links_against_the_ir_it_evaluates() {
    assert_eq!(crate::IR_VERSION, "1.1");
    let rules: Vec<veld_ir::Rule> = serde_json::from_str("[]").unwrap();
    assert!(rules.is_empty());
}

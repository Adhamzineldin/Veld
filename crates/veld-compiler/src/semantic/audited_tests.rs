//! `@audited` (`spec/platform.md` section 4): where it stands and what it lowers to.

use crate::testing::{codes, ir as build};
use veld_ir::{Endpoint, Feature, Ir};

const ROOM: &str = "model Room { id: uuid }\n";

fn find<'a>(ir: &'a Ir, name: &str) -> &'a Endpoint {
    ir.services[0]
        .endpoints
        .iter()
        .find(|e| e.name == name)
        .unwrap()
}

#[test]
fn an_endpoint_and_a_service_say_it_and_the_ir_has_it() {
    let src = format!(
        "{ROOM}service Rooms at \"/r\" {{\n  @audited\n  get \"/{{id}}\" one(id: uuid) -> Room\n  get \"/\" all() -> Room[]\n}}"
    );
    let ir = build(&src);
    assert!(find(&ir, "one").audited && !find(&ir, "all").audited);
    let used = ir.features_used(&[ir.services[0].id]);
    assert!(used.contains(&Feature::Audit), "{used:?}");
    let all = format!(
        "{ROOM}@audited\nservice Rooms at \"/r\" {{ get \"/\" all() -> Room[]\n get \"/{{id}}\" one(id: uuid) -> Room }}"
    );
    let ir = build(&all);
    assert!(find(&ir, "one").audited && find(&ir, "all").audited);
}

#[test]
fn a_service_default_skips_an_endpoint_that_streams_and_its_own_is_kept() {
    let src = format!(
        "{ROOM}@audited\nservice Rooms at \"/r\" {{\n  get \"/ticks\" ticks() -> stream Room\n  ws \"/w\" chat(body: stream Room) -> stream Room\n  get \"/\" all() -> Room[]\n}}"
    );
    let ir = build(&src);
    assert!(!find(&ir, "ticks").audited && !find(&ir, "chat").audited);
    assert!(find(&ir, "all").audited);
    let own = format!(
        "{ROOM}service Rooms at \"/r\" {{\n  @audited\n  get \"/ticks\" ticks() -> stream Room\n}}"
    );
    let ir = build(&own);
    assert!(
        find(&ir, "ticks").audited,
        "a generator refuses it with E0801"
    );
}

#[test]
fn it_takes_no_argument_and_stands_on_a_service_or_an_endpoint_only() {
    let with_arg =
        format!("{ROOM}service Rooms at \"/r\" {{ @audited(true)\n get \"/\" all() -> Room[] }}");
    assert_eq!(codes(&with_arg), vec!["E0602"]);
    let twice = format!(
        "{ROOM}service Rooms at \"/r\" {{ @audited @audited\n get \"/\" all() -> Room[] }}"
    );
    assert_eq!(codes(&twice), vec!["E0602"]);
    assert_eq!(codes("@audited\nmodel M { a: int }\n"), vec!["E0602"]);
    assert_eq!(codes("model M { a: int @audited }\n"), vec!["E0602"]);
}

#[test]
fn a_contract_without_it_keeps_its_json() {
    let ir = build(&format!(
        "{ROOM}service Rooms at \"/r\" {{ get \"/\" all() -> Room[] }}"
    ));
    assert!(!serde_json::to_string(&ir).unwrap().contains("audited"));
    assert!(
        !ir.features_used(&[ir.services[0].id])
            .contains(&Feature::Audit)
    );
}

#[test]
fn audited_stays_an_ordinary_name() {
    assert!(
        crate::testing::one("model M { audited: bool }\n")
            .ir
            .is_some()
    );
}

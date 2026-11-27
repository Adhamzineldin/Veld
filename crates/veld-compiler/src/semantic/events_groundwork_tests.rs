//! What the event generators need from the IR: the origin of every `publishes` line, the events
//! of a service and the declarations they reach.

use crate::testing::ir;

const HELPERS: &str = r#"model Money { cents: int }
model Zone { code: string }
model Note { text: string }
enum Kind { a, b }
union Pick { z: Zone, n: Note }
event Paid {
  id: uuid
  price: Money
  pick: Pick
  kind: Kind
}
event Left { id: uuid }
event Other { id: uuid }
channel Pays { carries Paid, Left
  key id }
channel Others { carries Other
  key id }
service Shop at "/shop" {
  publishes Pays, Others
  get "/n" n() -> Note
}
service Mail {
  subscribes Pays as "mail" { on Paid -> h }
}
"#;

fn names(ir: &veld_ir::Ir, ids: &[veld_ir::DeclId]) -> Vec<String> {
    ids.iter().map(|i| ir.decl(*i).name.clone()).collect()
}

fn service_id(ir: &veld_ir::Ir, name: &str) -> veld_ir::ServiceId {
    ir.services.iter().find(|s| s.name == name).unwrap().id
}

#[test]
fn every_publishes_channel_has_the_origin_of_its_name() {
    let ir = ir(HELPERS);
    let shop = &ir.services[service_id(&ir, "Shop").index()];
    let at: Vec<(u32, u32)> = shop
        .publishes
        .iter()
        .map(|p| (p.origin.line, p.origin.col))
        .collect();
    assert_eq!(at, vec![(19, 13), (19, 19)]);
    assert_eq!(shop.first_event_origin().map(|o| o.line), Some(19));
    let mail = &ir.services[service_id(&ir, "Mail").index()];
    assert_eq!(mail.first_event_origin().map(|o| o.line), Some(23));
}

#[test]
fn the_events_of_a_service_are_the_events_of_its_channels_without_duplicates() {
    let ir = ir(HELPERS);
    let shop = service_id(&ir, "Shop");
    let mail = service_id(&ir, "Mail");
    assert_eq!(
        names(&ir, &ir.events_of(&[shop])),
        ["Left", "Other", "Paid"]
    );
    // `Mail` handles only `Paid`, but its channel carries `Left` too.
    assert_eq!(names(&ir, &ir.events_of(&[mail])), ["Left", "Paid"]);
    assert_eq!(
        names(&ir, &ir.events_of(&[shop, mail])),
        ["Left", "Other", "Paid"]
    );
    assert!(ir.events_of(&[]).is_empty());
}

#[test]
fn the_declarations_events_reach_are_listed_and_the_event_only_ones_are_marked() {
    let ir = ir(HELPERS);
    let shop = service_id(&ir, "Shop");
    let reached = ir.event_reachable_decls(&[shop]);
    assert_eq!(
        names(&ir, &reached),
        ["Kind", "Money", "Note", "Pick", "Zone"]
    );
    // `Note` is also the response of an endpoint, the others exist only because of events.
    let only = ir.event_only_decls(&[shop]);
    assert_eq!(names(&ir, &only), ["Kind", "Money", "Pick", "Zone"]);
    assert!(
        !ir.reachable_decls(&[shop])
            .iter()
            .any(|i| ir.decl(*i).is_event())
    );
}

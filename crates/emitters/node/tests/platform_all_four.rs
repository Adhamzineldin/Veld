//! A contract that uses timed moves, flags, a deadline and a service level together
//! (`spec/targets/typescript.md` section 5j): the regions of the four features pair up, the
//! generated server and client compile under `tsc --strict` in both editions, and the files are
//! snapshotted.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use common::toolchain::*;
use common::*;
use serde_json::json;

const CONTRACT: &str = "tests/toolchain/contract/platform_p2/all_four.veld";

#[test]
fn the_four_features_compile_together_in_both_editions() {
    let Some(tsc) = tsc_path() else { return };
    let ir = compile_path(CONTRACT);
    for (edition, _) in TARGETS {
        let guard = scratch("p2-all-");
        build_service(guard.path(), &tsc, &ir, "Rooms", &[], edition);
    }
}

#[test]
fn snapshots_of_a_contract_with_the_four_features() {
    let ir = compile_path(CONTRACT);
    let server = gen_server_with(&ir, "Rooms", &[], &json!({ "testing": true }));
    insta::assert_snapshot!(
        "p2_all_server_file_list",
        server.paths().collect::<Vec<_>>().join("\n")
    );
    insta::assert_snapshot!("p2_all_server_models", text(&server, "models/bookings.ts"));
    insta::assert_snapshot!("p2_all_server_module", text(&server, "server/rooms.ts"));
    insta::assert_snapshot!(
        "p2_all_test_handler",
        text(&server, "server/rooms-test-handler.ts")
    );
    let client = gen_client(&ir);
    insta::assert_snapshot!("p2_all_client", text(&client, "bookings/client.ts"));
    insta::assert_snapshot!(
        "p2_all_client_models",
        text(&client, "bookings/models/bookings.ts")
    );
}

#[test]
fn every_part_of_each_feature_is_there() {
    let ir = compile_path(CONTRACT);
    let server = gen_server(&ir, "Rooms", &[]);
    let module = text(&server, "server/rooms.ts");
    for word in [
        "export interface RoomsDueLoaders",
        "export function runRoomsDueMoves",
        "flags: veld.FlagProvider;",
        "metrics?: (m: veld.Metric) => void | Promise<void>;",
        "timeoutMs: 2000,",
        "flagged: { live: 'archiving' },",
        "afterMs: 3600000n,",
    ] {
        assert!(module.contains(word), "`{word}` is missing in:\n{module}");
    }
    let client = gen_client(&ir);
    let models = text(&client, "bookings/models/bookings.ts");
    assert!(models.contains("timedMoves()"));
    assert!(models.contains("invariantOn('smallGroups')"));
    assert!(text(&client, "bookings/client.ts").contains("timeoutMs: 2000,"));
}

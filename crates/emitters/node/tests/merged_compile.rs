//! What two branches printed side by side must compile: the test handler of a service that has
//! endpoint rules *and* a subscription (`testing = true`), the server entry of a server that
//! consumes another service and has events with `messaging = ["kafka", "rabbitmq"]`
//! (`spec/targets/typescript.md` sections 5c, 5f and 5h). Compiled with `tsc --strict` for both
//! editions; nothing is run.
//!
//! The toolchain test needs `node` and `tests/toolchain/ts/node_modules`; without them it is
//! skipped unless `VELD_REQUIRE_TOOLCHAINS=1`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use std::collections::BTreeMap;
use std::process::Command;

use common::toolchain::{run, scratch, tsc_path};
use common::*;
use serde_json::json;
use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_ir::{Ir, PackageKind};
use veld_project::output::{WriteOptions, write};

const IAM: &str = "
model User {
  id: uuid @readonly
  name: string
}

service Iam at \"/iam\" {
  get \"/users/{id}\" getUser(id: uuid) -> User | NotFound
}
";

const BOOKINGS: &str = "
use iam:users.{User}

model Room {
  id: uuid
  capacity: int @min(1)
  closed: bool
}

model NewBooking {
  roomId: uuid
  guests: int @min(1)
  ownerId: uuid
}

model Booking {
  id: uuid @readonly
  roomId: uuid
  guests: int
}

error RoomFull 409 { capacity: int }
error RoomClosed 409 {}

event BookingMade {
  bookingId: uuid
  guests: int
  rule atLeastOne: guests >= 1
}

event PaymentTaken {
  bookingId: uuid
  cents: int
}

@retry(max: 3, backoff: exponential)
@deadLetter
channel BookingEvents {
  carries BookingMade
  key bookingId
}

@retry(max: 2, backoff: fixed)
@deadLetter
channel PaymentEvents {
  carries PaymentTaken
  key bookingId
}

service Bookings at \"/bookings\" {
  publishes BookingEvents
  subscribes PaymentEvents as \"bookings\" {
    on PaymentTaken -> paid
  }

  post \"/\" createBooking(body: NewBooking) -> 201 Booking | RoomFull | RoomClosed {
    fact room: Room = body.roomId
    require not room.closed else RoomClosed
    require body.guests <= room.capacity else RoomFull { capacity: room.capacity }
  }
}

service Billing at \"/billing\" {
  publishes PaymentEvents
  subscribes BookingEvents as \"billing\" {
    on BookingMade -> charge
  }
  get \"/ping\" ping() -> 204
}
";

fn contract() -> Ir {
    let file = |path: &str, text: &str| SourceFile {
        path: path.into(),
        text: text.into(),
    };
    let out = compile(&CompileInput {
        packages: vec![
            PackageInput {
                name: "iam".into(),
                kind: PackageKind::Service,
                files: vec![file("users.veld", IAM)],
                base_url: Some("http://iam:3001".into()),
            },
            PackageInput {
                name: "bookings".into(),
                kind: PackageKind::Service,
                files: vec![file("bookings.veld", BOOKINGS)],
                base_url: None,
            },
        ],
        consumes: BTreeMap::from([("bookings".to_string(), vec!["iam".to_string()])]),
    });
    assert!(out.ir.is_some(), "{}", out.render());
    out.ir.unwrap_or_else(|| unreachable!())
}

#[test]
fn rules_events_a_consumed_service_testing_and_both_brokers_compile_strict() {
    let Some(tsc) = tsc_path() else {
        return;
    };
    let ir = contract();
    for (edition, target) in TARGETS {
        let guard = scratch("merged-compile-");
        let work = guard.path();
        let options = json!({
            "typescript": edition,
            "testing": true,
            "messaging": ["kafka", "rabbitmq"]
        });
        let files = gen_servers_with(&ir, &["Bookings", "Billing"], &["Iam"], &options);
        for path in [
            "server/bookings-test-handler.ts",
            "server/kafka.ts",
            "server/rabbitmq.ts",
            "runtime/events.ts",
        ] {
            assert!(files.get(path).is_some(), "{path} was not generated");
        }
        write(&work.join("server"), &files, &WriteOptions::default()).unwrap();
        std::fs::write(
            work.join("tsconfig.json"),
            serde_json::to_string_pretty(&json!({
                "compilerOptions": {
                    "strict": true, "target": target, "lib": [target, "dom"], "types": ["node"],
                    "module": "commonjs", "moduleResolution": "node10", "esModuleInterop": true,
                    "skipLibCheck": true, "noEmit": true, "forceConsistentCasingInFileNames": true
                },
                "include": ["server/**/*.ts"]
            }))
            .unwrap(),
        )
        .unwrap();
        run(
            Command::new("node")
                .arg(&tsc)
                .args(["-p", "tsconfig.json"])
                .current_dir(work),
            "tsc the merged server",
        );
    }
}

/// The same server without a broker adapter, compiled alone under the strictest flags the
/// generated packages are held to: no types packages, the language library only, nothing skipped.
#[test]
fn rules_events_and_a_consumed_service_compile_without_any_types_package() {
    let Some(tsc) = tsc_path() else {
        return;
    };
    let ir = contract();
    for (edition, target) in TARGETS {
        let guard = scratch("merged-compile-plain-");
        let work = guard.path();
        let options = json!({ "typescript": edition, "testing": true });
        let files = gen_servers_with(&ir, &["Bookings", "Billing"], &["Iam"], &options);
        write(&work.join("server"), &files, &WriteOptions::default()).unwrap();
        std::fs::write(
            work.join("tsconfig.json"),
            serde_json::to_string_pretty(&json!({
                "compilerOptions": {
                    "strict": true, "target": target, "lib": [target], "types": [],
                    "module": "commonjs", "moduleResolution": "node10", "noEmit": true,
                    "skipLibCheck": false, "forceConsistentCasingInFileNames": true
                },
                "include": ["server/**/*.ts"]
            }))
            .unwrap(),
        )
        .unwrap();
        run(
            Command::new("node")
                .arg(&tsc)
                .args(["-p", "tsconfig.json"])
                .current_dir(work),
            "tsc the merged server, strict",
        );
    }
}

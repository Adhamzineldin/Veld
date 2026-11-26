//! `veld export retention` (`spec/platform.md` section 23): the whole document, byte for byte.

use super::retention_report;
use crate::testutil::ir;

const SRC: &str = r#"
@retain(days(30), since: checkedOutAt, then: anonymize)
model Guest {
  id: uuid @readonly
  name: string @pii
  phone?: string @pii
  password: string @secret
  checkedOutAt?: datetime
}

@retain(days(1), since: at, then: delete)
model Session {
  token: string @secret
  at: datetime
}

model Note {
  text: string @pii
}

model Plain {
  id: uuid
}

error Leak 400 { who: string @pii }

event GuestRegistered {
  guest: Guest
}

channel GuestEvents { carries GuestRegistered }

@auth(bearer)
service Guests at "/guests" {
  get "/{id}" getGuest(id: uuid) -> Guest | Leak
  post "/notes" addNote(body: Plain) -> 201 Plain
}
"#;

const WANT: &str = "# Data inventory

The models that hold personal data (`@pii`) or secrets (`@secret`), how long each is kept, and where it travels. Written by `veld export retention`.

## Package `app`

### `Guest`

- Personal data: `name`, `phone`
- Secrets: `password`
- Kept: 30 days after `checkedOutAt`, then anonymized (`name`, `phone`)
- Travels:
  - `GET /guests/{id}` (`Guests.getGuest`)
  - the event `GuestRegistered`

### `Note`

- Personal data: `text`
- Kept: kept without limit
- Travels: nowhere: no endpoint or event can carry it

### `Session`

- Secrets: `token`
- Kept: 1 day after `at`, then deleted
- Travels: nowhere: no endpoint or event can carry it
";

#[test]
fn the_inventory_is_the_golden_document() {
    assert_eq!(retention_report(&ir(SRC)), WANT);
}

#[test]
fn the_document_is_deterministic() {
    assert_eq!(retention_report(&ir(SRC)), retention_report(&ir(SRC)));
}

#[test]
fn a_workspace_without_personal_data_says_so() {
    let text = retention_report(&ir(
        "model A { x: int }\nservice S at \"/s\" { get \"/\" f() -> A }\n",
    ));
    assert!(
        text.ends_with("No model holds a `@pii` or `@secret` field.\n"),
        "{text}"
    );
    assert!(text.starts_with("# Data inventory\n"));
}

#[test]
fn errors_are_not_models_and_are_not_listed() {
    assert!(!retention_report(&ir(SRC)).contains("Leak"));
}

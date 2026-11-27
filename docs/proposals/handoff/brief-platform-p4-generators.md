# Brief: platform phase P4 in the generators, and captures

Three agents get this brief, one per target (Node with TypeScript, Java, Rust). The message that started you says which is yours. Read `common.md` first (the law of this machine), then `CLAUDE.md`, then `brief-in-the-generators.md` (the three targets, the gates, how to work next to the other agents, the report), then this file.

## Goal

Four things that are done outside the generators and missing in yours, in this order, each as its own series of commits:

1. **No message repeats a value** (`spec/rules.md` 19.3, "Messages"): a tested rule of every runtime.
2. **Captures** (`spec/rules.md` 17.3 and 19.3): a server option, off by default, that adds a capture to the event of `onRuleViolation`, with `@pii` and `@secret` values left out. No feature to declare.
3. **Message catalogs** (`spec/platform.md` 22; feature `messages`, server and client).
4. **Retention** (`spec/platform.md` 23; feature `retention`, server only).

## Read first

- `spec/rules.md` 5.3 (the event of `onRuleViolation`), 17.3 (the JSON of a capture, written out in full with its rules: `params`, `facts`, `principal`, `now`, `redacted`), 19.3 (`@pii`, `@secret`); `spec/platform.md` 22 and 23 with what the compiler work added.
- The IR: `Field.pii`, `Field.secret`; `Package.messages` and `Package.messages_dir`; `Model.retain`; `Ir::retention_plan(&services)`; `veld_ir::message_keys` (`rule_key`, `forbid_key`, the fixed keys), `Ir::violation_key(rule_id, forbid)`, `Ir::catalog_keys(package)`.
- The reference: `veld_rules::redact_decl`, `redact_type` and their `_at` forms (goldens `tests/wire/redaction/`); `veld_rules::messages` (`accepted_languages`, `choose_language`, `localize_violation`, `localize_denial`, `content_language`; goldens `tests/wire/messages/`: a workspace with `ar` and `pt-br` catalogs, header rows, text rows, key rows); `veld_testing::from_violation` (what the tool does with a capture: your captures must round-trip through it).
- `crates/veld-emit-conformance/src/{messages,retention}.rs`.
- **The nearest things your emitter already does:** the event your runtime builds for `onRuleViolation` and the server options next to `validate` (for the capture option); how the runtime writes a violation into a response and into `Forbidden` (for `messageKey` and the translated text); the rules digest or any other per-package constant data your emitter embeds (for the catalogs and the retention plan).

## 1. No message repeats a value

Per runtime (server and client), one test that feeds a marker value (a string no message could contain by accident) through every path that produces a message: every validation issue code, every violation kind, every error the runtime itself raises (a config problem, a malformed header, a bad key), every log line generated code writes. It collects every message, `detail`, `title` and log text and searches for the marker. A message that repeats a value is changed to a fixed text with paths and names. List what you changed.

## 2. Captures

- A server option `captureViolations` (your target's casing), off by default, documented with the sentence of 17.3 that a capture contains whatever the caller sent.
- With it on, the event of `onRuleViolation` gains `capture` exactly as 17.3 writes it: `service`, `endpoint`, `operation`, `params` (wire form after binding, keyed by parameter name), `facts` (wire form, `null` for a fact the loader did not find), `principal` (wire form or `null`), `now` (RFC 3339), `redacted`. Every `@pii` and `@secret` value is left out at any depth, exactly as the reference `redact_*` functions do, and its wire path is in `redacted` (the path forms of 17.3: `params.<name>…`, `facts.<name>…`, `principal…`).
- Off, the event is byte for byte what it is today, and a server whose option is off pays nothing for it.
- Tests: the redaction goldens of `tests/wire/redaction/` through your runtime's redaction; a capture for an input violation, one for an output violation (`ensure`), one with a fact and a principal, one with redacted values; and **the round trip**: a test writes the captured event to a file and the Rust test calls `veld_testing::from_violation` on it and checks that the printed example compiles and has the expected outcome.

## 3. Message catalogs (`messages`)

- **Server:** a server target of a member with catalogs embeds them (the IR's `messages`, sorted). Per request it picks the language from `Accept-Language` as section 22 says (the reference's `choose_language` is the rule: reproduce its header rows), writes every message of the answer in it (the `message` of each violation, the `message` of a `Forbidden`), sets `Content-Language` when at least one message came from the catalog, and gives every violation `messageKey` (from the IR's key functions: never build a key from an id yourself; a refused transition and a refused version guard have fixed keys; a policy denial uses the forbid's key). A violation with an evaluation error carries `messageKey` and is not translated.
- **Without catalogs nothing changes**: no `messageKey`, no header, the same bytes.
- **Client:** a client target of a member with catalogs gets them as data (`messages.ar` in TypeScript; your target's natural form), and the violations of its local check have `messageKey`.
- The capability for both roles, and the conformance checks.
- Tests: every header row, text row and key row of `tests/wire/messages/` through the generated server (a request with that `Accept-Language` that violates that rule; the body's `message` and `messageKey` and the `Content-Language` header); the client's catalogs and keys; a contract without catalogs keeps its bytes.

## 4. Retention (`retention`)

- A server target with retained models gets `retentionPlan` (data, built from `Ir::retention_plan`: the model, `afterMs`, the `since` field, `then`, and for `anonymize` the wire paths of the `@pii` fields; the same entries in the same order in every target), the interface `RetentionJobs` (`delete(model, before)` and `anonymize(model, before, fields)`, each answering how many values it changed) and `run<Target>Retention(jobs, options)` (once per entry with `before = now - afterMs`; `now` from the options, default the system clock; it answers the counts; a failure of one entry does not stop the others and is in the report).
- The capability (server role) and the conformance check; a client generates nothing.
- Tests: the plan of a fixture against `Ir::retention_plan` (the Rust test compares the generated data with the function's output); the runner with recording jobs (the `before` of each entry for a fixed `now`, the order, a failing entry).

## Gates

As `brief-in-the-generators.md` says. A contract with none of this keeps its bytes; the capture option off keeps the event's bytes.

## Your report

As `brief-in-the-generators.md` says, one part per item, plus the messages you changed in part 1 and the generated names of parts 2 to 4.

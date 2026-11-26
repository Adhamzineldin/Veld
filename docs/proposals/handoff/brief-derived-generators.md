# Brief: derived fields in the generators

Three agents get this brief, one per target (Node with TypeScript, Java, Rust). The message that started you says which is yours. Read `common.md` first (the law of this machine), then `CLAUDE.md`, then `brief-in-the-generators.md` (the three targets, the gates, how to work next to the other agents, the report), then this file.

## Goal

A field whose `=` is followed by an expression is **derived** (`total: decimal = subtotal - discount`): nobody supplies its value, the server computes it. This is implemented outside the generators. Your generator refuses a contract with a derived field (`E0801`, feature `rules_derived`). Build it in your target, server and client.

## Read first

- `spec/rules.md` section **17.2**, all of it, including the paragraphs the compiler work added (where the id lives, the order, a failing value, facts, output mode `off`). Also 3 (expressions), 5 (invariants, the pipeline: steps 5 and 8), 9.7 (the rules digest), 12.2 (examples), 15 (the `"computed"` key of a vector case).
- `spec/ir.md`: `Field.derived` (an `Expr`, typed exactly like a rule: paths start at `self`, no `now`, no facts) and `Field.derived_id` (embed it verbatim; never compute an id).
- Shared helpers in `veld-ir`: `Decl::derived_fields()` (the order in which a value's derived fields are computed), `Field::derived_reads()`, `Decl::has_derived()`, `Ir::type_reaches_derived`, `Ir::fact_decls`.
- The reference: `veld_rules::with_defaults`, `check_value`, `Pipeline::run_reported`, `envelope::{encode, decode}`, `vectors::{run_case, value_at}`; its tests in `crates/veld-rules/tests/derived.rs` state cases the vectors cannot.
- The vectors: `tests/rules/core/derived.veld.pending` with `derived.json.pending` (22 cases; the new key `"computed"` maps a path to a wire value, `null` for no value) and `tests/rules/endpoints/derived.veld.pending` (9 examples).
- `crates/veld-emit-conformance/src/derived.rs`: `check_derived_is_refused`, `check_derived_is_generated`.
- **The nearest thing your emitter already does:** an invariant. Follow one from the IR to its generated check function (the expression printer, the helpers, where the server calls it in steps 5 and 8, where the client calls it before a send, `.partial`) and follow how defaults are applied to a value. A derived field is an expression printed by the same printer, evaluated where defaults are applied.

## What an implementation does

**The order, per value** (the reference's, and the vectors hold you to it):

1. drop what was supplied for a derived field (a request: what the caller sent, even when it is malformed; a response: what the handler put there);
2. apply the defaults of the other fields;
3. compute nested values first (list items, map values, page items, union arms);
4. compute the derived fields of the value in `Decl::derived_fields()` order;
5. then run the invariants.

**Where it runs:** on a request before step 5; on a fact after it is loaded and on the principal, before the policies; on a result, a declared error, an `else` payload, a stream item and a published event before the output rules; in a subscriber after decoding (a consumer recomputes and does not believe the message). Output mode `off` still computes.

**A failing expression** (an evaluation error): the field stays absent and there is one violation whose `rule` is the field's id and whose `error` is set: `422 ValidationError` in a request, the output mode in a response, `500 Internal` for a fact or the principal. A rule or a derived field that reads an absent derived field is skipped.

**The handler's types:** in the type a handler returns, a derived field is optional and whatever it holds is overwritten. In the type a handler receives it is present.

**The idempotency fingerprint** covers the computed values (it is taken from the parameters after step 5).

**The client:** takes a derived value from a response as it is and never recomputes it. Before a send, its local check computes the derived fields of the request value first, because rules may read them (the rules digest covers the ids of derived fields for that reason); it never sends them. A request type has no derived field for the caller to fill in.

**Every target, clients included,** gets one public function per declaration with derived fields that computes them for a value, so a form can show `total` while the user types. It is the code the server runs. Name it next to the check function of that declaration, in the same style.

**Gates:** as `brief-in-the-generators.md` says. A constraint on a derived field is not checked (the reference does not). A client is gated only for derived fields an endpoint reaches; one that only a fact or the principal has concerns servers only.

## Tests that must exist when you hand back (written and compiled, not run)

- The 22 core cases through your target's vector runner, comparing `expect`, the violations and `"computed"`; the 9 endpoint examples through your generated test handler with `veld_testing`'s live run. The files keep their `.pending` names; no expected outcome edited.
- Over the real entry point: a value the caller sent for a derived field is replaced; a malformed one is dropped, not a `422`; a derived value read by a `require`; a fact's and the principal's derived fields; a failing derived field of a fact is `500`; the handler's value is overwritten; output mode `off` still computes; a stream item; an `else` payload.
- Events, if your target has them: the publisher computes, the subscriber recomputes.
- The client: the local check computes before it checks and does not send the field; a response's derived value is kept as it came; the public compute function.
- Snapshots of the generated files of a small contract with a chain of derived fields.
- The conformance check.

## Your report

As `brief-in-the-generators.md` says, plus the name of the public compute function in your target and where a failing derived field is reported.

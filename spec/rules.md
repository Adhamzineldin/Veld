# Veld Rules — v1

Status: **DRAFT.** Sections 1 to 8 (expressions and invariants) are being implemented first and are complete. Sections 9 to 12 (endpoint rules, state machines, policies, examples) fix the syntax and the decisions already taken; each is completed before its phase starts.

This document is normative and extends `spec/language.md`. The keywords MUST, MUST NOT and SHOULD carry their usual meaning. Every rule here is backed by a conformance case in `tests/conformance/` or a vector in `tests/rules/`. The background and the reasons are in `docs/proposals/rules-and-events.md`; where the two differ, this document wins, and §16 lists every difference.

## 1. What a rule is

A rule is a boolean expression that the contract declares and that generated code enforces. Rule expressions are **pure** (no side effects), **total** (they always finish with `true`, `false` or a defined evaluation error) and **bounded** (their cost is known from the contract).

| Kind | Syntax | Section | Phase |
|---|---|---|---|
| Invariant | `rule name: expr` in a model or error | §5 | 1 |
| Precondition | `require expr else Error` in an endpoint block | §9 | 2 |
| Postcondition | `ensure expr` in an endpoint block | §9 | 2 |
| State transition | `flow a -> b` in an enum, `transition` in an endpoint block | §10 | 3 |
| Policy | `policy` with `permit` and `forbid`, used by `allow` | §11 | 4 |
| Example | `example "..." { ... }` | §12 | 2 |

**Two gates hold for everything in this document:**

1. A contract that uses none of this syntax MUST compile to the same IR JSON and the same generated files, byte for byte, as before this document existed (apart from the single `ir_version` change of §13).
2. A generator that cannot enforce a rule kind MUST report `E0801`. It MUST NOT generate code that ignores the rule.

## 2. Lexical additions

### 2.1 Tokens

New punctuation: `&&` `||` `!` `==` `!=` `<=` `>=` `+` `-` `/` `%` `=>`. The tokens `<` `>` `*` `=` `.` already exist.

**Minus.** A `-` that is immediately followed by a digit is part of a numeric literal, as today, **unless** the previous token is an identifier, a numeric or string literal, `)` or `]`. In that case it is the minus operator. So `@min(-5)`, `= -1` and `[-1, -2]` are literals, and `x -1`, `end - start` and `days(30) -1` are subtractions.

### 2.2 Words

Every new word is **contextual**: it is a keyword only where the grammar below expects it, and an ordinary identifier everywhere else. A field, variant, parameter, endpoint or declaration MAY be named after any of them. The reserved list of `language.md` §3.2 does not grow.

```
rule message implies not in has
fact require else ensure old now result
flow final emits transition
policy permit forbid when allow principal
example given expect
```

Disambiguation, with at most three tokens of lookahead:

| Where | Tokens | Meaning |
|---|---|---|
| model or error body | `rule` IDENT `:` | a rule. `rule: T` and `rule?: T` are fields named `rule`. |
| enum body | `flow` IDENT `->` | a flow statement (§10). Otherwise `flow` is a variant. |
| enum body | `final` IDENT on the same line | a final statement (§10). Otherwise `final` is a variant. |
| after an expression | `in` or `implies` followed by `:` or `?` | the start of a member named `in` or `implies`, not an operator. |
| top level | `policy` IDENT `(`, `principal` IDENT, `example` STRING | declarations (§11, §12). |

The single source of truth for these words is `crates/veld-compiler/src/language.rs` (`CONTEXTUAL_RULE_WORDS`); the editor grammars are generated from it.

## 3. Expressions

### 3.1 Grammar

```ebnf
expr        = implies ;
implies     = or [ "implies" implies ] ;                    (* right associative *)
or          = and { "||" and } ;
and         = cmp { "&&" cmp } ;
cmp         = add [ cmpop add ] ;                           (* not associative: a < b < c is E0902 *)
cmpop       = "==" | "!=" | "<" | "<=" | ">" | ">=" | "in" ;
add         = mul { ( "+" | "-" ) mul } ;
mul         = unary { ( "*" | "/" | "%" ) unary } ;
unary       = ( "!" | "not" | "-" ) unary | postfix ;
postfix     = primary { "." IDENT } ;
primary     = literal | "." IDENT | call | IDENT | "(" expr ")" | "[" [ expr { "," expr } [","] ] "]" ;
call        = IDENT "(" [ arg { "," arg } [","] ] ")" ;
arg         = IDENT "=>" expr | expr ;
literal     = STRING | INT | FLOAT | "true" | "false" ;
```

- `not` means exactly `!`, with the same precedence: `not a == b` is `(not a) == b`.
- `.name` as a primary is an enum literal. Its enum is taken from the other operand or the expected type (`status == .paid`, `status in [.paid, .checkedIn]`); if no enum can be inferred it is `E0902`.
- `IDENT => expr` is a lambda. It is allowed only as the second argument of `all`, `any`, `count` and `sum` (§3.5).
- There is no `null`, no ternary, no indexing (`a[0]`, `m["k"]`), no `?.` and no user-defined function.
- Expressions nest at most 64 levels deep (`E0005`).

### 3.2 Names

A bare `IDENT` resolves in this order: (1) a lambda variable; (2) the local scope of the rule (§5.1 for invariants, §9 for endpoint blocks, §11 for policies); (3) a `const` declaration that is in scope by the normal import rules. A name that resolves to nothing is `E0901`, with a did-you-mean hint.

A `const` used in an expression is inlined as a literal. It MUST be a `bool`, `string`, `int`, `long`, `float` or `decimal` constant (`E0905`).

`a.b` reads field `b` of the model-typed value `a`. A `Page<T>` value has the fields `items`, `total`, `page` and `size`.

### 3.3 Types

Every expression has a static type: a Veld type (`language.md` §5) or `duration`, which exists only inside expressions. A rule's expression MUST have type `bool` (`E0902`).

**Numeric literals** take their type from the other operand: `amount > 0` compares decimals when `amount` is a decimal. With no context an `INT` literal is an `int` (a `long` if it does not fit 32 bits) and a `FLOAT` literal is a `decimal`. A literal that does not fit its type is `E0908`.

**Widening** is implicit only where it is exact: `int` to `long`, `int` to `float`, `int` or `long` to `decimal`. Nothing else converts: `long` with `float` and `decimal` with `float` are `E0902`. The compiler makes every conversion explicit in the IR (§13), so a generator never applies a promotion rule itself.

### 3.4 Operators

| Operators | Operand types | Result | Semantics |
|---|---|---|---|
| `&&` `\|\|` `implies` | `bool` | `bool` | Short-circuit, left to right. `a implies b` is `!a \|\| b`. |
| `!` `not` | `bool` | `bool` | |
| `==` `!=` | two values of the same type: `bool`, numeric, `string`, `uuid`, `date`, `datetime`, `time`, `duration`, or the same enum | `bool` | By value, see below. Models, unions, lists, maps, `bytes` and `json` cannot be compared (`E0905`). |
| `<` `<=` `>` `>=` | numeric, `date`, `datetime`, `time`, `duration` | `bool` | By value. Strings cannot be ordered (`E0905`). |
| `in` | `x in list` where `list` is a `T[]` and `x` a `T` that supports `==`; or `key in map` with a `string` key | `bool` | True if some element equals `x`; true if the map has the key. |
| `+` `-` `*` | `int`, `long`, `float`, `decimal` | same | `int` and `long` are checked: overflow is an evaluation error (§3.7). `decimal` is exact. |
| `/` `%` | `int`, `long`, `float` | same | Integer division truncates toward zero; `%` takes the sign of the dividend (`-7 / 2 == -3`, `-7 % 2 == -1`). Division by zero is an evaluation error. On `decimal` both are `E0905`: write `a <= b * 2`, not `a / 2 <= b`. |
| unary `-` | numeric, `duration` | same | Checked. |
| `-` | `datetime - datetime`, `date - date` | `duration` | |
| `+` `-` | `datetime` and `duration`; `duration` and `duration` | `datetime`; `duration` | Checked. |

**Value semantics.** These are what make every target agree, and each has vectors:

- **decimal** compares numerically: `1.50 == 1.5` is true. (Canonical bytes still keep the text; `same()` is not affected.)
- **float** follows IEEE 754 double arithmetic. A result that is not finite is an evaluation error. `==` and `!=` on floats are allowed and produce `W0901`.
- **string** equality is by code points, with no normalisation and no locale.
- **uuid** compares in lower case.
- **datetime** is an instant with **millisecond precision**: the wire value is converted to UTC and digits after the third fractional digit are dropped. `2026-01-01T02:00:00+02:00 == 2026-01-01T00:00:00.000999Z` is true.
- **date** is a calendar day; `date - date` is a whole number of days as a duration.
- **time** compares by milliseconds since midnight, with the same truncation.
- **duration** is a signed 64-bit count of milliseconds.
- **enum** values compare by variant.

### 3.5 Built-in functions

| Function | Types | Result | Notes |
|---|---|---|---|
| `has(path)` | a path that ends in an optional field or passes through one | `bool` | True when every optional step of the path is present. §3.6. |
| `len(x)` | `string`, list, map, `bytes` | `int` | A string's length is its number of Unicode code points, the same as `@length`. `bytes` counts bytes. |
| `matches(s, "re")` | `string`, a string **literal** | `bool` | The regex subset of `language.md` §7.1 (`E0603` otherwise), with the same semantics as `@pattern`: it matches anywhere unless anchored. |
| `startsWith(s, p)` `endsWith(s, p)` `contains(s, p)` | `string`, `string` | `bool` | By code points. |
| `days(n)` `hours(n)` `minutes(n)` `seconds(n)` `ms(n)` | `int` or `long` | `duration` | Checked multiplication to milliseconds. |
| `all(list, x => bool)` | list | `bool` | True for an empty list. Stops at the first false. |
| `any(list, x => bool)` | list | `bool` | False for an empty list. Stops at the first true. |
| `count(list, x => bool)` | list | `int` | Checked. |
| `sum(list, x => n)` | list, `n` numeric | type of `n` | Checked. Zero for an empty list. |

A lambda variable MUST NOT shadow a name of the local scope or an enclosing lambda (`E0901`). Unknown functions and wrong argument counts or types are `E0906`. `min` and `max` are not part of v1 (§16).

### 3.6 Missing values

Rules use two-valued logic. A value is never "unknown".

- A field is **optional in a rule** when it is declared `name?: T`. A field with a default is always present, because rules run after defaults are applied.
- Reading an optional value is allowed only where it is **known present**. Anything else is `E0903`.
- `has(p)` makes `p`, and every prefix of `p`, known present in:
  - the right operand of `has(p) && …`
  - the right operand of `has(p) implies …`
  - the right operand of `!has(p) || …`
  
  The left operand may be a conjunction: in `has(a) && has(a.b) && e`, both are known inside `e`.
- `has` is the only way to ask about presence. There is no comparison with `null`.

### 3.7 Evaluation errors

Three things can go wrong at run time: integer or duration **overflow**, **division by zero**, and a **non-finite** float result. All three are defined behaviour, the same in every target:

- The rule **fails closed**: it counts as violated.
- The violation carries `"error": "overflow" | "division_by_zero" | "non_finite"` (§7), and the rule's own message is replaced by `rule <name> could not be evaluated`.
- Short-circuiting is observable: in `a && b`, `b` is not evaluated when `a` is false, so its error does not happen.

### 3.8 Cost

Evaluating a rule costs time proportional to the size of the value. A comprehension nested in another comprehension, where either list has no `@length(max: n)`, is `W0902`.

## 4. Where rule syntax may appear

```ebnf
model     = "model" IDENT [ "extends" typeref ] "{" { field | rule } "}" ;
error     = "error" IDENT [ STATUS ] "{" { field | rule } "}" ;
rule      = { doc } { annotation } "rule" IDENT ":" expr [ "message" STRING ] ;
```

Rules and fields may be mixed in any order. The formatter prints fields first, then one blank line, then rules, each in source order.

## 5. Invariants

```veld
model Booking {
  start: datetime
  end: datetime
  status: BookingStatus = .pending
  paidAt?: datetime

  /// Shown under the date picker.
  rule endAfterStart: end > start
    message "A booking must end after it starts"
  rule maxStay: end - start <= days(30)
  rule paidHasTime: status in [.paid, .checkedIn] implies has(paidAt)
}
```

### 5.1 Scope and naming

- The local scope of an invariant is the fields of its declaration, by source name, including inherited fields. `now` is not available (`E0904`): data that was valid yesterday stays valid today.
- Rule names MUST be unique in a declaration, counting inherited rules (`E0907`). A rule and a field may share a name.
- `extends` inherits rules together with fields. The child's rule list is the parent's rules followed by its own.
- An invariant that reads no field at all is `W0903`.
- The default message of a rule without `message` is `violates rule <name>`.

### 5.2 When invariants are checked

An invariant of model `M` is checked for **every value of type `M`**, wherever it appears: as a request body, as a `query` object, inside a list, map, page, union arm or optional field of another checked value, and in responses.

- Invariants run on the **decoded value, after defaults are applied**, and only when shape and constraint validation of the whole parameter produced no issue. A response is therefore either a list of validation issues or a list of violations, never a mix.
- **All** invariants of a value are evaluated and all violations are reported (no short-circuit between rules).
- **Order:** for a value of model type, its own rules in list order, then its fields in declaration order, recursively. List elements are visited by index. The order of map entries is not specified; tests compare violations as a set.
- **Request position:** a rule that reads a `@readonly` field is skipped for a request body, because the field is absent there. It is checked on responses.
- **Errors:** invariants of an `error` declaration are checked when the server encodes the error and when a client decodes it.

### 5.3 The server pipeline

The existing order of `spec/targets/typescript.md` §5 is kept; rule steps are added, never moved in front of auth:

1. match
2. auth
3. middleware
4. bind and validate path, query, headers and body
5. **invariants on the bound parameters** → `422 ValidationError` with `violations` (§7)
6. *(Phase 2 and later: facts, policies, transition guard, `require`, in that order)*
7. the handler
8. **invariants on the result**, according to the output mode
9. *(Phase 2 and later: `ensure`, target state)*
10. serialise

**Output mode.** Checks on what callers send are always on. Checks on the server's own output (steps 8 and 9) have a mode, set per server instance:

| Mode | Behaviour |
|---|---|
| `strict` (default) | A violated output rule becomes `500 Internal`. The caller sees no rule detail; the violation goes to the hook. |
| `audit` | The response is sent unchanged; the violation goes to the hook. |
| `off` | Output rules are not evaluated. |

Every generated server has one `onRuleViolation(event)` hook. The event has the rule id, the operation signature, the position (`input` or `output`), the outcome (`rejected` or `audited`), the violation and the time the check took. The server logs the active output mode once at startup. Until the typed runtime config exists, the mode is an option of the existing hooks object of each target (`rules: { output }`).

### 5.4 Clients

A generated client checks the invariants of a request body and of a `query` object **before sending**. On a violation it does not send; it fails with the same typed `ValidationError` the server would return, with the same `violations`, marked as local. A client option turns the local check off.

**Partial check (TypeScript only).** Each model with rules also gets a partial check for forms, `<Model>.partial(draft)`. It evaluates a rule only if the evaluation reads no absent required field; otherwise the rule is skipped, not failed. It returns the same violation objects.

## 6. Rule identity

```
rule_id    = "veld:" package "/" Decl "." ruleName "@" hex16( sha256( rule_bytes ) )
rule_bytes = RFC 8785 JSON of { "d": "<qualified decl name>", "n": ruleName, "e": ExprShape }
```

- `hex16` is the first 16 lowercase hex characters, as in `spec/ir.md` §5.1.
- `ExprShape` is the expression of §13 with source names replaced by wire names and types kept, and without spans. The message, docs and annotations are not part of it. Changing a message keeps the id; changing the expression changes it.
- An inherited rule has the id it has in the declaration that wrote it.
- **A rule does not change the signature of its declaration.** `Decl.sig` stays exactly as `spec/ir.md` §5.1 defines it, like `@min` and `@max` today. Tightening a rule therefore never breaks typed error matching in a client that is already deployed.
- Rule ids are computed only in `veld-compiler::signature`. Generators embed them verbatim.

## 7. Violations on the wire

A rejected request is the existing `422 ValidationError` problem document (`spec/ir.md` §5.5). Its shape and its signature do not change. Rule violations appear twice:

```json
{ "type": "veld:veld/ValidationError@…", "title": "ValidationError", "status": 422,
  "operation": "veld:bookings/Bookings.createBooking@…",
  "data": { "issues": [
    { "path": "body", "code": "rule", "message": "A booking must end after it starts" } ] },
  "violations": [
    { "rule": "veld:bookings/Booking.endAfterStart@91c0d2aa0b1c2d3e",
      "path": "body", "fields": ["end", "start"],
      "message": "A booking must end after it starts" } ] }
```

- `data.issues` gets one issue per violation with code `rule` (or `rule_error` for an evaluation error), so a client that knows nothing about rules still shows every problem.
- `violations` is a new top-level member of the problem document. Each entry has:
  - `rule`: the rule id
  - `path`: the path of the **value that carries the rule**, in the same syntax as issue paths (`body`, `body.items[2]`, `query`, `$` for a standalone check)
  - `fields`: the wire names of the fields of that value which the rule reads, in order of first use, without duplicates. For `a.b.c` the entry is `a`.
  - `message`
  - `error`: only for an evaluation error (§3.7)
- The generated typed `ValidationError` exposes `violations` in every target. A problem document without the member decodes to an empty list.

## 8. Generated code

- For every model or error with at least one rule, each target generates a **check function** as plain code in the target language. There is no interpreter and no rule data structure at run time.
- The check function calls **runtime helpers** for the value semantics of §3.4 (`cmpDecimal`, `addInt`, `instantMs`, `lenCodePoints`, ...). The helpers live in the embedded runtime, have the same names in every target (adjusted to the target's naming style) and are tested by the same vectors. A helper file is emitted only when the contract has rules.
- Expressions are printed through `veld_codegen::expr::ExprPrinter`. A generator supplies only the leaf hooks (field access, literal, helper call, operator and precedence).
- Capability flags, all `E0801` when unsupported: `rules_invariants` (Phase 1), `rules_endpoint`, `rules_transitions`, `rules_policies`.

## 9. Endpoint rules (Phase 2)

An endpoint MAY end with a block:

```veld
post "/" createBooking(body: NewBooking) -> 201 Booking | RoomFull | RoomClosed {
  fact room: Room = body.roomId
  require not room.closed              else RoomClosed
  require body.guests <= room.capacity else RoomFull { capacity: room.capacity }
  require notInPast: body.start >= now
  ensure  result.status == .pending
}
```

```ebnf
endpoint   = … [ "->" responses ] [ "{" { stmt } "}" ] ;
stmt       = fact | allow | transition | require | ensure ;
fact       = "fact" IDENT [ "?" ] ":" type [ "=" expr ] ;
require    = "require" [ IDENT ":" ] expr [ "else" typeref [ "{" { IDENT ":" expr [","] } "}" ] ] ;
ensure     = "ensure" [ IDENT ":" ] expr ;
```

Decisions already taken:

- **Scope:** the endpoint's parameters by name, facts, the principal (§11), `now`, and in `ensure` also `result`. A name declared twice is an error.
- **Facts** are loaded by the developer's code through a generated loader interface, in declaration order, after step 5. A required fact that the loader does not find is `404 NotFound`. `fact x?: T` may be absent and needs a `has` guard. A fact's key may read earlier facts.
- **`require`** lines run in source order and the first failure wins. With `else E`, `E` MUST be one of the endpoint's declared errors and the block gives its fields; without `else` the failure is a `422` with a violation. A `require` whose evaluation fails with an evaluation error fails the same way.
- **`ensure`** is checked on the handler's result according to the output mode (§5.3). A fact read inside `ensure` MUST be written `old(fact.path)`. Every `old(...)` is evaluated and saved **before** the handler runs, so a handler that mutates a loaded object cannot change it.
- **`now`** comes from a clock that is injected into the server. Tests set it.
- **Ids:** a named statement has the id `veld:<package>/<Service>.<endpoint>.<name>@<hex16>`; an unnamed one uses `require` or `ensure` in place of the name.
- **Stale clients:** every response of a service with rules carries `Veld-Rules: <hex16>`, a digest of the sorted ids of every rule a client checks locally. A client generated from other rules stops its local checks, lets the server decide, and calls `onContractMismatch`.

## 10. State machines (Phase 3)

```veld
enum BookingStatus {
  pending, paid, checkedIn, cancelled, refunded
  flow pending -> paid -> checkedIn
  flow pending -> cancelled
  flow paid -> refunded
  final checkedIn, cancelled, refunded
}
```

```ebnf
flow        = "flow" IDENT "->" IDENT { "->" IDENT } [ "emits" typeref ] ;
final       = "final" IDENT { "," IDENT } ;
transition  = "transition" path ":" IDENT { "," IDENT } "->" IDENT ;
```

- `transition booking.status: pending -> paid` in an endpoint block names an enum field of a fact. Every listed edge MUST exist in the enum's flows.
- Before the handler, a current value outside the listed source states is `409 InvalidTransition` (a new prelude error, §13). After the handler, if the result has the fact's type, its field MUST equal the target state (output mode).
- **This is a guard, not an atomic guarantee.** A second request can change the row after the check. The handler therefore receives the transition as data (field, expected source state, target state) and MUST apply it with a conditional write (compare-and-swap or a row lock) in its own transaction. The documentation and every diagnostic say "guard", never "guarantee".
- `veld check` reports a state that no flow reaches and a non-final state with no way out.

## 11. Policies (Phase 4)

```veld
principal User

policy ManageBooking(user: User, booking: Booking) {
  permit when booking.guestId == user.id
  permit when "support" in user.roles
  forbid when user.suspended
}
```

```ebnf
principal  = "principal" typeref [ "as" IDENT ] ;
policy     = "policy" IDENT "(" params ")" "{" { ( "permit" | "forbid" ) "when" expr } "}" ;
allow      = "allow" IDENT "(" [ expr { "," expr } ] ")" ;
```

- A package declares at most one principal. Its value is in scope of that package's endpoint blocks under the name `user`, or the name after `as`.
- A policy permits when at least one `permit` is true **and** no `forbid` is true. `allow` in an endpoint block calls it; a denial is `403 Forbidden`. Several `allow` lines must all permit.
- Policies are generated for clients too, so a UI can ask before it shows a button. The server always decides.

## 12. Examples (Phase 2)

```veld
example "a full room is rejected" {
  given room = { capacity: 2, closed: false }
  createBooking(body: { guests: 3 }) -> RoomFull { capacity: 2 }
}
```

An example names facts, optionally a principal (`as { … }`) and a clock, calls one endpoint and states the outcome. `veld test` runs examples against the reference evaluator with the given facts, and with `--live` against a running server that was started in test mode. The full grammar is completed with Phase 2.

## 13. IR additions

`Model` (and through it `ErrorDecl`) gains `rules`. It is omitted from the JSON when empty, so the IR of a contract without rules is unchanged.

```jsonc
Model = { "kind": "model", "parent": DeclId|null, "fields": [Field], "rules": [Rule] }   // rules: inherited first

Rule = {
  "name": "endAfterStart",
  "id": "veld:bookings/Booking.endAfterStart@91c0d2aa0b1c2d3e",       // §6
  "expr": Expr,
  "message": "A booking must end after it starts" | null,
  "reads": ["end", "start"],            // source names of the fields of this declaration that the rule reads, first use first
  "reads_readonly": false,              // true when one of them is @readonly (§5.2)
  "inherited_from": DeclId | null,
  "docs": .., "deprecated": .., "origin": Origin
}

Expr = { "t": ExprType, "e": ExprKind }
ExprType = Type | "duration"
ExprKind =
    { "lit": { "bool": true } | { "int": 5 } | { "long": "9007199254740993" } | { "float": 0.5 }
           | { "decimal": "120.00" } | { "string": "x" } | { "enum": { "decl": DeclId, "name": "paid", "wire": "PAID" } } }
  | { "path": { "root": Root, "steps": [ { "name": "roomId", "wire": "room_id", "optional": false } ] } }
  | { "now": true }
  | { "not": Expr } | { "neg": Expr }
  | { "bin": { "op": Op, "l": Expr, "r": Expr } }
  | { "has": { "root": Root, "steps": [...] } }
  | { "call": { "f": Builtin, "args": [Expr] } }
  | { "quant": { "q": "all|any|count|sum", "list": Expr, "var": "i", "body": Expr } }
  | { "list": [Expr] }
  | { "widen": Expr }                                  // exact conversion to this node's "t" (§3.3)
  | { "old": Expr }                                    // Phase 2

Root    = "self" | { "var": "i" } | { "param": "body" } | { "fact": "room" } | "principal" | "result"
Op      = "and|or|implies|eq|ne|lt|le|gt|ge|in|add|sub|mul|div|rem"
Builtin = "len|matches|starts_with|ends_with|contains|days|hours|minutes|seconds|ms"
```

- Both operands of a `bin` have the same type, except `in` (element and collection) and the datetime and duration combinations of §3.4. Every other difference was resolved by a `widen` node.
- A `const` is already replaced by its `lit`. An `int` or `long` literal is stored in its final type.
- `Feature` gains `rules_invariants`, `rules_endpoint`, `rules_transitions` and `rules_policies`.
- **One-time change to existing output:** `ir_version` becomes `1.1`, and the prelude gains `error InvalidTransition 409 { field: string, from: string, to: string }`. Both change every stored IR (`veld.lock.ir.json` and IR snapshots) once. No generated file of an existing contract changes, because a target emits only the prelude declarations a service reaches.

## 14. Diagnostics

Range `E09xx` (errors) and `W09xx` (warnings). Codes are never reused.

| Code | Meaning |
|---|---|
| `E0901` | Unknown name in an expression, or a lambda variable that shadows another name |
| `E0902` | Type error in an expression: operand types do not fit the operator, a rule that is not `bool`, a chained comparison, an enum literal with no enum to infer |
| `E0903` | An optional value is read where it is not known present (§3.6) |
| `E0904` | `now`, `result` or `old` used where it is not available |
| `E0905` | Operation not supported on this type: `/` or `%` on decimal, ordering strings, comparing models, lists, maps, bytes or json, a `const` of an unsupported type |
| `E0906` | Unknown function, wrong number or type of arguments, lambda where none is allowed |
| `E0907` | Duplicate rule name in a declaration, counting inherited rules |
| `E0908` | Numeric literal out of range for its type |
| `W0901` | `==` or `!=` on `float` |
| `W0902` | Nested comprehension over a list without `@length(max:)` |
| `W0903` | A rule that reads no field |

`E0920` to `E0929` are reserved for endpoint rules, `E0930` to `E0939` for state machines, `E0940` to `E0949` for policies and `E0950` to `E0959` for examples.

## 15. Test vectors

`tests/rules/` holds the shared vectors. Every generated target and the reference evaluator (`crates/veld-rules`) MUST produce exactly the expected result for every case. This is the rules counterpart of `tests/wire/`.

```
tests/rules/<group>/<name>.veld      a contract (package name "t")
tests/rules/<group>/<name>.json      cases for it
```

```jsonc
{ "cases": [
  { "name": "ends before it starts",
    "type": "Booking",                       // a model of the contract
    "value": { "start": "2026-01-02T00:00:00Z", "end": "2026-01-01T00:00:00Z" },   // wire form
    "expect": [ { "rule": "endAfterStart", "path": "$", "fields": ["end", "start"] } ] },
  { "name": "int overflow fails closed",
    "type": "Sum", "value": { "a": 2147483647, "b": 1 },
    "expect": [ { "rule": "small", "path": "$", "fields": ["a", "b"], "error": "overflow" } ] }
] }
```

- `expect` is the list of violations, compared as a set of `(rule name, path, fields, error)`. `[]` means the value is valid. Messages and rule ids are covered by snapshots, not by vectors.
- A case with `"request": true` checks the value in request position (§5.2). A case with `"partial": true` runs the partial check and applies to TypeScript and the reference evaluator only.
- The starter set MUST cover, at least: code-point length with astral characters; `int` and `long` overflow at both bounds, including a `long` beyond 2^53; truncating division and remainder with negative operands; division by zero; decimal equality across scales and exact decimal arithmetic; float non-finite results; datetime offsets and sub-millisecond digits; `date` differences; duration overflow; `uuid` case; enum literals and `in`; every short-circuit form of §3.6; `all`, `any`, `count` and `sum` on empty and non-empty lists; nested models, lists, maps, pages and union arms; inherited rules; `@readonly` in request position; defaults applied before rules.

## 16. Differences from the proposal

| Proposal | This document | Why |
|---|---|---|
| Rules feed the declaration signature | Rules have their own ids; `Decl.sig` is unchanged (§6) | A rule change must not break typed error matching in deployed clients. Decided by the owner on 2026-11-24. |
| Decode, then invariants, then auth | Auth and middleware first (§5.3) | Servers already answer `401` before `422`; changing it would alter existing behaviour and tell unauthenticated callers about rules. |
| JSON pointer in `path` | The existing issue path syntax, plus `fields` (§7) | One path syntax in one error document; `fields` tells a form which inputs to mark. |
| `x?.y` as a guard | Not in v1; `has(x.y)` covers it (§3.6) | An optional chain needs a third truth value. |
| `min`, `max` | Not in v1 (§3.5) | They have no value for an empty list, which breaks totality. `all(items, i => i.qty <= 10)` states a bound. |
| Strings can be ordered | `==`, `!=`, `in` and the string functions only (§3.4) | JavaScript and Java order strings by UTF-16 units, Rust by code points. |
| Decimal division | Not supported (§3.4) | It needs a rounding rule; multiplication states the same condition exactly. |
| `final a, b` anywhere in an enum | A statement only when an identifier follows on the same line (§2.2) | `enum X { final }` must keep meaning a variant. An existing enum with a variant named `final` followed on the same line by another variant without a comma is the one program whose meaning changes. |
| `v.end.getTime()` in the TypeScript sketch | Runtime helpers (§8) | `datetime`, `decimal` and `long` are strings in TypeScript and Rust. |
| `principal User`, used as `user` | The same, plus `as name` (§11) | A package whose endpoints already have a parameter named `user` needs a way out. |

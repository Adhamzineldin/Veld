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

Inside an expression the result does not depend on that choice: **a unary minus applied directly to a numeric literal is part of the literal.** `x implies -2147483648 < n` and `not -1 == n` therefore contain the literals `-2147483648` and `-1`, although the lexer saw an operator after the word, and the smallest `int` and `long` can be written anywhere.

### 2.2 Words

Every new word is **contextual**: it is a keyword only where the grammar below expects it, and an ordinary identifier everywhere else. A field, variant, parameter, endpoint or declaration MAY be named after any of them. The reserved list of `language.md` §3.2 does not grow.

```
rule message implies not in has
fact require else ensure old now result
flow final emits transition
policy permit forbid when allow principal
example given absent returns ok expect
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

- `not` means exactly `!`, with the same precedence: `not a == b` is `(not a) == b`. `not` is the operator only when an operand follows it; `not in xs` and `not implies y` read a field named `not`.
- A chained comparison is reported by the parser, so no later check runs on that file.
- `.name` as a primary is an enum literal. Its enum is taken from the other operand or the expected type (`status == .paid`, `status in [.paid, .checkedIn]`); if no enum can be inferred it is `E0902`.
- `IDENT => expr` is a lambda. It is allowed only as the second argument of `all`, `any`, `count` and `sum` (§3.5).
- There is no `null`, no ternary, no indexing (`a[0]`, `m["k"]`), no `?.` and no user-defined function.
- An expression tree is at most **32 levels** deep (`E0005`). Every operator, call and list adds a level, also in a chain: `a && b && c` is two levels deep, so one rule can join at most 33 terms with the same operator. The limit keeps the IR JSON of any rule within the nesting limit of common JSON readers (128 in `serde_json`); a rule at the limit MUST read back from its IR JSON. The compiler also reports `E0005` for a rule whose IR would nest too deep for another reason, which takes a very deeply nested list type. Split a longer rule into several rules: each then reports its own message.

### 3.2 Names

A bare `IDENT` resolves in this order: (1) a lambda variable; (2) the local scope of the rule (§5.1 for invariants, §9 for endpoint blocks, §11 for policies); (3) a `const` declaration that is in scope by the normal import rules. A name that resolves to nothing is `E0901`, with a did-you-mean hint.

A `const` used in an expression is inlined as a literal. It MUST be a `bool`, `string`, `int`, `long`, `float` or `decimal` constant (`E0905`).

`a.b` reads field `b` of the model-typed value `a`. A `Page<T>` value has the fields `items`, `total`, `page` and `size`.

### 3.3 Types

Every expression has a static type: a Veld type (`language.md` §5) or `duration`, which exists only inside expressions. A rule's expression MUST have type `bool` (`E0902`).

**Numeric literals** take their type from the other operand: `amount > 0` compares decimals when `amount` is a decimal. With no context an `INT` literal is an `int` (a `long` if it does not fit 32 bits) and a `FLOAT` literal is a `decimal`; when both operands are literals and one is a `FLOAT`, both are decimals. A literal that does not fit its type is `E0908`. So is an integer literal beyond 2^53 used as a `float`, and a literal with an exponent used as a `decimal`.

**Widening** is implicit only where it is exact: `int` to `long`, `int` to `float`, `int` or `long` to `decimal`. Nothing else converts: `long` with `float` and `decimal` with `float` are `E0902`. The compiler makes every conversion explicit in the IR (§13), so a generator never applies a promotion rule itself.

### 3.4 Operators

| Operators | Operand types | Result | Semantics |
|---|---|---|---|
| `&&` `\|\|` `implies` | `bool` | `bool` | Short-circuit, left to right. `a implies b` is `!a \|\| b`. |
| `!` `not` | `bool` | `bool` | |
| `==` `!=` | two values of the same type: `bool`, numeric, `string`, `uuid`, `date`, `datetime`, `time`, `duration`, or the same enum | `bool` | By value, see below. Models, unions, lists, maps, `bytes` and `json` cannot be compared (`E0905`). |
| `<` `<=` `>` `>=` | numeric, `date`, `datetime`, `time`, `duration` | `bool` | By value. Strings cannot be ordered (`E0905`). |
| `in` | `x in list` where `list` is a `T[]` and `x` a `T` that supports `==`; or `key in map` with a `string` key | `bool` | True if some element equals `x`; true if the map has the key. A `duration` cannot be used with `in` (`E0902`): no Veld list holds durations. `in` is the only operator whose operand types differ; the element side is widened when that is exact. |
| `+` `-` `*` | `int`, `long`, `float`, `decimal` | same | `int` and `long` are checked: overflow is an evaluation error (§3.7). `decimal` is exact. |
| `/` `%` | `int`, `long`, `float` | same | Integer division truncates toward zero; `%` takes the sign of the dividend (`-7 / 2 == -3`, `-7 % 2 == -1`). A zero divisor is the evaluation error `division_by_zero`, for floats too (it is checked before dividing). `%` never overflows: the smallest `int` `% -1` is `0`, while the same division overflows. On `decimal` both are `E0905`: write `a <= b * 2`, not `a / 2 <= b`. |
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
| `has(path)` | a path that ends in an optional field or passes through one | `bool` | True when every optional step of the path is present. §3.6. A path that can never be absent is `E0906`. |
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
- The analysis follows the logic, not three fixed shapes. For an expression `x`, let `when_true(x)` be the paths known present if `x` is true, and `when_false(x)` those known present if it is false:
  - `when_true(has(p))` is `p` and its prefixes
  - `when_true(a && b)` is `when_true(a)` joined with `when_true(b)`; `when_false(a || b)` is `when_false(a)` joined with `when_false(b)`
  - `when_true(!a)` is `when_false(a)`, and `when_false(!a)` is `when_true(a)`
  - the right operand of `&&` and of `implies` is checked with `when_true(left)`; the right operand of `||` with `when_false(left)`
  
  So `has(a) && has(a.b) && e`, `!has(a) || !has(b) || e` and `!(has(a) && has(b)) || e` all make both paths known inside `e`.
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
- **Errors:** invariants of an `error` declaration are output checks. The server checks them when it encodes a declared error, under the output mode (§5.3): in `strict` the caller gets `500 Internal` and not the malformed error. A client never checks them when it decodes an error, so it can always read what a server sent.
- **Streams:** every item a server sends on an SSE or WebSocket stream is an output: it is checked under the output mode before it is sent, and in `strict` a violating item is not sent and the stream ends the way it ends on any server failure. Every message a client sends on a WebSocket is an input: the client checks it locally like a request body, and the server rejects a violating message the way it rejects a message of the wrong shape, with the `violations` attached.

### 5.3 The server pipeline

The existing order of `spec/targets/typescript.md` §5 is kept; rule steps are added, never moved in front of auth:

1. match
2. auth
3. middleware
4. bind and validate path, query, headers and body
5. **invariants on the bound parameters** → `422 ValidationError` with `violations` (§7)
6. facts, policies, transition guard, `require`, in that order (§9, §10, §11)
7. the handler
8. **invariants on the result**, according to the output mode
9. `ensure` and the target state (§9, §10)
10. serialise

**Request validation turned off.** Rules need well-formed values. A target option that turns request validation off (`validate = false`) therefore turns steps 5 and 6 off too, and the client's local check with it. A server built that way says so once, when its handler is created, so nobody runs unchecked by accident. `veld generate` warns (`W0904`) when such a target reaches a declaration or endpoint with rules.

**Output mode.** Checks on what callers send are always on. Checks on the server's own output (steps 8 and 9) have a mode, set per server instance:

| Mode | Behaviour |
|---|---|
| `strict` (default) | A violated output rule becomes `500 Internal`. The caller sees no rule detail; the violation goes to the hook. |
| `audit` | The response is sent unchanged; the violation goes to the hook. |
| `off` | Output rules are not evaluated. |

Every generated server has one `onRuleViolation(event)` hook. The event has the rule id, the operation signature, the position (`input` or `output`), the outcome (`rejected` or `audited`), the violation and the time the check took. A mode other than `strict` is reported once, when the handler is created. Until the typed runtime config exists, the mode is an option of the existing hooks object of each target (`rules: { output }`). In `audit` a result whose shape is invalid is passed through like any other result; its rules are not evaluated.

### 5.4 Clients

A generated client checks the invariants of a request body and of a `query` object **before sending**. On a violation it does not send; it fails with the same typed `ValidationError` the server would return, with the same `violations`, marked as local. A client option turns the local check off.

**Partial check (TypeScript only).** Each model with rules also gets a partial check for forms, `<Model>.partial(draft)`. A draft may lack required fields and is not shape-validated; the fields it has are assumed well formed, and defaults are applied. A rule is **skipped, not failed**, when evaluating it reads a required field that is absent; a rule that reads only optional fields under their guards always runs. This is decided while evaluating, not from the list of fields the rule mentions: with only `b: 0` present, `b != 0 && a / b > 1` is a violation, because the short circuit never reads `a`. Nested values are checked the same way. It returns the same violation objects.

## 6. Rule identity

```
rule_id    = "veld:" package "/" Decl "." ruleName "@" hex16( sha256( rule_bytes ) )
rule_bytes = RFC 8785 JSON of { "d": "<qualified decl name>", "n": ruleName, "e": ExprShape }
```

- `hex16` is the first 16 lowercase hex characters, as in `spec/ir.md` §5.1.
- `ExprShape` is the `Expr` JSON of §13, made independent of anything that may change without changing the rule's meaning:
  - a declaration id is never written: a type `{ "decl": id }` becomes `{ "ref": "<qualified name>" }` (as in `spec/ir.md` §5.1), and an enum literal becomes `{ "enum": { "ref": "<qualified name>", "wire": "PAID" } }`
  - a path step is `{ "w": "<wire name>" }` only, so renaming a field in source while keeping its wire name keeps the id
  - lambda variables are renamed `v0`, `v1`, … by nesting depth
  - every numeric literal is a string, so no number formatting is involved: `int` and `long` as decimal digits, `decimal` as its source text, `float` as the 16 lowercase hex digits of its IEEE 754 bits
- The message, docs, annotations and spans are not part of the id. Changing a message keeps the id; changing the expression changes it.
- Types (`t`) stay in the shape, and `d` is the qualified name while the id text uses the plain declaration name. For `rule r: a > 0` on `model M { a: int @wire("A") }` in package `app`, module `app`, the hashed bytes are exactly:
  ```json
  {"d":"app:app.M","e":{"e":{"bin":{"l":{"e":{"path":{"root":"self","steps":[{"w":"A"}]}},"t":"int"},"op":"gt","r":{"e":{"lit":{"int":"0"}},"t":"int"}}},"t":"bool"},"n":"r"}
  ```
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
  - `fields`: the wire names of the fields of that value which the rule reads, in order of first use in the source text, without duplicates. For `a.b.c` the entry is `a`. A field named in `has(...)` counts as read, and so does one that was not evaluated because of a short circuit: the list is a property of the rule, not of the request.
  - `message`
  - `error`: only for an evaluation error (§3.7)
- The generated typed `ValidationError` exposes `violations` in every target. A problem document without the member decodes to an empty list.

## 8. Generated code

The same structure in every target, so the three implementations can be compared line by line:

- **Check functions.** For every model or error with at least one rule, a target generates one check function as plain code in the target language: it evaluates that declaration's rules, in list order, on one value and appends violations to a list. There is no expression interpreter and no rule data structure at run time.
- **Nesting belongs to the runtime, not to generated code.** The declaration's schema node refers to its check function. The embedded runtime already walks a value by its schema (for validation and canonical bytes); the same walk calls the check function of every object it visits, the object's own rules before its fields. This gives the order and the paths of §5.2 once per target instead of once per model.
- **Modes.** A check runs in one of three modes: *full*; *request* (rules with `reads_readonly` are skipped); *partial* (§5.4, TypeScript only).
- **Runtime helpers** carry the value semantics of §3.4 (checked arithmetic, decimals, instants, code points, the regex subset). They live in the embedded runtime, in a file that is emitted only when the contract has rules, have the same names in every target adjusted to its naming style, and are tested by the vectors. The reference list of names is in `spec/targets/typescript.md`.
- **Evaluation errors** are raised by the helpers in the target's own way (an exception in TypeScript and Java, a `Result` in Rust). The check function catches them per rule and records the violation with its `error` (§3.7). Nothing else may escape a check function.
- **Printing.** Expressions are printed through `veld_codegen::expr::ExprPrinter`: one shared traversal, and per-target hooks for field access, literals, operators by operand type, helper calls and comprehension forms. The IR already contains every conversion (`widen`), so a hook never decides a type.
- **Public surface per declaration with rules:** a full check that returns the violations of a value; the rule ids as named constants; in TypeScript also `partial(draft)`.
- **Client.** The local check before sending is on by default and has one option to turn it off. A local failure is the target's typed `ValidationError`, with `violations` filled and a flag that says it was raised locally.
- **Server.** Step 5 always runs. Steps 8 and 9 follow the output mode; when the mode is not `off`, the server first validates the shape of the result (as its response validation option does), so output rules only ever see well-formed values. The options object of the server gains `rules: { output }` and `onRuleViolation`. The server reports a mode other than `strict` once, when the handler is created.
- **Capability flags**, all `E0801` when unsupported: `rules_invariants` (Phase 1), `rules_endpoint`, `rules_transitions`, `rules_policies`.

## 9. Endpoint rules (Phase 2)

An endpoint MAY end with a block of statements:

```veld
post "/" createBooking(body: NewBooking) -> 201 Booking | RoomFull | RoomClosed {
  fact room: Room = body.roomId
  require not room.closed              else RoomClosed
  require body.guests <= room.capacity else RoomFull { capacity: room.capacity }
  require notInPast: body.start >= now
    message "A booking cannot start in the past"
  ensure  result.status == .pending
}
```

```ebnf
endpoint   = … [ "->" responses ] [ "{" { stmt } "}" ] ;
stmt       = fact | allow | transition | require | ensure ;
fact       = "fact" IDENT [ "?" ] ":" type [ "=" expr ] ;
require    = { doc } "require" [ IDENT ":" ] expr [ "else" typeref [ payload ] ] [ "message" STRING ] ;
payload    = "{" { IDENT ":" expr [","] } "}" ;
ensure     = { doc } "ensure" [ IDENT ":" ] expr [ "message" STRING ] ;
```

A statement starts with one of the five words; nothing else may appear in the block. Until their phases land, `allow` and `transition` are rejected with `E0003` and a message that names the phase. `allow` is §11 and `transition` is §10. The formatter prints the statements in this order: facts, `allow`, `transition`, `require`, `ensure`, each group in source order.

### 9.1 Scope

Inside the block these names are in scope:

| Name | What it is |
|---|---|
| each parameter | by its name: path parameters, `@query` and `@header` parameters, `body`, `query`. A `stream` body is not in scope. |
| each fact | the value the loader returned |
| the principal | under the name of §11, when the package declares one and the endpoint's auth is not `none` |
| `now` | a `datetime`: one reading of the server's clock, taken once per request before the facts load |
| `result` | in `ensure` only: the success value. An endpoint with no success body, or with a `stream` success, has no `result` (`E0904`) |

A fact named like a parameter, another fact, the principal, `now` or `result` is `E0920`. A parameter named `now` or `result` hides the built-in name in that block. Statement names (`require notInPast: …`) are unique in an endpoint (`E0907`); two unnamed statements with the same expression share an id.

A fact's key sees the parameters and the facts declared before it. `require` and `ensure` see every fact of the endpoint, wherever they are written, because all facts load first. `now` may be used in a fact's key, in `require`, in an `else` payload and in `ensure`.

### 9.2 Facts

A fact is a value the rules need and only the application can load: the room being booked, the account being charged.

- `fact name: T = key` declares it. `key` is an expression over the parameters and the facts declared **before** it; it may be left out for a fact that needs no key.
- For each service the generator emits **one loader interface**, with one method per fact name. Two facts with the same name in one service MUST have the same type, the same key type and the same optionality (`E0922`), so one loader serves every endpoint.
- Facts are loaded in declaration order, after step 5, whatever their position among the other statements.
- A required fact that the loader does not find answers **`404 NotFound`** with `resource` set to the fact's name. An endpoint with a required fact implicitly declares `NotFound`, like the implicit `ValidationError`.
- `fact x?: T = key` may be absent and needs a `has` guard where it is read. If its key reads an optional value that is absent, the loader is not called and the fact is absent. The key of a required fact must be known present (`E0903`).
- Facts are trusted values of the application: they are not validated. They are handed to the handler together with the parameters, so the handler does not load them a second time.

### 9.3 `require`

- `require` lines are evaluated in source order after the facts, the policies and the transition guard. **The first failure ends the request.**
- With `else E`, the answer is the error `E`, which MUST be one of the endpoint's declared errors (`E0921`). The payload block gives its fields: every field of `E` that is neither optional nor defaulted MUST be given, an unknown field is `E0922`, and a value of the wrong type is `E0902`. Payload expressions are evaluated only on failure, in the same scope.
- Without `else`, the answer is `422 ValidationError` with one issue of code `rule`. An endpoint with such a `require` declares `ValidationError` even if it has no parameters.
- Either way the problem document carries one entry in `violations` (§7), so a caller and the hook can tell which rule fired. For an endpoint rule, `path` is `request` and `fields` are the names of the **parameters** the expression reads, in order of first use.
- A `require` whose evaluation fails with an evaluation error (§3.7) has failed: the same answer, with `error` in the violation.
- The default message is `violates rule <name>`, or `precondition failed` for an unnamed `require`.
- If the payload of an `else` error cannot be evaluated, or the error it builds breaks one of its own invariants, the answer is `500 Internal`: the server could not build the error it promised.

### 9.4 `ensure` and `old`

- `ensure` is checked on the handler's **success** result, after the invariants of the result, according to the output mode (§5.3). It is not checked when the handler answers with a declared error.
- It may read `result`, the parameters, `now` and `old(...)`.
- **`old(e)`** is the value of `e` before the handler ran. `e` may read facts and parameters. Every `old(...)` is evaluated and saved at the end of step 6, so a handler that changes a loaded object in place cannot change it. A fact read in `ensure` MUST be inside `old(...)` (`E0923`); `old` anywhere else is `E0904`.
- `ensure` is not allowed on an endpoint whose success is a `stream` (`E0924`).
- An `ensure` violation has the same `path` (`request`) and `fields` (the parameters it reads) as a `require` violation. Its default message is `violates rule <name>`, or `postcondition failed` when unnamed.

### 9.5 The pipeline, complete

Steps 1 to 5 and 7, 8, 10 are those of §5.3. Step 6 is: read the clock; load the facts in order (`404`); policies (`403`, §11); transition guard (`409`, §10); `require` in order; save every `old(...)`. Step 9 is: `ensure` in source order; the target state of a transition (§10).

The server's options gain the fact loaders (required by the type system when the service has facts) and a `clock`. The default clock is the system clock.

### 9.6 Identity

```
rule_id    = "veld:" package "/" Service "." endpoint "." name "@" hex16( sha256( rule_bytes ) )
rule_bytes = RFC 8785 JSON of { "d": "<qualified service name>.<endpoint>", "k": "require" | "ensure", "n": name, "e": ExprShape }
```

`name` is the statement's name, or `require` or `ensure` for an unnamed statement. The `else` error, its payload and the message are not part of the id. In `ExprShape` a parameter is `{ "param": name }` and a fact is `{ "fact": name }`.

### 9.7 Stale clients

A client checks invariants locally (§5.4). If the server's rules have been loosened since the client was generated, that check would reject requests the server now accepts. The **rules digest** prevents it.

- `digest = hex16( sha256( RFC 8785 JSON array of the sorted, distinct rule ids of every invariant of every declaration the service's endpoints reach ) )`. It is computed by the compiler, stored on the service in the IR, and embedded in the generated client and server. Facts do not count: a client never checks them. A service that reaches no invariant has none.
- The server sends `Veld-Rules: <digest>` on every response of that service, errors included.
- The client remembers the digest of the last response. **It rejects locally only while that digest equals its own.** Before the first response, or when the header is missing, a request that fails the local check is sent anyway and the server decides.
- When a response carries a different digest, the client stops its local checks for that service and calls `onContractMismatch` once per digest it had not seen, with the service name, its own digest and the server's.
- A browser client can only read the header if the server's CORS configuration exposes it (`Access-Control-Expose-Headers: Veld-Rules`). Without it the client behaves as before the first response: it never rejects locally.

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

## 12. Examples and `veld test` (Phase 2)

An example states what one call does. It is documentation that runs.

```veld
example "a full room is rejected" {
  given room = { capacity: 2, closed: false }
  createBooking(body: { roomId: room.id, guests: 3 }) -> RoomFull { capacity: 2 }
}

example "a missing room is not found" {
  given room = absent
  createBooking(body: { guests: 1 }) -> NotFound
}

example "the booking starts as pending" {
  at "2026-03-01T09:00:00Z"
  createBooking(body: { guests: 1, start: "2026-03-02T00:00:00Z", end: "2026-03-03T00:00:00Z" })
    returns { status: .paid }
    -> Internal
}
```

```ebnf
example  = { doc } "example" STRING "{" { given } [ "as" value ] [ "at" STRING ] call "}" ;
given    = "given" IDENT "=" ( value | "absent" ) ;
call     = [ IDENT "." ] IDENT "(" [ IDENT ":" value { "," IDENT ":" value } [","] ] ")"
           [ "returns" value ] "->" outcome ;
outcome  = "ok" [ value ] | typeref [ value ] ;
value    = literal | "." IDENT | IDENT { "." IDENT }
         | "{" [ IDENT ":" value { "," IDENT ":" value } [","] ] "}"
         | "[" [ value { "," value } [","] ] "]" ;
```

The words `returns`, `ok` and `absent` are contextual, like every other word of this document.

### 12.1 Meaning

- An example is a top-level declaration. Its name is unique in its package (`E0950`).
- The call names an endpoint of the package, as `endpoint` or as `Service.endpoint` when two services share the name. Unknown or ambiguous is `E0950`. Arguments are given by parameter name.
- `given name = value` sets the fact `name` of that endpoint (`E0951` if the endpoint has no such fact). `given name = absent` makes the loader find nothing.
- `as value` is the principal (`E0952` until §11 gives it a type). `at "…"` is the clock; without it the clock is `2026-01-01T00:00:00Z`.
- A value is checked against the type it is used at (`E0952`). `a.b` reads a field of a given.
- `returns value` states what the handler returns. Without it, an offline run stops before the handler.
- The **outcome** is `ok`, or the name of an error the endpoint can answer with, including the implicit ones (`ValidationError`, `NotFound`, `Unauthorized`, `Forbidden`, `InvalidTransition`, `Internal`). An error the endpoint cannot answer with, an unknown argument and a clock that is not a datetime are `E0952`. A value after it is a **pattern**: every field it names must equal the actual one by the value semantics of §3.4, fields it does not name are ignored, objects match recursively, and lists match in length and order.

### 12.2 Values an example leaves out

Examples stay short because everything they leave out is filled in, the same way every time:

| Type | Filled in with |
|---|---|
| a field with `@example` or a default | that value, `@example` first; this holds for an optional field too |
| a required fact the example does not give | a filled-in value, after the givens |
| an optional field | nothing: it stays absent |
| `string` | `a` repeated to the minimum length, at least once. A field with `@pattern` or `@format` and no `@example` cannot be filled in (`E0953`). |
| `int`, `long`, `float`, `decimal` | `0`, moved inside `@min` and `@max` |
| `bool` | `false` |
| `uuid` | `00000000-0000-4000-8000-000000000001`, counting up in decimal through the example |
| `datetime` | the example's clock, plus one millisecond for each `datetime` already filled in during the example: the smallest step a rule can see. So `start` and `end`, declared in that order and both left out, are a millisecond apart and `end > start` holds. |
| `date` | the date of the example's clock, plus one day for each `date` already filled in |
| `time` | the time of the example's clock |
| `bytes`, list, map | empty, or `@length(min)` filled-in elements for a list |
| `json` | `{}` |
| `Page<T>` | an empty page: no items, total `0`, page `1`, size `0` |
| enum | its first variant |
| model | each of its fields by these rules |
| union | its first arm |

Values are filled in in this order: the givens as written, the facts the example does not give, the principal, the arguments in the endpoint's parameter order, the `returns` value; inside a model, its fields in declaration order. The uuid, datetime and date counters run through the whole example; values the example wrote do not advance them.

A filled-in value must be valid. If an invariant is violated and at least one field it reads was filled in, the example is wrong, not the server: `E0953` names the rule and the fields the example must give. An invariant whose fields the example wrote itself may be violated on purpose; that is how an example expects a `ValidationError`.

### 12.3 `veld test`

- **`veld test`** runs every example of the workspace (`-p member`, `--filter text`) against the reference evaluator (`crates/veld-rules`): it applies steps 5 and 6 of the pipeline with the given facts, principal and clock, and, when the example has `returns`, steps 8 and 9 in `strict` mode. No server, no network, milliseconds. It exits with 1 when an example fails and prints, per failure, the expected and the actual outcome with the violations. An example that expects `ok` with a pattern but has no `returns` cannot be checked offline; it is reported as skipped, with the reason, and does not fail the run.
- **`veld test --live <base-url>`** sends each example as a real request to a running **test handler** (§12.4) and compares the status, the error and the patterns. An example without `returns` runs the application's real handler, so `ok { … }` checks what it actually answered.
- **`--fuzz N`** (with `--live`; `--seed S` to repeat a run) builds `N` requests per endpoint from the types, constraints and rules, valid and invalid, works out the expected outcome with the reference evaluator, and fails on any difference in status, error or violation set. A failure prints the seed and the request in example syntax.
- **Boundaries.** With `--live`, every comparison of a number, a time or a length with a constant in a rule also yields requests at the bound and one step on each side.

### 12.4 The test handler

A test must hand a server its facts, its principal and its clock. The production handler has no such door, and gets none.

- A server target with the option `testing = true` generates a **second entry point** in a file of its own, for services that have endpoint rules. Nothing in the production entry point changes.
- The test handler runs the same pipeline, but takes the facts from the request header `Veld-Test-Facts` (base64url of a JSON object: fact name to wire value, or `null` for absent), the principal from `Veld-Test-Principal`, the clock from `Veld-Test-Now` and, when present, the result from `Veld-Test-Result` in place of calling the application's handler. The application's loaders and auth hook are not called.
- It answers `GET /__veld/test` with the rules digests of its services, so `veld test --live` can tell a test handler from a production server and refuse the latter.
- It MUST NOT be mounted in production. The generated file says so at its top, and creating the handler reports it once.

### 12.5 Examples are the vectors for endpoint rules

`tests/rules/endpoints/` holds contracts whose examples cover §9, §10 and §11. Every server target runs them through `veld test --live` against its own generated test handler with stub application handlers, and the offline run covers the reference evaluator. A target passes when every example passes.

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
- **One-time change to existing output:** `ir_version` becomes `1.1`: every stored IR (`veld.lock.ir.json`, IR and OpenAPI snapshots) changes by its version line. The prelude gains `error InvalidTransition 409 { field: string, from: string, to: string }`, which changes nothing stored, because a declaration appears in an IR only when a contract reaches it.

### 13.1 Endpoint rules and examples (Phase 2)

Everything here is omitted from the JSON when empty.

```jsonc
Endpoint = { …, "rules": EndpointRules }
EndpointRules = {
  "facts":    [ { "name": "room", "type": Type, "optional": false, "key": Expr | null, "origin": Origin } ],   // load order
  "requires": [ { "name": "notInPast" | null, "id": "veld:bookings/Bookings.createBooking.notInPast@…",
                  "expr": Expr, "message": "…" | null, "reads": ["body"],                 // parameter names
                  "else": { "decl": DeclId, "status": 409,
                            "fields": [ { "name": "capacity", "wire": "capacity", "value": Expr } ] } | null,
                  "docs": .., "origin": Origin } ],                                         // source order
  "ensures":  [ { "name": null, "id": "…ensure@…", "expr": Expr, "message": null, "reads": [..],
                  "docs": .., "origin": Origin } ]
}

Service = { …,
  "facts": [ { "name": "room", "type": Type, "key": Type | null, "optional": false } ],     // one loader each, sorted by name
  "rules_digest": "3fa9c2e1b7d04a55" }                                                     // §9.7

Ir = { …, "examples": [ Example ] }
Example = {
  "name": "a full room is rejected", "package": PackageId, "service": ServiceId, "endpoint": "createBooking",
  "givens": [ { "fact": "room", "value": ExampleValue | null } ],     // null is `absent`
  "principal": ExampleValue | null, "now": "2026-03-01T09:00:00Z" | null,
  "args": [ { "param": "body", "value": ExampleValue } ],
  "returns": ExampleValue | null,
  "outcome": { "ok": ExampleValue | null } | { "error": { "decl": DeclId, "pattern": ExampleValue | null } },
  "docs": .., "origin": Origin
}
ExampleValue = { "json": <a wire value> }                          // a literal, already typed
             | { "ref": { "given": "room", "path": ["id"] } }       // wire names
             | { "object": [ { "name": "guests", "wire": "guests", "value": ExampleValue } ] }
             | { "list": [ ExampleValue ] }
```

- An endpoint with a required fact has `NotFound` among its `errors`, like the implicit `ValidationError`.
- `Feature::RulesEndpoint` is used when an endpoint of the requested services has `rules`. Examples need no feature: they are read by `veld test`, not by generators.
- Filling in what an example leaves out (§12.2) happens when the example runs; the IR holds only what was written.

## 14. Diagnostics

Range `E09xx` (errors) and `W09xx` (warnings). Codes are never reused.

| Code | Meaning |
|---|---|
| `E0901` | Unknown name in an expression, or a lambda variable that shadows another name |
| `E0902` | Type error in an expression: operand types do not fit the operator, a rule that is not `bool`, a chained comparison, an enum literal with no enum to infer |
| `E0903` | An optional value is read where it is not known present (§3.6) |
| `E0904` | `now`, `result` or `old` used where it is not available |
| `E0905` | Operation not supported on this type: `/` or `%` on decimal, ordering strings, comparing models, lists, maps, bytes or json, a `const` of an unsupported type |
| `E0906` | Unknown function, wrong number or type of arguments, lambda where none is allowed, `has` on a path that can never be absent |
| `E0907` | Duplicate rule name in a declaration, counting inherited rules |
| `E0908` | Numeric literal out of range for its type |
| `W0901` | `==` or `!=` on `float` |
| `W0902` | Nested comprehension over a list without `@length(max:)` |
| `W0903` | A rule that reads no field |
| `W0904` | A target with request validation turned off reaches rules, which it will not enforce (§5.3) |
| `E0920` | A fact named like a parameter, another fact, the principal, `now` or `result` |
| `E0921` | The error after `else` is not one of the endpoint's declared errors |
| `E0922` | Two facts with one name but different types, key types or optionality in a service; an unknown or missing field in an `else` payload |
| `E0923` | A fact read in `ensure` outside `old(...)` |
| `E0924` | `ensure` on an endpoint whose success is a stream |
| `E0950` | Duplicate example name; unknown or ambiguous endpoint in an example |
| `E0951` | `given` names something that is not a fact of the called endpoint |
| `E0952` | A value in an example does not fit the type it is used at |
| `E0953` | A value the example leaves out cannot be filled in, or the filled-in value breaks an invariant |

`E0930` to `E0939` are reserved for state machines and `E0940` to `E0949` for policies.

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

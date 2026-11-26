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
ifMatch any stored other running
```

Disambiguation, with at most three tokens of lookahead:

| Where | Tokens | Meaning |
|---|---|---|
| model or error body | `rule` IDENT `:` | a rule. `rule: T` and `rule?: T` are fields named `rule`. |
| enum body | `flow` IDENT `->` | a flow statement (§10). Otherwise `flow` is a variant. |
| enum body | `final` IDENT on the same line | a final statement (§10). Otherwise `final` is a variant. |
| after an expression | `in` or `implies` followed by `:` or `?` | the start of a member named `in` or `implies`, not an operator. |
| top level | `policy` IDENT `(`, `principal` IDENT, `example` STRING | declarations (§11, §12). |
| example body | `ifMatch` followed by an integer or a name | the `ifMatch` line (§11.9). Followed by `(` it is the call of an endpoint called `ifMatch`. |
| example body | `key` followed by a string | the `key` line (§11.9). Otherwise `key` is a name. |
| after `key` STRING | `stored`, `other` or `running` followed by a name | the word. Followed by `(` or `.` it is a call, so an endpoint may be called `stored`. |
| after `stored` and its outcome | a value on the line of the outcome | the value of the outcome. A name on the next line is the call. |

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

**Line width.** The formatter wraps at 100 columns, for these constructs only: a rule, `require`, `ensure`, `permit` or `forbid` whose expression is wider is broken before the binary operators of its lowest precedence level, the continuation lines indented four spaces more; an example's call has one argument per line, a record or list of an example one member per line, and the outcome after `->` hangs on the call or goes to the next line; a saga step's call is broken the same way. A line that fits is never changed.

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
5. **invariants on the bound parameters** → `422 ValidationError` with `violations` (§7). The same step refuses a malformed `If-Match` or idempotency key, and a required key that is missing, with `422` and no `violations` (§11.9); after it comes the idempotency lookup, which may end the request (§11.9)
6. facts, policies, version guard, transition guard, `require`, in that order (§9, §10, §11)
7. the handler
8. **invariants on the result**, according to the output mode
9. `ensure` and the target state (§9, §10)
10. serialise

**Request validation turned off.** Rules need well-formed values. A target option that turns request validation off (`validate = false`) therefore turns off step 5, every `require` of step 6, step 9, and the client's local check with them. Three things in step 6 still run: the facts are loaded (the handler still receives them), and **the policies and the transition guard are still applied**, because who may call an endpoint and which move is legal must not depend on a validation option. An `allow` argument that cannot be evaluated denies, as always (§11.3). A server built that way says so once, when its handler is created, so nobody runs unchecked by accident. `veld generate` warns (`W0904`) when such a target reaches a declaration or endpoint with rules.

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
- A server of a member with message catalogs (`spec/platform.md` §22) adds `messageKey` to each entry, and writes `message` in the language of the request.
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

A statement starts with one of the five words; nothing else may appear in the block. `allow` is §11 and `transition` is §10. The formatter prints the statements in this order: facts, `allow`, `transition`, `require`, `ensure`, each group in source order.

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

Steps 1 to 5 and 7, 8, 10 are those of §5.3. Step 6 is: read the clock; load the facts in order (`404`); policies (`403`, §11); version guard (`412`, §11.9); transition guard (`409`, §10); `require` in order; save every `old(...)`. Step 9 is: `ensure` in source order; the target state of a transition (§10).

The server's options gain the fact loaders (required by the type system when the service has facts) and a `clock`. The default clock is the system clock.

### 9.6 Identity

```
rule_id    = "veld:" package "/" Service "." endpoint "." name "@" hex16( sha256( rule_bytes ) )
rule_bytes = RFC 8785 JSON of { "d": "<qualified service name>.<endpoint>", "k": "require" | "ensure", "n": name, "e": ExprShape }
```

`name` is the statement's name, or `require` or `ensure` for an unnamed statement. The `else` error, its payload and the message are not part of the id. In `ExprShape` a parameter is `{ "param": name }` and a fact is `{ "fact": name }`.

### 9.7 Stale clients

A client checks invariants locally (§5.4). If the server's rules have been loosened since the client was generated, that check would reject requests the server now accepts. The **rules digest** prevents it.

- `digest = hex16( sha256( RFC 8785 JSON array of the sorted, distinct rule ids of every invariant of every declaration the service's endpoints reach ) )`. It is computed by the compiler, stored on the service in the IR, and embedded in the generated client and server. Facts do not count: a client never checks them. A service that reaches no invariant has none. The digest does not depend on endpoint rules: a contract whose only rules are invariants has one, and its clients need it just the same.
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

service Bookings at "/bookings" {
  post "/{id}/pay" payBooking(id: uuid, body: Payment) -> Booking | AmountMismatch {
    fact booking: Booking = id
    transition booking.status: pending -> paid
    require body.amount == booking.total else AmountMismatch { expected: booking.total }
  }

  post "/{id}/cancel" cancelBooking(id: uuid) -> Booking {
    fact booking: Booking = id
    transition booking.status: pending, paid -> cancelled
  }
}
```

```ebnf
enum        = "enum" IDENT "{" { variant [","] | flow | final } "}" ;
flow        = "flow" IDENT "->" IDENT { "->" IDENT } ;
final       = "final" IDENT { "," IDENT } ;
transition  = "transition" path ":" IDENT { "," IDENT } "->" IDENT ;
```

`flow` and `final` are recognised by the lookahead of §2.2, so an enum may still have variants named `flow` and `final`. `flow a -> b emits Event` belongs to events (`spec/events.md` §11): the event is part of the move, and is no part of the transition's id or of the enum's signature.

### 10.1 The machine

- An enum with at least one `flow` is a **state machine**. `flow a -> b -> c` declares the edges `a -> b` and `b -> c`. An edge may be declared once; a second declaration is `W0930`. `a -> a` is a legal edge.
- Every name in a `flow` or `final` MUST be a variant of the enum (`E0930`).
- A **final** state has no way out: a `final` state with an outgoing edge is `E0931`. `final` without any `flow` in the enum is `E0931` too.
- `veld check` warns about a state that is not final and has no outgoing edge (`W0931`, a stuck state) and about a variant of a state machine that takes part in no flow and is not final (`W0932`).
- Flows do not change the enum's signature, like rules.

### 10.2 `transition`

- `transition booking.status: pending, paid -> cancelled` names a field of a **fact**. The path MUST start at a fact, end in a field whose type is a state machine, and pass through no optional step (`E0932`).
- Every pair of a listed source and the target MUST be an edge of the machine (`E0933`). An endpoint has at most one `transition` (`E0934`).
- **Guard, before the handler** (step 6, after the policies, before `require`): if the field's current value is not one of the sources, the answer is `409 InvalidTransition` with `field` set to the wire path of the field inside the fact (`status`), `from` to the current wire value and `to` to the target's wire value, and one entry in `violations`: `rule` is the transition's id, `path` is `request`, `fields` is empty and `message` is `violates rule transition`. An endpoint with a `transition` implicitly declares `InvalidTransition`.
- **Target, after the handler** (step 9, after `ensure`): when the success type is the same declaration as the fact's type, the same field of the result MUST hold the target state. This is an output check under the output mode.
- **This is a guard, not an atomic guarantee.** A second request can change the row after the check. The handler therefore receives the transition as data, next to the parameters and the facts, under the name `transition`: the field's wire path, the value it had when it was checked, and the target. The two states are values of the enum's generated type, not wire strings; a fact that leaves the field out has its default. Where a handler takes separate arguments, `transition` follows the facts. It MUST apply the change with a conditional write (compare-and-swap or a row lock) in its own transaction, and answer `InvalidTransition` itself when that write finds another value. Documentation and diagnostics say "guard", never "guarantee". A parameter or fact named `transition` on such an endpoint is `E0920`.
- **Lint `L0018` `post-moves-state-as-created`** (a warning by default; `[lint]` changes the level): a `post` with a `transition` whose success type is the fact's own type and whose success status is `201`, the default of every `post`. The move changes something that exists, so the contract almost always wants `-> 200 <Type>`; the help says so. The IR does not remember whether `201` was written, so an author who means it turns the rule off.
- Identity: `veld:<package>/<Service>.<endpoint>.transition@<hex16>` over the RFC 8785 JSON of `{ "d": "<qualified service name>.<endpoint>", "k": "transition", "f": "<fact>", "p": [<wire names of the path>], "s": [<source wire values, sorted>], "t": "<target wire value>" }`.

### 10.3 Generated helpers

Every target, clients included, gets for each state machine:

- the states that can follow a state (`nextStates(.pending)` gives `paid` and `cancelled`, in declaration order of the edges), and
- whether a state is final.

A client uses them to show only the buttons the server would accept. The server still decides.

### 10.4 In the IR

```jsonc
Enum = { "kind": "enum", "variants": [..],
         "flows":  [ { "from": "pending", "to": "paid" } ],     // variant names; first declaration order, no duplicates
         "finals": [ "checkedIn", "cancelled", "refunded" ] }   // declaration order
EndpointRules = { …, "transition": { "id": "veld:…transition@…", "fact": "booking",
                   "steps": [ { "name": "status", "wire": "status", "optional": false } ],
                   "decl": DeclId,                               // the state machine
                   "from": [ "pending", "paid" ], "to": "cancelled",       // variant names, sources in source order
                   "origin": Origin } | null }
```

`flows`, `finals` and `transition` are omitted from the JSON when empty or absent. `Feature::RulesTransitions` is used when a reachable enum has flows or an endpoint has a `transition`.

### 10.5 In examples

An example states an illegal move like any other outcome: `given booking = { status: .paid }` and `payBooking(id: booking.id, body: { … }) -> InvalidTransition { from: "paid", to: "paid" }`. A left-out field whose type is a state machine is filled in with the enum's first variant, as for any enum.

## 11. Policies (Phase 4)

```veld
principal User

policy ManageBooking(user: User, booking: Booking) {
  permit when booking.guestId == user.id
  permit when "support" in user.roles
  forbid when user.suspended
    message "A suspended account cannot manage bookings"
}

@auth(bearer)
service Bookings at "/bookings" {
  post "/{id}/pay" payBooking(id: uuid, body: Payment) -> Booking | AmountMismatch {
    fact booking: Booking = id
    allow ManageBooking(user, booking)
    transition booking.status: pending -> paid
    require body.amount == booking.total else AmountMismatch { expected: booking.total }
  }
}
```

```ebnf
principal  = { doc } "principal" typeref [ "as" IDENT ] ;
policy     = { doc } "policy" IDENT "(" pparam { "," pparam } [","] ")" "{" { clause } "}" ;
pparam     = IDENT ":" type ;
clause     = ( "permit" | "forbid" ) "when" expr [ "message" STRING ] ;
allow      = "allow" IDENT "(" [ expr { "," expr } [","] ] ")" ;
```

### 11.1 The principal

- A package declares at most one principal (`E0941`), and its type MUST be a model (`E0940`).
- In the endpoint blocks of that package the caller is in scope under the name `user`, or the name after `as`. It is available only where the endpoint's effective auth is not `none` (`E0904` otherwise). A parameter or fact with the principal's name on such an endpoint is `E0920`.
- Its value is what the application's auth hook returned for the request. In every target the hook's result has the generated type of the principal's model. It is a trusted value, like a fact: it is not validated.
- In an example, `as { … }` has the principal's type; an example that leaves it out for an endpoint with auth gets a filled-in principal (§12.2).

### 11.2 Policies

- A policy is a top-level declaration, named and imported like any other. Its clauses see only its parameters and `now`; the caller reaches a policy as an argument, never implicitly.
- **A policy permits when at least one `permit` is true and no `forbid` is true.** The order of the clauses does not matter.
- A clause that cannot be evaluated (§3.7) fails closed: such a `permit` does not permit, and such a `forbid` forbids.
- A policy with no `permit` permits nobody (`W0940`).
- Identity: `veld:<package>/<Policy>@<hex16>` over the RFC 8785 JSON of `{ "d": "<qualified name>", "c": [ { "k": "permit" | "forbid", "e": ExprShape } … ] }`, clauses in source order; a parameter is `{ "param": name }`. Messages are not part of it.

### 11.3 `allow`

- `allow P(args)` calls the policy `P` with the given arguments (`E0942` for an unknown policy, the wrong number of arguments or an argument of the wrong type).
- It runs in step 6, after the facts and before the transition guard. Several `allow` lines run in source order and all must permit; the first denial ends the request.
- A denial is **`403 Forbidden`**, which the endpoint then declares implicitly. Its `violations` has one entry: `rule` is the policy's id, `path` is `request`, `fields` are the parameters the arguments read, and `message` is the message of the first `forbid` that was true if it has one, otherwise `not permitted`. The error's own `message` field carries the same text.
- Facts load before policies, because a policy usually needs them. A caller without permission can therefore tell a missing record (`404`) from a forbidden one (`403`); `@tenant` (platform features) is the tool that hides records.

### 11.4 Generated code

- Every target, clients included, gets one function per policy that an `allow` of the generated services names, with the policy's parameters and a boolean result. A policy that no such endpoint uses generates nothing, so declaring one changes no output. The function reads the system clock for `now`; the server calls a form that takes the request's clock and also gives the message of the denial.
- For every endpoint with an `allow` or a `transition`, clients get **`can<Endpoint>(…)`**: it takes the facts, the principal and the parameters that the endpoint's `allow` arguments and its `transition` read, and answers whether the server would let the call past the policies and the transition guard. It never rejects a request; it is for showing or hiding a button. The server decides.
  - Its arguments are only what is read, in this order: the facts in declaration order, then the principal, then the parameters in declaration order.
  - It reads the system clock, and an evaluation error answers `false`.
- The target check of a transition (§10.2) is an `ensure` like any other: it runs after the written ones and follows the output mode.
- `Feature::RulesPolicies` is used when an endpoint has an `allow`. A policy that no endpoint uses needs no feature.

### 11.5 In the IR

```jsonc
Ir      = { …, "policies": [ Policy ] }                                        // sorted by qualified name
Policy  = { "name": "ManageBooking", "package": PackageId, "module": "…", "qualified": "…",
            "id": "veld:bookings/ManageBooking@…",
            "params": [ { "name": "user", "type": Type } ],
            "clauses": [ { "effect": "permit" | "forbid", "expr": Expr, "message": "…" | null, "origin": Origin } ],
            "docs": .., "origin": Origin }
Service = { …, "principal": { "decl": DeclId, "name": "user" } }
EndpointRules = { …, "allows": [ { "policy": <index into Ir.policies>, "args": [Expr], "reads": ["id"], "origin": Origin } ] }
```

All three are omitted from the JSON when empty or absent. In a policy's expressions a parameter is the root `{ "param": name }`; in an endpoint block the caller is the root `"principal"`.

### 11.6 A lint for public endpoints

`auth-required` (`L0014`, default `warn`): an endpoint whose effective auth comes from nowhere, neither its own `@auth` nor its service's, should say `@auth(none)` if it is meant to be public.

The code is `L0014`: `L0005` is `missing-description` since v2.0 and codes are never reused. The IR does not tell an `@auth(none)` that the author wrote from the default, and it must not grow a key for it (the JSON of every existing contract stays the same). The compiler therefore marks the `Auth` of an endpoint or service as *declared* when an `@auth` was written on it or on its service; the mark is not part of the JSON, and only this lint reads it, in the process that compiled the contract.

### 11.7 Optimistic concurrency: `@version`

- `@version` marks one `int` or `long` field of a model as its version (`E0602` on another type or a second field).
- A response whose success type is such a model carries `ETag: "<version>"`.
- On an endpoint with a fact of such a model (the first such fact, in declaration order), a request MAY send `If-Match`. If it does and the value is neither `*` nor the fact's current version, the answer is **`412 PreconditionFailed`**, a new prelude error with `expected` and `actual`, which such an endpoint declares implicitly. The check runs after the policies and before the transition guard.
- Like a transition, this is a guard: the handler receives the version that was checked and MUST write conditionally, answering `PreconditionFailed` itself when the row has moved on.
- Generated client methods of such endpoints take an optional `ifMatch`.

### 11.8 Idempotency: `@idempotent`

- `@idempotent` on an endpoint, optionally `@idempotent(header: "Idempotency-Key", required: true)`. The default header is `Idempotency-Key` and it is not required.
- The server gets an `IdempotencyStore` interface to implement, with a memory implementation for tests. When a request carries the header, the store is asked after step 5: a stored answer for the same key is replayed (status, headers that matter and body) without loading facts or calling the handler; otherwise the request runs and its answer is stored. Success answers and declared errors are stored; `5xx` answers are not.
- The key is scoped to the operation and, when the endpoint has auth, to the caller: one caller's key never replays another caller's answer.
- The same key with different parameters, or while the first request is still running, is `409 Conflict`.
- With `required: true`, a request without the header is `422 ValidationError`.

### 11.9 Version and idempotency: the exact behaviour, examples and the test handler

**Status: implemented outside the generators.** The language, the checks, the IR, the reference evaluator (`crates/veld-rules`), `veld test`, `veld fmt`, `veld diff`, the OpenAPI export and the language server know both annotations. No generator implements them yet: `rules_version` and `idempotency` are opt-in features (§13), so a generator that does not declare them reports `E0801` at the first use. Sections 11.7 and 11.8 say what the two annotations are for. This section fixes what a server does, so that an example can state it and three generators can be held to it. §11.10 records what the implementation chose where the text was silent, and §11.11 how the tools judge the two annotations.

**`@version`**

- The prelude gains `error PreconditionFailed 412 { expected: long, actual: long }`.
- `If-Match` is read as a header of the request. It MUST be `*`, or one strong entity tag whose text is the version in decimal digits (`"3"`). Anything else, a weak tag or a list included, is `422 ValidationError` with one issue at `header.If-Match`.
- The check runs in step 6, after the policies and before the transition guard. `*` passes. A tag that is not the fact's current version answers `412 PreconditionFailed` with `expected` (the version the caller sent) and `actual` (the fact's version), and one violation: `rule` is `veld:<package>/<Service>.<endpoint>.version@<hex16>` over the RFC 8785 JSON of `{ "d": "<qualified service name>.<endpoint>", "k": "version", "f": "<fact>", "p": [<wire names of the path to the version field>] }`, `path` is `request`, `fields` is empty and `message` is `violates rule version`.
- A request without `If-Match` is not checked. The handler receives, next to the facts and under the name `version`, the version that was checked, or nothing when the header was absent; like a transition it is a guard, and the handler MUST write conditionally.
- A success response whose type is a model with a version field carries `ETag: "<version of the returned value>"`.
- In an example, `ifMatch 3` or `ifMatch any` stands before the call. It is `E0952` on an endpoint without such a fact. A live run sends the header as above.

**`@idempotent`**

- The store holds, per **scope** and key, a **fingerprint** and either "running" or a stored answer.
  - The scope is the operation signature and, when the endpoint's effective auth is not `none`, the caller's identity: a text the application gives through a hook of the server. A server whose endpoint needs it and has no such hook does not start.
  - The fingerprint is the first 16 hex characters of the SHA-256 of the RFC 8785 JSON of the bound parameters in their canonical wire form (§11.10, "What is hashed"), keyed by parameter name. The key's own header is not part of it.
  - A stored answer is the status, the body bytes, the content type, and the `ETag` and `Location` headers when present.
- With a key, after step 5 the server asks the store:
  - nothing stored: it records "running" with the fingerprint, runs the rest of the pipeline, stores the answer when the status is below `500`, and removes the entry otherwise;
  - a stored answer with the same fingerprint: that answer is sent again, with `Veld-Idempotent-Replay: true`; no fact is loaded and the handler is not called;
  - "running", or another fingerprint: `409 Conflict`, whose `message` says which of the two it was.
- How long an entry is kept is the store's business. The generated memory store keeps everything and is for tests.
- In an example, a `key` line stands before the call and states the header and what the store holds for it:

  ```veld
  example "a key that was used replays its answer and loads nothing" {
    given room = absent
    key "k-1" stored RoomFull { capacity: 2 }
    createBooking(body: { guests: 1 }) -> RoomFull { capacity: 2 }
  }

  example "the same key with other parameters is a conflict" {
    key "k-1" other
    createBooking(body: { guests: 1 }) -> Conflict
  }
  ```

  ```ebnf
  example = { doc } "example" STRING "{" { given } [ "as" value ] [ "at" STRING ]
            [ "ifMatch" ( INT | "any" ) ] [ key ] call "}" ;
  key     = "key" STRING [ "stored" outcome | "other" | "running" ] ;
  ```

  `key "k"` alone is a key the store does not know. `stored` gives the answer the store holds for this key and these parameters, written like an outcome; `other` says the store holds the key with another fingerprint; `running` says the first request has not finished. A `key` line on an endpoint without `@idempotent` is `E0952`. `ifMatch`, `key`, `any`, `stored`, `other` and `running` are contextual words.
- The test handler takes the store's state for the request from the header `Veld-Test-Idempotency`: unpadded base64url of `{ "state": "stored", "status": 409, "type": "<error signature>" | null, "data": { … } }`, `{ "state": "other" }` or `{ "state": "running" }`. Without the header the store is empty. The key itself travels in its ordinary header.
- Capability flags: `rules_version` and `idempotency`, each `E0801` in a generator that does not implement it.

### 11.10 What the implementation chose where the text was silent

**`@version`**

- A `@version` field is required (not `field?`): an optional one is `E0602`, like a field of an error or an event, and a second one in a model, an inherited one counted (reported once, at the own field). `@version` may stand on a `@readonly` field.
- The guard is the first fact, in declaration order, whose type is a model with a version field. An optional fact counts; when it is absent there is nothing to compare and the request passes. The path of the rule id is the one wire name of the field.
- A parameter bound to `If-Match` on an endpoint with a guard is `E0602`: the guard reads the header itself.
- `expected` and `actual` are `long` and therefore decimal strings on the wire, also when the version field is an `int`.
- A version tag is `0`, or a digit from 1 to 9 followed by digits, no sign, within `long`. `"03"`, `"-1"`, `W/"3"`, a list, a number beyond `long` and a header with spaces around it are malformed. A malformed `If-Match` on an endpoint with a guard is refused at step 5 with the issue `{ "path": "header.If-Match", "code": "type", "message": "must be * or one strong entity tag holding a version, such as \"3\"" }`, next to the other issues of that step, header issues first. It carries no `violations`: it is not a rule. An endpoint without a guard ignores the header.
- `validate = false` turns off neither the guard nor the syntax of `If-Match` and of the key: they are part of what the endpoint does, not validation of its parameters.
- A parameter or a fact called `version` on an endpoint with a guard is `E0920`: the handler gets the checked version under that name, as it gets the transition under `transition`.
- `*`, like no header, hands the handler no version to write against. A version is handed over only when the caller sent a tag and it was compared: a guarded fact that is absent has nothing to compare and nothing is handed over.
- The `ETag` is on the success whose type is the model itself (not a list, a `Page` or a union), whatever the 2xx status, and on a replayed success. `rules_version` is used by a service with a guard and by one that has such an endpoint; the first use is the fact of the guard, or the endpoint for a response that only owes the `ETag`.

**`@idempotent`**

- The arguments are `header:` (a non-empty HTTP token, each given once) and `required:`. `E0602` for a header that is not a token, for `Authorization`, `Content-Type`, `Content-Length`, `Accept`, `Host`, `If-Match`, `ETag`, `Veld-*`, for one that an `@header` parameter or the `apiKey` of the endpoint uses (compared without case), and on a stream or `ws` endpoint: there is no single answer to store. `get` is allowed.
- A key is 1 to 255 characters of visible ASCII (0x21 to 0x7E). Another key is refused at step 5 with `{ "path": "header.<Name>", "code": "type", "message": "must be 1 to 255 visible ASCII characters" }`, and a required key that is missing with `{ "path": "header.<Name>", "code": "required", "message": "is required" }`, without `violations`. These come before the store is asked, so a request that is not valid is never replayed.
- The message of the `409 Conflict` is `this idempotency key was used with other parameters` for another fingerprint and `a request with this idempotency key is still running` for a request that has not finished. The error is the prelude's own `Conflict 409 { message: string }`; the endpoint declares it implicitly, as it declares `ValidationError`.
- A replayed answer carries no `violations`; for a success it is the stored body, with the `ETag` of that body. A replay does not check `If-Match` and does not evaluate any rule: the lookup is before step 6. The answers stored are the success and every error below 500, those of step 6 included (`404`, `403`, `412`, `422` of a `require`); a `500` removes the entry.
- In an example, the value after `stored` is the stored body in full: what it leaves out is filled in (§12.2), and the outcome of the call is compared with it as a pattern. `stored ok` without a value stores a filled-in value of the success type. `stored` of an error whose status is 500 or more is `E0952`, because such an answer is never stored. A `stored` value must start on the line of its outcome. A replay is compared with the stored body, not with `returns`.
- The scope of a key includes the caller only when the endpoint's effective auth is not `none`. An example states what the store holds for the scope of its request, so it cannot show that the key of another caller is not replayed; the tests of each generator do.
- The caller text, when the application does not give it (the reference evaluator, the test handler), is the RFC 8785 JSON of the principal, or `caller`.
- **What is hashed.** The fingerprint is taken from the bound parameters after step 5 (`Reported.fingerprint`), in the canonical form of the request as the server understands it. It is not part of an example: the state of an example says what the store holds for it. The goldens are `tests/wire/idempotency/fingerprints.json`, which `crates/veld-rules/tests/idempotency_fingerprints.rs` holds to the reference (`idempotency::hashed_params`), and the older ones in `crates/veld-rules/src/idempotency.rs` and `tests/version_idempotency.rs`. A target reproduces the object byte for byte:
  - **Keys and position.** One member for each non-stream parameter that was sent or has a value, keyed by the **parameter name** (not the name of a header or of a query key), whatever its position: path, `@query`, `@header`, `body`, and the `query` model as one member `query` that holds its fields. A `@header` parameter is part of it; the header of the key, and every other header, is not.
  - **Absent.** A parameter that was not sent is not in the object, and an optional member of a body (or of any nested value) that was not sent is not either: nothing is hashed as `null` for it. A default is applied before hashing, at every depth (body, nested model, item of a list, value of a map). A derived field is the value the server computed (§17.2). `null` is a value only for a `json` member; for any other optional it is `422` (language.md §5.3) and never reaches the fingerprint.
  - **Dropped.** A member the model does not have, and a `@readonly` member the caller sent, are not part of it, at any depth: the server ignores both, so they are not parameters. (The discriminator of a union is a member and stays.)
  - **Text.** A path, query or header value is text. Binding parses it into the typed value, and the wire form of that value is hashed: an `int` and a `float` as a JSON number (`007` and `7` are 7; `7.0` and `7` are 7; `7.50` is 7.5), a `long` as the canonical decimal string of its value (`007` is `"7"`, `-0` is `"0"`), a `bool` as `true` or `false`, a list in a query as an array of such values in the order the keys came. The reference has no text binder (its input is already wire form); the test of the reference binds the goldens this way.
  - **Canonical scalars.** A `uuid` is in lower case. A `datetime` is in UTC with `Z`, the fraction `.fff` only when the milliseconds are not zero (`2026-06-10T14:00:00+02:00`, `2026-06-10T12:00:00.000Z` and `2026-06-10T12:00:00Z` are one value; `.25Z` is `.250Z`). This is the canonical form of an event's `data` (`events.md` §6.3, `veld_rules::envelope`), and a target uses the same function. A `long` is also in the canonical decimal form inside a body: every target accepts `"007"` there (`^-?\d{1,19}$`), so it is hashed as `"7"`. Every other scalar is the text as sent: `decimal` `1.50` and `1.5` are two values, and `date`, `time`, `bytes`, a `string` and the wire value of an enum are as written. A JSON number is hashed by its value in the RFC 8785 form (`1.5`, `1.50` and `15e-1` are one value).
  - **Composites.** The members of an object are sorted by the canonical form, so a body in another member order, and a map whose keys came in another order, are the same value. A list keeps its order; an empty list (or map) is hashed as `[]` (or `{}`), not left out.
  - **The hash.** The RFC 8785 text of that object, UTF-8; the first 16 hex characters of its SHA-256. A request with no parameter at all hashes `{}` (`44136fa355b3678a`).
  - *A limit of the reference.* It does not check the text of a scalar (as in `events.md` §6.3), so it cannot refuse `null` for an optional `string`; `tests/wire/idempotency` has that case as `rejected` and does not run it.
- An offline run without `returns` stops before the handler: the entry of a new key is still "running" (the reference evaluator says so in `Reported.store`).
- A dry run (§12.4) stops after step 6, and a replay is answered before it: `Veld-Test-Dry-Run: 1` with a stored answer gets the stored answer, with its status and body, not `204` and `Veld-Test-Passed`. `veld test --live --dry-run` expects that. A stored success without a body is sent to the test handler as `"data": null`.

**The reference evaluator** (`veld_rules`): `Request` has `if_match`, `key` and `caller`; `Reported` has `replayed`, `store` (`Untouched`, `Stored`, `Removed`, `Running`), `checked_version`, `body` and `etag`; `parse_if_match`, `etag`, `fingerprint` and `Scope::of` are public. The pipeline asks the store after step 5 and runs the version guard after the policies and before the transition.

### 11.11 `veld diff`, the export and the language server

`veld diff` judges each change by what it does to a caller built against the old contract:

| Change | Verdict | Why |
|---|---|---|
| `@version` added to a field (`VersionAdded`) | non-breaking | responses gain an `ETag`; a request without `If-Match` is not checked |
| `@version` removed (`VersionRemoved`) | breaking | a caller that sends `If-Match` loses the protection of its write, and the `ETag` is gone |
| another field is the version (`VersionFieldChanged`) | breaking | the tags callers hold count something else |
| an endpoint gets a guard (`GuardAdded`) | non-breaking | only a request that sends `If-Match` is checked |
| an endpoint loses its guard (`GuardRemoved`) | breaking | `If-Match` is ignored: a write the caller thought protected is not |
| the guard compares another fact or field (`GuardChanged`) | breaking | the tags are compared with something else |
| `@idempotent` added (`IdempotencyAdded`) | non-breaking, breaking with `required: true` | a request without the key is as before, unless it is now `422` |
| `@idempotent` removed (`IdempotencyRemoved`) | breaking | a retry with the same key runs again |
| `required` becomes true (`IdempotencyRequiredChanged`) | breaking | a request without the key is `422` |
| `required` becomes false | non-breaking | |
| the header changes (`IdempotencyHeaderChanged`) | breaking | callers send the old header, which is ignored, or `422` when it is required |

The `PreconditionFailed`, `Conflict` and `ValidationError` that a feature adds to or takes from an endpoint belong to its change and are not reported again as `ErrorAdded` or `ErrorRemoved`; an error the author adds next to them is.

The OpenAPI export adds, for a guard, an optional `If-Match` header parameter and `x-veld-version` (`id`, `fact`, `field`) on the operation, and `x-veld-version` (the wire name) on the model; for `@idempotent`, the header parameter (`required` as written, a string of 1 to 255 characters), `x-veld-idempotent` (`header`, `required`, `caller`) and a `Veld-Idempotent-Replay` response header on the success and on every answer below 500; for a response that is a model with a version field, an `ETag` response header. The `412` and `409` responses are the endpoint's errors. A contract without the annotations exports the same bytes. The language server offers `@version` in a model body and `@idempotent` (and its arguments `header` and `required`) in a service body, and says what each does on hover.

### 11.12 Generated code: the names every target uses

Sections 11.9 and 11.10 fix what a server does. This section fixes what the generated code is called, so that the targets mirror each other. The spelling here is the TypeScript one; Java uses the same words in its casing and Rust in snake case (`IdempotencyStore`, `idempotency_key`). Each target's spec says in which file a piece lives. A contract without the annotations keeps its generated bytes in every target.

**`@version`, server**

- The handler receives the checked version under the name `version`, next to the facts and the transition. Where a handler takes separate arguments their order is the order in which the pipeline checks them: the facts, `version`, `transition`. It has the type of the version field in the target's developer-facing form and it is optional: it is absent without `If-Match`, with `*`, and when the guarded fact is absent (§11.10). Its doc comment says what the doc comment of a transition says: the check is a guard, not a guarantee; write conditionally, and answer `PreconditionFailed` yourself when the row has moved on.
- The runtime reads and checks `If-Match` and writes `ETag`. Application code touches neither header.
- A `412` goes to `onRuleViolation` with position `input` and outcome `rejected`, like a refused transition.

**`@version` and `@idempotent`, client**

- A client method of an endpoint with a guard takes an optional `ifMatch`: a version, typed like the version field, or the target's constant for `*` (`'*'` in TypeScript, `IfMatch.any()` in Java, `IfMatch::Any` in Rust). The client writes the header as §11.9 defines it.
- A client method of an `@idempotent` endpoint takes `idempotencyKey`, a string: optional, and required when the annotation says `required: true`. The client sends what it is given and never invents a key.
- Both stand where the target puts a header parameter of the endpoint, after the declared parameters. A declared parameter with one of these two names on such an endpoint is `E0801` in a generator.
- A client checks the syntax of neither value: the server answers `422` for a key or a tag that is malformed.

**`@idempotent`, server**

- `IdempotencyStore` has three operations, all asynchronous where the target is:
  - `begin(scope, key, fingerprint)` answers one of `started`, `replay` (with the stored answer), `running` and `other`. It is atomic: of two concurrent calls for a key the store does not know, exactly one gets `started`. A key held with another fingerprint is `other`, whether its request is running or its answer is stored.
  - `complete(scope, key, answer)` stores the answer of a started request.
  - `release(scope, key)` removes the entry of a started request.
- `StoredAnswer` is `status`, `body` (the bytes that were sent) and the optional `contentType`, `etag` and `location`.
- `scope` is one text, opaque to the store: the operation signature, and, for an endpoint whose scope includes the caller, a line feed followed by the caller text.
- `MemoryIdempotencyStore` is the generated implementation for tests. It keeps every entry for as long as it lives.
- The server's hooks gain `idempotency`: `store`, and `caller`, a function from the principal and the request context to the caller text. A service with an `@idempotent` endpoint needs `store`; it needs `caller` when one of those endpoints has `caller: true` in the IR. The type requires them where the target's types can, and creating the handler fails, naming the first endpoint that lacks what it needs, where they cannot. No server finds this out at its first request.
- The runtime calls `begin` after step 5, `complete` when the answer's status is below `500`, and `release` otherwise, also when the server itself fails after `begin`: an exception, a panic, a request that is cancelled or dropped before it has its answer. An entry MUST NOT stay "running" because the server gave up; where the language cannot run asynchronous code at that moment, the release is handed to a task of its own.
- **A store that fails.** When `begin` fails the answer is `500 Internal` and the handler is not called: running without the store could run a request twice. A `caller` hook that fails is treated the same way. When `complete` or `release` fails the answer is sent as it is and the failure goes to `onError`.
- **The test handler** (`testing = true`) gives every request a fresh memory store, puts into it what `Veld-Test-Idempotency` says (§11.9) under the fingerprint of the request (`other`: under another one), and uses the caller text of §11.10. A service with an `@idempotent` endpoint, a guard or an endpoint that owes an `ETag` gets a test handler in every target, also when it has no other endpoint rule: `veld test --live` must be able to reach it.
- **The fingerprint is the reference's.** Where a value can be spelt in several ways (a uuid in upper case, a datetime with an offset, a decimal with trailing zeros, a number in a path or a query string), every target gives the fingerprint the reference evaluator gives. §11.10 says what that is, and the goldens in `tests/wire/idempotency/` hold those cases; a target's own tests are not enough, because three targets can each agree with themselves.

**Capabilities.** A generator declares `rules_version` and `idempotency` for a role only when everything above and in §11.9 and §11.10 holds for that role.

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
example  = { doc } "example" STRING "{" { given } [ "as" value ] [ "at" STRING ]
           [ "ifMatch" ( INT | "any" ) ] [ key ] call "}" ;      (* ifMatch and key: §11.9 *)
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
- `as value` is the principal, a value of the principal's model (§11.1): `E0952` when it does not fit, and when the package declares no principal. `at "…"` is the clock; without it the clock is `2026-01-01T00:00:00Z`.
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
- **`veld test --live <base-url>`** sends each example as a real request to a running **test handler** (§12.4) and compares the status, the error and the patterns. An example writes down only part of an answer, so an answer that agrees with it is also held to the reference evaluator: when the evaluator answers the same request with an error, the server MUST give that whole error: the problem document of §7 with the media type `application/problem+json`, the same `type`, `title`, status and `operation`, the same payload and the same `violations`, each with the same `rule`, `path`, `fields`, `message` and `error`. Violations and issues are compared in order, or as a set when a parameter of the endpoint contains a map (§5.2). Every answer of a service that has a rules digest, a dry-run pass included, MUST carry it in `Veld-Rules` (§9.7). An example without `returns` runs the application's real handler, so `ok { … }` checks what it actually answered; with `--dry-run` such an example stops before the handler (§12.4).
- **`--fuzz N`** (with `--live`; `N` from 1 to 100000; `--seed S` to repeat a run) builds `N` requests per endpoint from the types, constraints and rules, valid and invalid, works out the expected outcome with the reference evaluator, and fails on any difference, compared as for an example. A failure prints the seed and the request in example syntax. A request of `--fuzz` sends no `If-Match` and no key, so an endpoint whose key is required answers `422` to every one of them and is named as an endpoint that ends one way. A run that cannot go on, because the server stopped answering, fails and still reports what it found before. An endpoint whose every request ended in the same error is named, because nothing after that check was reached.
- **Boundaries.** The boundary requests belong to `--fuzz`, not to a plain `--live` run: they are the first requests of a fuzz run. Every comparison of a number, a time or a length with a constant in a rule yields requests at the bound and one step on each side.

### 12.4 The test handler

A test must hand a server its facts, its principal and its clock. The production handler has no such door, and gets none.

- A server target with the option `testing = true` generates a **second entry point** in a file of its own, for services that have endpoint rules. Nothing in the production entry point changes.
- The test handler runs the same pipeline, but takes the facts from the request header `Veld-Test-Facts` (base64url of a JSON object: fact name to wire value, or `null` for absent), the principal from `Veld-Test-Principal`, the clock from `Veld-Test-Now` and, when present, the result from `Veld-Test-Result` in place of calling the application's handler. The application's loaders and auth hook are not called.
- With the request header `Veld-Test-Dry-Run: 1` the test handler stops after step 6: if every check passed it answers `204` with the response header `Veld-Test-Passed: 1` and does not call the application's handler. `veld test --live --dry-run` sends it for every example that has no `returns`, which makes a live run check exactly what an offline run checks. The shared vectors run this way against every server target.
- It answers `GET /__veld/test` with status `200` and `{ "veldTestHandler": 1, "digests": { "<Service>": "<rules digest>" | null } }`, one entry per service it serves. `veld test --live` refuses a server that does not answer exactly so, before it sends anything else, so a production server is never sent test headers. It sends the examples and the fuzz requests of a service only to a handler that names that service; the others are reported as skipped. A workspace with several services runs `veld test --live` once per test handler.
- **The request a live example sends** is the one a generated client would send (the endpoint's path without a trailing slash, path parameters percent-encoded, query pairs sorted by key, `Accept: application/json`, and the body in wire form), plus the test headers: `Veld-Test-Facts` is always there and is the unpadded base64url of a compact JSON object, `null` for an absent fact; `Veld-Test-Principal` and `Veld-Test-Result` are the unpadded base64url of the wire JSON and appear only when the example has `as` or `returns`; `Veld-Test-Now` is the clock as plain RFC 3339 text.
- **The answer** is judged like this: an error must be a problem document whose `type` is the error's declaration signature, with the status the endpoint declares for it; a success must have exactly the endpoint's success status.
- **Audited rules** (`@rollout(audit)`, §17.1). When a request made the server report audited rules to `onRuleViolation`, the answer of the test handler has the response header `Veld-Test-Audited`: the unpadded base64url of a compact JSON array with one entry per report, each with the members of a `violations` entry (§7): `rule`, `path`, `fields`, `message` and, for an evaluation error, `error`. The array holds at most the first 100 reports, and the header is absent when there is none. It is on every kind of answer: a success, an error, and the `204` of a dry run (which then lists the rules of steps 5 and 6 only). `veld test --live` compares the entries with what the reference evaluator reports for the same example, as a set of `(rule, path, error)`, fails the example when they differ, and prints them under the example as the offline run does; when the reference reports more than 100 it only requires every entry of the header to be one of them. `--fuzz` does not compare the header.
- **Flags** (`spec/platform.md` §11). The flags that are on come from the request header `Veld-Test-Flags`: the unpadded base64url of a compact JSON array of names. Without it every flag is off.
- **Version and idempotency** (§11.7 to §11.9). A live example sends `If-Match` (`*` or `"3"`) and the idempotency header with the key of its `key` line, and for a store that is not empty `Veld-Test-Idempotency`; the test handler uses that state instead of the application's store, and an absent header is an empty store. The caller text of a key is the RFC 8785 JSON of the principal of `Veld-Test-Principal`, or `caller` when the request has none; the application's hook is not called. An answer is also held to the headers of the reference evaluator: `Veld-Idempotent-Replay: true` on a stored answer that was sent again and on no other answer, and `ETag` on a success whose type is a model with a version field, equal to the version of what the handler returned (or of the stored body). `veld test --live` reports a difference like any other.
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
- `Feature` gains `rules_invariants`, `rules_endpoint`, `rules_transitions` and `rules_policies`, and later `rules_rollout` (§17.1), `rules_version` and `idempotency` (§11.9). The last three are opt-in.
- `Field` gains `"version": true`, `EndpointRules` gains `"version"`, `Endpoint` gains `"idempotency"` and `Example` gains `"if_match"` and `"key"` (`spec/ir.md` §3.5). Each is omitted from the JSON when absent.
- `Rule`, `EndpointRequire` and `EndpointEnsure` gain `"rollout": "audit"` for an audited rule (§17.1). It is omitted from the JSON when the rule is enforced.
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
| `E0920` | A fact named like a parameter, another fact, the principal, `now` or `result`; a parameter or fact named `transition` on an endpoint that has one, or `version` on an endpoint with a version guard |
| `E0921` | The error after `else` is not one of the endpoint's declared errors |
| `E0922` | Two facts with one name but different types, key types or optionality in a service; an unknown or missing field in an `else` payload |
| `E0923` | A fact read in `ensure` outside `old(...)` |
| `E0924` | `ensure` on an endpoint whose success is a stream |
| `E0950` | Duplicate example name; unknown or ambiguous endpoint in an example |
| `E0951` | `given` names something that is not a fact of the called endpoint |
| `E0952` | A value in an example does not fit the type it is used at, or writes a derived field (§17.2); an `ifMatch` line on an endpoint with no version guard, a `key` line on an endpoint that is not `@idempotent`, a key that is not 1 to 255 visible ASCII characters, a `stored` answer with a status of 500 or more or one the endpoint cannot give |
| `E0953` | A value the example leaves out cannot be filled in, or the filled-in value breaks an invariant |

| `E0930` | A `flow` or `final` names something that is not a variant of the enum |
| `E0931` | A `final` state with an outgoing flow, or `final` in an enum with no flow |
| `E0932` | The path of a `transition` does not start at a fact, does not end in a state machine, or passes through an optional step |
| `E0933` | A `transition` lists a move that is not a flow of the machine |
| `E0934` | More than one `transition` in an endpoint |
| `W0930` | The same flow declared twice |
| `W0931` | A state that is not final and has no way out |
| `W0932` | A variant of a state machine that takes part in no flow and is not final |

| `E0940` | The type after `principal` is not a model |
| `E0941` | More than one `principal` in a package |
| `E0942` | `allow` names an unknown policy, or its arguments do not fit the policy's parameters |
| `W0940` | A policy with no `permit` permits nobody |
| `L0014` | `auth-required`: an endpoint with no `@auth` of its own or its service's |

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
- A case may have an `"audited"` list of the same shape as `expect`, for the violations of **audited** rules (`@rollout(audit)`, §17.1). `expect` then holds only the violations of enforced rules, the ones that reject, and an audited violation is never in it. A missing `"audited"` means none. A target that evaluates rules on the server side (it reports to `onRuleViolation`) compares both lists, as sets. A target that only checks on a client compares `expect` alone and reports nothing audited, because a client skips audited rules, in `"partial": true` cases too. The vectors of audited rules are in `tests/rules/core/rollout*` and `tests/rules/endpoints/rollout*.veld`. The vectors of §11.7 to §11.9 are `tests/rules/endpoints/version.veld.pending` (19 examples) and `idempotency.veld.pending` (21): they wait under a `.pending` suffix until a generator declares `rules_version` or `idempotency`, and the reference evaluator runs them already (`crates/veld-rules/tests/version_idempotency.rs`). A vector cannot state a malformed `If-Match`, the `ETag` of a response, the version the handler receives, the fingerprint of a request, what the store holds afterwards, or that a key of another caller is not replayed; the tests of the reference evaluator state the first five, and the generators' own tests the last.
- A case may have `"output": "off"`: the case is a value in response position (no `"request": true`) checked by a server whose output mode is `off` (§5.3). Nothing is evaluated, so `expect` and `audited` are both empty and a server reports nothing, an audited rule included. A case with `"request": true` ignores the key, because input rules are evaluated in every mode. A target that only checks on a client has no output mode and skips these cases.
- A case may have a `"computed"` object, for derived fields (§17.2): its keys are paths from `$` in the syntax of a violation path (`$.total`, `$.items[1].line`, `$.page.items[0].sum`), its values are what the value has at that path once its defaults are applied and its derived fields computed, in wire form. `null` says the field has no value there: a failed expression, or a field that reads a `@readonly` field in a request. Only the paths named are compared, and `computed` applies in every output mode. A `computed` entry never names a `decimal`: the scale of a computed decimal is not specified (`1.5 * 2` may be `3` or `3.0`), so a decimal is observed through the invariants that read it. A derived field that failed is in `expect` with the field's wire name as its `rule`, the fields its expression reads as `fields` and `error` set. The vectors of derived fields are `tests/rules/core/derived.veld.pending` with `derived.json.pending` (22 cases) and `tests/rules/endpoints/derived.veld.pending` (9 examples): they wait under a `.pending` suffix until a generator declares `rules_derived`, and the reference evaluator runs them already (`crates/veld-rules/tests/derived.rs`). A vector cannot state that a value the handler gave for a derived field is overwritten (an example may not write one, `E0952`; a case with `"request": true` can), the `500 Internal` of a fact or a principal whose derived field fails (an example gives the facts, and the reference evaluator tests state it), what a client does (it never recomputes), or the helper that computes the derived fields of a value for a form.
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
| `409 Conflict` when `If-Match` does not match | `412 PreconditionFailed` (§11.7) | It is the status HTTP defines for a failed `If-Match` (RFC 9110), and clients and proxies treat it as such. `409 Conflict` stays for idempotency key conflicts. |
| Decimal division | Not supported (§3.4) | It needs a rounding rule; multiplication states the same condition exactly. |
| `final a, b` anywhere in an enum | A statement only when an identifier follows on the same line (§2.2) | `enum X { final }` must keep meaning a variant. An existing enum with a variant named `final` followed on the same line by another variant without a comma is the one program whose meaning changes. |
| `v.end.getTime()` in the TypeScript sketch | Runtime helpers (§8) | `datetime`, `decimal` and `long` are strings in TypeScript and Rust. |
| `principal User`, used as `user` | The same, plus `as name` (§11) | A package whose endpoints already have a parameter named `user` needs a way out. |

## 17. Lifecycle (Phase 5)

**Status: 17.1 is implemented in the compiler, the tools, the reference evaluator and the four built-in generators (`node`, `typescript`, `java`, `rust`); a generator that does not declare `rules_rollout` refuses it with `E0801`. 17.2 is implemented outside the generators (the compiler, the IR, the reference evaluator, `veld test`, `veld fmt`, `veld diff`, the OpenAPI export and the fuzzer); no generator implements it yet, so each reports `E0801` for `rules_derived`. 17.3 is implemented on the side of the tool (`veld test --from-violation`); the server option that produces a capture is not built. 18 is implemented (`veld check --prove` and `veld diff --prove`).** Sections 17 and 18 were written before their code, as the rest of this document was.

### 17.1 Rolling a rule out: `@rollout(audit)`

```veld
model Booking {
  guests: int
  @rollout(audit)
  rule smallParty: guests <= 6
}
```

```ebnf
require = { doc } { annotation } "require" … ;      (* was: { doc } "require" … *)
ensure  = { doc } { annotation } "ensure" … ;
```

- `@rollout(audit)` MAY stand on an invariant, a `require` or an `ensure`. Anywhere else, or with another argument, it is `E0602`.
- An audited rule is evaluated exactly where it would be, and **never rejects**. A violation, or an evaluation error, goes to `onRuleViolation` with the outcome `audited`, and the request continues as if the rule held. A `require … else E` that is audited never answers with `E`. An audited violation never appears in a response.
- Output mode `off` still skips an audited output rule. In `strict` and `audit` it is evaluated and reported.
- Clients skip audited rules, in the local check and in `partial`.
- The annotation is not part of the rule's id (§6), so removing it later keeps the id. **The rules digest (§9.7) covers only enforced invariants**: an audited invariant is left out of its input. Adding or removing the annotation therefore changes the digest, and a deployed client that still enforces the rule locally stops its local checks. A contract with no audited rule has the digest it has today.
- `veld test` treats an audited rule as not enforced, and lists under each example the audited rules that example would violate, without failing it.
- `veld diff`: adding the annotation to an existing rule is judged like removing the rule; removing the annotation like adding the rule. A new rule that arrives audited is compatible.
- In the IR, a rule, a `require` and an `ensure` gain `"rollout": "audit"`, omitted when enforced. The feature is `rules_rollout`: a generator that does not know it would enforce a rule that must not reject, so it reports `E0801`.

Where the text above was silent, the compiler, the tools and the reference evaluator do this:

- **Where it may stand.** `@rollout` may be written after the doc comment and in any order with `@deprecated` on an invariant. On a `require` or an `ensure` it is the only annotation that is legal: any other is `E0602` (`E0601` when unknown). On a `fact`, an `allow`, a `transition`, a policy, a principal and an example, `@rollout` is `E0602`; any other annotation there stays `E0003`. Written twice, or with no argument or an argument other than the word `audit` (a string, a number, a named or a second argument), it is `E0602`.
- **An audited invariant is evaluated wherever its declaration is checked**, so also on a declaration inside a list, a map, a page or a union arm, on an `error` the server encodes (an audited invariant never turns that answer into `500`) and on an event. A rule inherited from a parent is audited in the child too.
- **The digest.** A service whose reached invariants are all audited has no `rules_digest`, like a service with no invariant; the same holds for the digest of an event (`spec/events.md` §6.1). The digest of a contract without an audited rule is the one it had before.
- **The feature.** A service uses `rules_rollout` when a declaration its endpoints reach has an audited invariant (a model or an error), or one of its own endpoints has an audited `require` or `ensure`. No endpoint reaches an event, so a service also uses `rules_rollout` when it publishes or subscribes to a channel which carries an event with an audited invariant, on the event itself or on a declaration the event reaches (`Ir::features_used`; done). A generator which knows events and not rollout therefore refuses the contract and never rejects a message on a rule that must not reject. A client generates nothing for events, so the check does not apply to it, and only the services a target generates count, not the ones it merely calls. A feature that changes what a rule means is opt-in: a generator does not support it unless it says so. The gate reports `E0801` once, at the first audited rule.
- **`require ... else E`.** An audited one never answers, so its payload expressions are never evaluated (§9.3 evaluates them only when the answer is built).
- **Output mode in the reference evaluator.** `strict` evaluates an audited output rule and reports it; `audit` does the same; `off` evaluates no output rule, an audited one included. An audited input rule (step 5 and the `require` lines of step 6) is evaluated in every mode.
- **`veld test` and `veld check`.** An audited rule is not enforced, so it cannot fail an example and cannot make a filled-in value wrong (`E0953`). `veld test` prints the audited violations of an example under its line; `veld check` prints no such lines.
- **`veld diff`.** An audited rule is judged as if it were absent on both sides: adding the annotation is a removal, removing it is an addition, and a rule that is new and audited, or that changes or goes away while audited, is compatible.
- **The OpenAPI export.** The entry of an audited rule in `x-veld-rules` (a declaration or an operation) carries `"rollout": "audit"`; the entry of an enforced rule has no such key.
- **Vectors.** A case may say which violations are audited with the `"audited"` list of §15.
- **Live runs.** A test handler says which audited rules a request made it report, in the response header `Veld-Test-Audited` (§12.4), and `veld test --live` compares that with the reference evaluator. Without it a server that never reports an audited rule would pass every example: letting the request through is all an example can see.

What the generators do where the text above is silent:

- **Paths of a report.** An audited rule is reported with the path the reference evaluator gives it (the path of `Veld-Test-Audited`, §12.4). A body or a query object: its parameter name (`body`, `body.rows[1]`), like an enforced violation of the input. A result and a stream item: `result` in place of the `$` of the value (`result.rows[1]`). A declared error, whether the handler raised it or a `require ... else E` answered with it: `error`. A WebSocket message from the client: `message`. An audited `require` or `ensure`: `request`, like an enforced one. An enforced violation of an output is not told to the caller, and the hook of a generator may give it the plain `$`; the vectors compare the `rejected` events of an output with `expect` after reading `$` for the root.
- **A message the server sends.** Every item of an SSE stream and every message a server sends on a WebSocket is an output: its audited invariants are reported under the output mode and the item is sent whatever they say.
- **`validate = false`.** The audited rules that the option switches off are those it switches off for enforced ones: the invariants of the body, the query object and the WebSocket messages, every `require`, every `ensure`. The audited invariants of a result, an error and a stream item are still evaluated, like the enforced ones.
- **Live runs without `returns`.** An example that is not a dry run and has no `returns` runs the application's real handler. `veld test --live` gives the reference evaluator the success answer as the result that steps 8 and 9 saw, and compares the set of §12.4 with what that gives. An answer that is not a success has no result to give, so the server may report more than the reference knows: it must report at least what steps 5 and 6 give. The stub handlers of the shared vectors are never reached by an example: it is a dry run or it brings its result.
- **The partial check and the helpers of a client.** A client evaluates no audited rule, in `check`, in the local check before a send and in `partial`. Its `can...` helpers and its policy functions read no audited rule and are not affected.

### 17.2 Derived fields

```veld
model Order {
  items: Item[] @length(max: 100)
  discount: decimal = 0
  subtotal: decimal = sum(items, i => i.price * i.qty)
  total:    decimal = subtotal - discount
}
```

- A field whose `=` is followed by a literal has a default, as today. A field whose `=` is followed by any other expression is **derived**: its value is computed from the other fields of the same value.
- The scope is that of an invariant (§5.1): the fields of the declaration, no `now`, no facts. A derived field MAY read other derived fields; a cycle is `E0960`. Values are computed in dependency order, and in declaration order among fields that do not depend on each other.
- The expression's type MUST fit the field's type by the widening rules of §3 (`E0902`). A derived field MUST be required, and MUST NOT have `@readonly` or `@example` (`E0961`). Reading an optional field needs a guard (§3.6). Models, errors and events may have derived fields.
- **Nobody supplies a derived value.**
  - *A request.* The server ignores a value the caller sent for a derived field and computes it, after defaults and before the invariants (step 5), so rules may read derived fields.
  - *A response, a declared error, a stream item, a published event.* The handler's value need not contain derived fields: in the type a handler returns they are optional, and a value it does supply is overwritten. The server computes them before the output rules (step 8).
  - *A fact.* A fact is given its defaults and its derived fields after it is loaded.
  - *A client* takes a derived value from a response as it is, and never recomputes it: the server's contract may be newer.
- Every target, clients included, gets one function per declaration with derived fields that computes them for a value, so a form can show `total` while the user types. It is the code the server runs.
- An evaluation error (§3.7) in a derived field is a violation whose `rule` is the field's id and whose `error` is set: `422 ValidationError` in a request, the output mode in a response.
- Identity: `veld:<package>/<Decl>.<field>@<hex16>` over the RFC 8785 JSON of `{ "d": "<qualified decl name>", "k": "derived", "n": "<wire name>", "e": ExprShape }`. The expression is not part of the declaration's signature; the field is, like any other.
- The rules digest (§9.7) also covers the ids of the derived fields a service reaches, because a client's local check reads values it derived itself. A contract with no derived field has the digest it has today.
- In an example, a derived field is never filled in (§12.2) and MUST NOT be written (`E0952`); it is computed.
- `veld diff`: a changed derived expression is "possibly breaking" (`DerivedChanged`). A field that becomes derived is possibly breaking where callers send the type, because what they send is now ignored without an error, and a note where only consumers read it (`FieldBecameDerived`). A field that stops being derived is a note (`FieldNoLongerDerived`), and is breaking where callers send the type because it became required (`FieldBecameRequired`). A derived field is not required of a caller, so adding one is never `RequiredFieldAdded`.
- The OpenAPI export marks the property `readOnly` with `x-veld-derived` (its expression), keeps it in `required` (a response always has it) and does not expand it into a parameter of a query object. A contract without derived fields exports the same bytes.
- `veld fmt` prints the expression, and keeps the parentheses of a literal written in parentheses: without them the field would read as a field with a default. The fuzzer draws no value for a derived field and removes what a repair wrote into one; a rule on a derived field gets no boundary of its own. `veld check --prove` leaves a derived field open (the part `derived field`, §18.3): the solver may pick any value of its type. That keeps a proved contradiction sound, and a `diff` question that reads one is *unsupported*, because it claims something about every value.
- In the IR, `Field` gains `"derived": Expr` and `"derived_id": "<id>"`, both omitted when absent. The feature is `rules_derived`; it is opt-in (§17.1), so every generator refuses a contract with a derived field (`E0801`) until it declares the feature.

What the implementation chose where the text above is silent:

- **Literal or expression.** The text after `=` is a literal when it is one literal (a string, a number with its sign, `true`, `false`, `null`, `.variant`, `[]`) and the next token ends the field; `= -1` is a literal, `= -x` and `= 1 + 1` are expressions. Anything else, a parenthesised literal included, is an expression.
- **Where the id lives.** Beside `derived` in the IR, so no consumer computes it (the signatures are computed only by `veld-compiler::signature`). The `fields` of a violation are the fields the expression reads, first use first (`Expr::self_reads`).
- **The same order everywhere.** The values of one declaration are computed in the order of `Decl::derived_fields` of `veld-ir`: a field after every derived field it reads, and in declaration order among the others; inherited fields come first. A value inside a nested declaration, a list, a map, a page or a union arm is computed before the value that contains it, so `sum(items, i => i.line)` reads the `line` of every item already computed.
- **A value that fails.** When the expression of a derived field raises an evaluation error (§3.7) the field stays absent, and there is one violation: its `rule` is the field's id, its rule name (in vectors, and so the name inside its id) is the field's wire name, its `path` is the path of the value that carries the field, its `fields` are the fields the expression reads, its `message` is `field <name> could not be computed`, and its `error` is set. A derived field that reads an absent derived field stays absent without a second violation, and an invariant that reads an absent derived field is skipped: the cause is already reported.
- **Request position and the partial check.** A derived field whose evaluation reads a field that is absent (a `@readonly` field of a request body, or a field of a draft in `partial`) is left absent, with no violation; the invariants that read it are skipped. A value the caller sent for a derived field is dropped before the value is decoded, so a malformed one cannot raise a shape error.
- **Facts and the principal.** A fact or a principal is given its defaults and its derived fields when it is handed to the endpoint block (step 6). An evaluation error there is the application's mistake, not the caller's: the answer is `500 Internal`, reported to the hook like an output violation.
- **Output mode `off`.** Derived fields are values, not rules: they are computed in every mode, and an evaluation error leaves the field absent without a report.
- **Constraints.** `@min`, `@max`, `@length` and the other constraints of a derived field are not checked, because no caller supplies the value; write an invariant to constrain it.
- **`else` and examples.** `else E { .. }` never has to give a derived field of `E`. In an example, a derived field is written nowhere (`E0952`) except in the pattern after `ok` or an error, where it is compared like any other field of the computed result.
- **Order of reports.** The violations of derived fields of a value come before its invariants; tests compare sets.
- **Events.** A publisher computes the derived fields of an event before its invariants, and a derived field that fails stops the publish like a violated invariant. A subscriber computes them again from the data of the message: the value in the message is not believed, so a newer publisher cannot make a subscriber read something its own contract does not say. The rules digest of an event (`spec/events.md` §6.1) covers the ids of the derived fields it reaches, and `veld diff` reports a changed expression of an event as the event changed.
- **Streams.** The reference evaluator does not evaluate the items of a stream (it never did: no output rule of one either), so the vectors say nothing about them. A generator computes the derived fields of every item before the output rules, as it does for a result.
- **Idempotency.** The fingerprint of a request (§11.9) is made of the parameters after step 5, so it covers the derived values the server computed; a value a caller sent for one is not in it.
- **A `stored` answer.** The value after `stored` in an example may name a derived field, like a pattern after `ok`; the filler of the example drops it, and the stored body has the computed one.
- **The id.** The field's wire name stands in the id text and in the bytes, so renaming a field in source while keeping its wire name keeps the id (as for a rule, §6).
- **A client.** A client is only gated (`E0801`) for a derived field that an endpoint reaches. A fact or the principal is never seen by a client, so a derived field only they have is the server's.

### 17.3 From a violation to an example: `veld test --from-violation`

- The event of `onRuleViolation` (§5.3) does not contain the request, on purpose. A server option, off by default, adds a **capture** to it: the service and endpoint, the parameters in wire form, the facts, the principal and the clock. The documentation of the option says that a capture contains whatever the caller sent.
- `veld test --from-violation <file>` reads one such event as JSON, finds the endpoint by its operation signature, and prints an `example` block: the captured request, and the outcome the reference evaluator gives it. The developer pastes it into the contract. It writes nothing.
- An event without a capture, an operation the contract does not have, and a capture that does not fit the contract's types are each reported as such, with exit code 1.

**The event and its capture, in JSON.** The event is the one of §5.3 with the same member names (`spec/targets/typescript.md` §5 spells them `rule`, `operation`, `position`, `outcome`, `violation`, `elapsedMicros`), plus one member that exists only when the capture option is on:

```json
{
  "rule": "veld:bookings/NewBooking.atMost30Days@3f2a9c41d0e15b77",
  "operation": "veld:bookings/Bookings.payBooking@…",
  "position": "input",
  "outcome": "rejected",
  "violation": { "rule": "veld:bookings/NewBooking.atMost30Days@3f2a9c41d0e15b77", "path": "$.body", "fields": ["start", "end"], "message": "A stay is at most 30 days" },
  "elapsedMicros": 41,
  "capture": {
    "service": "Bookings",
    "endpoint": "payBooking",
    "operation": "veld:bookings/Bookings.payBooking@…",
    "params": { "id": "00000000-0000-4000-8000-0000000000aa", "body": { "amount": "10.00" } },
    "facts": { "booking": { "guestId": "00000000-0000-4000-8000-0000000000aa", "total": "10.00" }, "coupon": null },
    "principal": { "id": "00000000-0000-4000-8000-0000000000aa", "roles": [] },
    "now": "2026-06-01T00:00:00Z",
    "redacted": ["facts.booking.card", "principal.password"]
  }
}
```

- The tool reads only `capture`; every other member is ignored, so a generator may add members to the event. A `capture` that is `null` or absent is an event without a capture.
- `capture.operation` is the signature of the endpoint (`spec/ir.md` §5.2), the key the tool finds it by. `service` and `endpoint` are for people and are not read.
- `params` has one member per parameter of the endpoint that the request had, by parameter name, each in wire form after binding; an optional parameter that was not sent is absent. A parameter of a stream is left out.
- `facts` has one member per fact the server loaded for the request, by fact name, in wire form, and `null` for a fact the loader did not find. These are the values `Veld-Test-Facts` carries (§12.4). A fact the endpoint declares and `facts` lacks is read as absent.
- `principal` is the caller in wire form, as `Veld-Test-Principal` carries it, or `null` (or absent) when the request had none.
- `now` is the clock the server read for the request, as RFC 3339 text, as `Veld-Test-Now` carries it.
- `redacted` lists the wire paths of the values the capture left out because the field is `@pii` or `@secret` (§19.3), or is empty or absent. A path starts with `params.<name>`, `facts.<name>` or `principal`, and goes on with `.<wire name>` for a field or a map key and `[<index>]` for a list element (`params.body.guests[1].phone`). The member itself is missing from its parent; a list keeps its length.

**What the tool does with it.**

- It looks the endpoint up by `capture.operation` alone, fills in each value named in `redacted` by the rules of §12.2 (the same counters run through the whole example, in the order params, facts, principal), runs the request against the reference evaluator in `strict` mode with no `returns`, and prints the `example` block with every value written out and the outcome the evaluator gives. Standard output holds the block and nothing else, so it can be piped into a file. Everything else goes to standard error, one line each, starting with `note:`.
- **Filled-in values are named.** One line `note: filled in <path>` per value that was filled in under a path of `redacted`.
- **Reproduction is checked, not assumed.** When the outcome does not contain the rule of the event (as a violation, or as an audited report), the tool says `note: the example does not reproduce the violation of <rule id>; it ends in <outcome>`, and adds `a value was filled in, so the original values may matter` when `redacted` was not empty. An event whose `position` is `output` is reproducible only with the handler's result, which a capture does not hold, so the tool says `note: the violation is of an output rule; add a returns line with the result the handler gave` and prints the block as it is.
- **Errors, each exit code 1, nothing written or printed to standard output.**
  1. *No capture*: `the event has no capture; turn the capture option of the server on`.
  2. *Unknown operation*: `the contract has no endpoint with the signature <signature>`.
  3. *A capture that does not fit*: `the capture does not fit the contract: <what>`, where what is one of an unknown parameter, fact or `redacted` path root, a required value (a parameter, or a field of a parameter, of a fact or of the principal) that the capture lacks and `redacted` does not name, a value of the wrong shape or type, a `now` that is not RFC 3339, or a value that cannot be filled in or cannot be written in the syntax of an example (a `json` value that holds `null`).
  A file that cannot be read or is not JSON is reported as such, also with exit code 1.

## 18. `veld check --prove` (Phase 5)

`--prove` asks an SMT solver questions about the rules. It finds contradictions that examples cannot find, because an example needs a value and a contradiction has none.

**What it is not.** It reasons about the rules of the contract, over the fragment below. It says nothing about handlers, databases, facts that are loaded, concurrency or deployed code, and its output never uses the words "verified" or "correct" for an application.

### 18.1 The questions

| Question | A finding |
|---|---|
| Can the invariants of a declaration all hold, together with its constraints? | `E0970`: no value of `M` can exist, with the rules of the unsatisfiable core |
| Can a `require` pass, given the invariants of the parameters and the `require` lines before it? | `E0970`: the `require` can never pass |
| Does a policy permit anybody? | `E0970`: the policy permits nobody |
| `veld diff --prove`: is every value the old rule accepted still accepted? | "tightened", with a value that was accepted and is now rejected; or "not tightened", proved |

Whether a state of a state machine can be reached is not asked: the contract does not say where a value starts.

### 18.2 Four results

Every question ends in exactly one of:

- **proved.** The solver answered `unsat`: no value exists (the first three questions), or no value distinguishes the two rules (`diff`).
- **counterexample.** The solver produced a value. **Veld evaluates it with the reference evaluator before it is shown**; a value the evaluator does not confirm is never printed, and the result becomes *unknown*. For the first three questions a confirmed value means the question has no finding.
- **unknown.** The solver timed out or answered `unknown`, or its value was not confirmed.
- **unsupported.** The question needs something outside the fragment in a place where leaving it open would make the answer meaningless.

Only *proved* (for the first three questions) and a confirmed *counterexample* (for `diff`) are findings. *Unknown* and *unsupported* are counted and listed, never turned into a pass or a failure.

### 18.3 The fragment, and why a proof can be trusted

| Veld | Modelled as |
|---|---|
| `bool`, `&&`, `\|\|`, `!`, `implies`, `==`, `!=`, `in` over a literal list | Booleans |
| `int`, `long`, arithmetic, comparisons | Integers with the range of the type; an operation that leaves the range is an evaluation error, which makes the rule violated (§3.7) |
| `decimal`, `+`, `-`, `*`, comparisons | Exact reals |
| `datetime`, `date`, `time`, `duration` | Integers: milliseconds, days |
| An enum | A finite set of its variants |
| An optional field and `has` | A Boolean beside the value |
| `len` of a string, list or map | A non-negative integer, bounded by `@length` |
| `@min`, `@max`, `@length` | Assertions |
| String equality | Equality over an uninterpreted sort |
| `float`; the content of strings; `matches`; every comprehension and `sum`; `now`; a derived field (§17.2) | **Left open**: an unconstrained value of the right sort |

- Leaving a part open makes the problem easier to satisfy, never harder. So `unsat` for the opened problem implies `unsat` for the real one: **a proved contradiction stays sound** however much was left open.
- A value found for an opened problem may not be a real one, which is why every value is confirmed by the evaluator (§18.2).
- A `diff` question claims something about all values, where leaving a part open is not sound. If either rule contains an open part, the result is *unsupported*.
- The encoder is tested against the evaluator: for generated values, the encoding with the value substituted and the evaluator must agree on every rule of the vectors (§15).

### 18.4 The solver

- An external program that reads SMT-LIB 2 on standard input: Z3 (`z3 -in`) or cvc5. `--solver <path>` or `VELD_SOLVER` names it; otherwise `z3` and then `cvc5` are looked up on `PATH`. `veld` stays one binary and links no solver.
- Without a solver, `--prove` says so in one line and the normal checks still run; the exit code is that of `veld check`.
- Each question has a time limit (5 seconds by default, `--prove-timeout`). The solver is started with a fixed random seed.
- The report names the solver and its version, lists the fragment's open parts that the contract uses, and counts the four results. `E0970` findings fail `veld check`; `W0970` reports that some questions ended *unknown* or *unsupported*, with their names.

### 18.4a How the questions are asked

Where this section was silent, these choices hold.

- **One process per run.** All questions of a run go to one solver process as one script: `(push 1)` ... `(pop 1)` per question, each opened by `(echo "Q<i>")`. The exit code of the solver is ignored (Z3 exits 1 when `(get-value)` follows `unsat`). The time limit is set in the solver per `check-sat` (`:timeout` for Z3, `:tlimit-per` for cvc5), and Veld kills the process after the limits of all questions plus 20 seconds; questions without an answer are *unknown*.
- **Arguments.** A program whose file name contains `cvc5` is started with `--lang smt2 --incremental`; any other program, whatever `--solver` names, with `-in -smt2`. The seed is `0`: `(set-option :random-seed 0)` for Z3, `(set-option :seed 0)` for cvc5. The name and version in the report come from `(get-info :name)` and `(get-info :version)`.
- **Values.** Strings and uuids are an uninterpreted sort with a length function for strings; a value is turned into a text of the length the solver gave, different for every distinct value. A part of a value no rule reads is filled in with the simplest value that keeps its constraints. `@min`, `@max`, `@length` and `@pattern` are checked by Veld on the value as well, because the evaluator of rules does not look at them. A string with a `@pattern` or `@format` that a rule reads gives *unknown* when the solver answers `sat`.
- **Evaluation errors** follow short-circuiting: every expression has a value and a condition under which evaluating it is an error, and a rule holds when its condition is false and its value true. No global assertion rules out an overflow or a zero divisor.
- **First question.** Invariants of nested models and of the items of lists are not part of the question (leaving them out only makes it easier to satisfy). Rules that read a `@readonly` field are included, as on a response. Audited rules (`@rollout(audit)`) never reject and are not assumed, here and in the other questions.
- **Second question.** A `require` is asked about only if it is enforced. The invariants of the parameters (skipping rules that read a `@readonly` field, as in a request), the constraints of the parameters, and the enforced `require` lines before it are assumed. Facts and the principal are trusted values (sections 9.2 and 11.1): only the domain of their type is assumed, never their constraints or invariants. Policies and `transition` before the `require` are not assumed.
- **Third question.** The arguments of a policy are trusted values in the same way. A policy with no `permit` is asked too, and is a finding.
- **Open parts besides the table.** `in` over anything but a literal list (a list field, a map), and the `@pattern` and `@format` of a field a rule reads, are left open and reported as such (`membership in a list or map`, `pattern and format of a string`).
- **`diff`.** The question covers the invariants of models and errors and the `require` lines of endpoints whose expression changed (a new rule id). It asks for a value that the old rule accepts and the new rule rejects, with the constraints and the parameter invariants of the new contract and the earlier `require` lines of the old endpoint assumed. A field whose type changed between the two contracts is *unsupported*. A tightened rule is breaking where requests meet it, and a note where only responses meet it. A rule proved not tightened is non-breaking where only requests meet it; on responses a looser rule loses a guarantee, so it stays *possibly breaking*. *Unknown* and *unsupported* leave the change as `veld diff` reports it without `--prove`, with the reason added. Policies, `ensure` and state machines are not asked.
- **Codes.** `E0970` findings fail `veld check`; `W0970` never does and is not counted by `--strict`.

### 18.5 Codes of sections 17 and 18

| Code | Meaning |
|---|---|
| `E0960` | Derived fields depend on each other in a cycle |
| `E0961` | A derived field is optional, or has `@readonly` or `@example` |
| `E0970` | `--prove`: the rules contradict each other, a `require` can never pass, or a policy permits nobody |
| `W0970` | `--prove`: some questions ended unknown or unsupported |

## 19. Extras (Phase 6)

Three small things that do not depend on each other. 19.1 and 19.2 are implemented (`veld test --coverage`, `veld export rules`); 19.3 is not built yet.

### 19.1 `veld test --coverage`

A rule that no example has seen fail is not tested. `--coverage` says which rules the examples exercise.

- It counts every **enforceable unit** of the workspace: each invariant of a declaration that a service reaches or an event example uses (by rule id), each `require`, each `ensure`, each `allow` line and each `transition` line.
- While the reference evaluator runs the examples (§12.3), a unit is **held** when some example evaluated it and it held, and **violated** when some example evaluated it and it did not hold or could not be evaluated. A unit an earlier rejection kept from being evaluated counts as neither. For an `allow` line, held is permitted and violated is denied; for a `transition`, held is a legal move and violated is an illegal one.
- A unit is **covered** when it was both held and violated. An audited rule (§17.1) counts like an enforced one.
- An `ensure` is evaluated only by an example with `returns`, so an endpoint whose examples never say what the handler returns leaves its `ensure` lines uncovered.
- The fuzzer does not count: `--coverage` is about what the contract's own examples show.

The report follows the list of examples: one line with the totals, then every unit that is not covered, with what is missing, ordered by package and then by position in the source.

```
coverage: 31 of 38 rules held and violated (81%)
  bookings/NewBooking.atMost30Days        never violated
  bookings/Bookings.cancelBooking require notCheckedIn   never evaluated
  bookings/Bookings.payBooking allow ManageBooking       never denied
```

- The percentage is rounded down to a whole number. A workspace with no unit reports `coverage: no rules` and is at 100.
- `--fail-under N` (0 to 100) makes the run exit 1 when the percentage is below `N`. Without it the exit code is that of the examples.
- `--coverage` works with and without `--live`: coverage comes from the reference evaluation either way.

Where the text above is silent, the tool does this:

- **Labels.** A unit is named `<package>/<Declaration>.<rule>` for an invariant (the declaration that declares it, not one that inherits it), and `<package>/<Service>.<endpoint> <kind> <name>` for an endpoint statement: `require <name>`, `ensure <name>`, `allow <Policy>` and `transition <from>, <from> -> <to>`. A `require` or `ensure` without a name is `require #N` or `ensure #N`, N counting from 1 among the statements of that kind of the endpoint. Two `allow` lines of one policy have the same label.
- **Layout.** The label is padded with spaces to the longest label of the report, then three spaces, then the words. Lines are indented by two spaces.
- **What is missing.** `never evaluated` when no example evaluated the unit. When it was evaluated but not both ways: `never violated` (held only), `never held` (violated only); for an `allow` `never denied` and `never permitted`; for a `transition` `never illegal` and `never legal`.
- **Which examples count.** Every example the run selected, failed ones included: the evaluator evaluated the same rules. An example that cannot be run (`E0950` to `E0953`) counts nothing. Event examples count the invariants of the declarations the event reaches.
- **Which units count.** The units of the packages the selection (`-p`, `--workspace`) names, whatever `--filter` and the file arguments say: they narrow the examples, not the rules, so a narrower run has a lower percentage. A unit is counted once, by its rule id (an `allow` line by its position). The invariants of a declaration that only a `fact` or a policy mentions, and that no endpoint reaches, are not counted. No endpoint reaches an event (§17.1), so the invariants of an event, and of what it reaches, count only when an event example delivers that event. The version guard of §11.7 and the idempotency check of §11.8 are not units.
- **Evaluation.** A rule the pipeline does not reach (an earlier check rejected the request), an invariant that reads a `@readonly` field of a request, and the check of the defaults of a loaded fact are not evaluations. An evaluation error counts as violated.
- **The run.** With `--coverage` the report is printed even when no example ran, and `--fail-under` then fails a workspace that has rules. A failing example fails the run as before, whatever the percentage. The report goes to standard output and is not hidden by `--quiet`. `veld test` has no machine-readable output, so there is no JSON form of the report.

### 19.2 `veld export rules`

`veld export rules [-o FILE]` writes the rule book of the workspace as Markdown, for people who do not read contracts. It is written as `veld export openapi` writes its file: the user owns it.

The document is deterministic and has, per service package in name order:

1. **Each declaration with invariants**, in source order: its name and doc comment, and one line per rule with the rule's message (or its name when it has none), its doc comment, and the expression as `veld fmt` prints it.
2. **Each state machine**: a table of the moves (from, to) and the final states.
3. **Each policy**: its doc comment, then its `permit` and `forbid` clauses in order, a `forbid` with its message.
4. **Each endpoint with rules**, in source order: the method and path, the facts it loads, who may call it (`allow`), the move it makes (`transition`), what it requires (each `require` with the error it answers), and what it promises (`ensure`).
5. **The examples** of the package, by name, each with its outcome.

An audited rule (§17.1) is marked "audited, not enforced". A deprecated rule is marked so. The export of a workspace without rules says that it has none.

Where the text above is silent, the tool writes this:

- **Document.** `# Rule book`, then per package ``## Package `name` ``, then `### Declarations`, `### State machines`, `### Policies`, `### Endpoints` and `### Examples`, each only when it has something. Sections are separated by one blank line. A package with nothing in any part is left out. The sentence of a workspace without rules is `This workspace has no rules.` under the title.
- **Declarations.** `#### Name (model|error|event)`, its doc comment, then one line per own rule (an inherited rule is shown with the declaration that declares it): ``- <message or name>: `<expression>` ``, and the rule's doc comment on the lines under it, indented by two spaces. Marks follow the expression: ` (audited, not enforced)`, ` (deprecated)` or ` (deprecated: <reason>)`. Declarations of library packages are not listed: the rule book is per service package.
- **State machines.** `#### Name`, the doc comment, the table `| From | To |` with the moves in declaration order, and `Final states: a, b`.
- **Policies.** `#### Name(param, param)`, the doc comment, then ``- permit when `expr` `` and ``- forbid when `expr`: <message>`` in order.
- **Endpoints.** Only those with a `fact`, `allow`, `transition`, `require` or `ensure`: `#### METHOD /full/path (Service.endpoint)`, the doc comment, then the bullets `Loads`, `Who may call it`, `Move`, `Requires` and `Promises`, the last two with one nested bullet per statement: ``<message, else name, else "require #N">: `expr`, answers `Error` `` (`ValidationError` for a `require` without `else`) and, for an `ensure`, no error. An `allow` is written `Policy(args)`, with the caller written `principal`.
- **Examples.** `- <name>: ok`, or the name of the error the example expects, or `rejected`, or the handlers an event example expects to be (or not be) called. Examples are in name order (item 5 of §19.2 says "by name"); everything else is in source order.
- **Expressions.** The IR keeps no source text, so the book uses the printer of the OpenAPI export and of `veld diff`, not the printer of `veld fmt` (which works on the source). Both give the fewest parentheses that keep the meaning and put spaces around binary operators, and they agree on every expression written that way. They differ where the IR has forgotten the spelling: `not` is always written `not` (never `!`), a number is printed from its value, and the caller of a policy is written `principal` whatever name the endpoint block gave it (`allow ManageBooking(principal, booking)`). The test of the rule book pins the expressions that `veld fmt` prints identically.
- **Flags.** `-o FILE` and `--force` as for `veld export openapi`; `--service NAME` (repeatable) limits the book to the packages of those services and to their endpoints. `veld export` has no `-p`.

### 19.3 `@pii` and `@secret`

```veld
model Guest {
  name: string @pii
  phone?: string @pii
  password: string @secret
}
```

- Both are annotations of a field of a model, an error or an event, and take no argument. Anywhere else, or with an argument, they are `E0602`. A field may have both.
- `@pii` says the field holds personal data. `@secret` says its value must never be shown: a password, a token, a key.
- **They change nothing on the wire.** The field's type, its validation, the declaration's signature and the rules are what they would be without the annotation, and a rule may read the field.
- In the IR a `Field` gains `"pii": true` and `"secret": true`, each omitted when false, so a contract without them keeps its IR (`spec/ir.md` §3.6). The OpenAPI export marks the property with `x-veld-pii` and `x-veld-secret`, and only the marked ones.
- **`veld diff`.** Adding or removing either annotation on a field is **non-breaking** (kinds `pii_added`, `pii_removed`, `secret_added`, `secret_removed`): nothing on the wire changes, so no caller built against the old contract is affected. The change is listed so that a reviewer sees it. Only a field that a declaration wrote itself is compared; an inherited one follows its parent.

**What generated code must not do.** Generated code already sends a caller only what the contract's types say. These two annotations are about everything else it emits:

- **Messages.** The message of a validation issue, of a violation or of a generated log line never contains the value of a field, annotated or not. Messages are meant to be fixed texts with paths and names; this makes it a tested rule: each runtime gets a test that feeds a marker value through every failing path and searches every message for it, and a message that repeats a value is changed.
- **Captures** (§17.3). A capture leaves out the value of every `@pii` and `@secret` field, in the parameters, the facts and the principal, at any depth, and lists the wire paths it left out in `redacted`. `veld test --from-violation` then prints an example in which those values are filled in (§12.2) and says which were: such an example may not reproduce the violation, and the tool says so instead of pretending.
- **Audit events** (the platform's `@audited`) follow the same rule when they are built.

**The reference.** `veld_rules::redact_decl(ir, decl, value)` and `redact_type(ir, ty, value)` (and the `_at` forms, which start the paths at a given root such as `params.body`) take a wire value and return it with every `@pii` and `@secret` field left out, and the wire paths of what was left out. This is what the generators must reproduce, and `tests/wire/redaction/` holds the byte goldens.

- A marked field is removed **whole**, whatever it holds, and only its own path is listed; a field the value does not have is not listed; a field with the value `null` is removed and listed.
- The walk goes through nested models, lists, maps, the `items` of a page and the arm of a union (chosen by the discriminator). A member the declaration does not know is kept.
- Paths are written like the paths of violations (§7): `$` is the root, `.wire` a member, `[2]` a list item (`$.home.street`, `$.past[1].geo.lat`). The list is sorted by the bytes of the paths and has no duplicates, so `$.a[10]` comes before `$.a[2]`.
- A value that does not have the shape of its declaration (an object where a list is declared, a union value that names no arm) is an error, not a pass: such a value cannot be known to be clean.

**Lints.**

| Code | Name | Reports |
|---|---|---|
| `L0015` | `pii-without-auth` | A `@pii` field that a response, a declared error or a stream item of an endpoint with `@auth(none)`, or with no auth at all, can contain |
| `L0016` | `secret-in-output` | A `@secret` field that any response, declared error, stream item or event can contain |

Both are warnings by default (`[lint]` in `veld.toml` changes the level). `secret-in-output` has one usual fix: a separate model for what the server returns.

**What they are not.** The annotations stop generated code from repeating a value where nobody asked for it. They do not encrypt anything, they do not reach the application's own logs or its database, and they are not, by themselves, compliance with any regulation.

Neither annotation is a feature a generator can refuse: a generator that emits captures or audit events must redact, and one that emits neither has nothing to redact.

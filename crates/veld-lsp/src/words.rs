//! The words of the rules language (`spec/rules.md` sections 3.5, 9, 10, 11 and 12), in one
//! table. Hover, completion and semantic tokens all read it, so a signature or a line of meaning
//! is written once.

/// What a word is, for the token type and the completion kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    /// A built-in function (`spec/rules.md` section 3.5).
    Function,
    /// A statement or declaration word.
    Keyword,
    /// A name that is in scope in some places (`now`, `result`).
    Name,
    /// A word inside an expression or a statement (`else`, `in`, `message`).
    Operator,
}

/// One word.
#[derive(Debug, Clone, Copy)]
pub struct Word {
    pub word: &'static str,
    pub class: Class,
    /// How it is written, for hover and the detail of a completion.
    pub signature: &'static str,
    /// One line of meaning.
    pub meaning: &'static str,
    /// The text a completion inserts, in the snippet syntax.
    pub snippet: &'static str,
}

const fn w(
    word: &'static str,
    class: Class,
    signature: &'static str,
    meaning: &'static str,
    snippet: &'static str,
) -> Word {
    Word {
        word,
        class,
        signature,
        meaning,
        snippet,
    }
}

/// The built-in functions, with `old`.
pub const BUILTINS: &[Word] = &[
    w(
        "has",
        Class::Function,
        "has(path) -> bool",
        "True when every optional step of the path is present. It makes the path readable in the rest of the expression.",
        "has(${1:path})",
    ),
    w(
        "len",
        Class::Function,
        "len(x) -> int",
        "The length of a string (in code points), a list, a map or bytes.",
        "len(${1:x})",
    ),
    w(
        "matches",
        Class::Function,
        "matches(s: string, \"re\") -> bool",
        "True when the string matches the regular expression, which is a string literal. It matches anywhere unless anchored.",
        "matches(${1:s}, \"${2:re}\")",
    ),
    w(
        "startsWith",
        Class::Function,
        "startsWith(s: string, p: string) -> bool",
        "True when the string starts with the prefix.",
        "startsWith(${1:s}, ${2:p})",
    ),
    w(
        "endsWith",
        Class::Function,
        "endsWith(s: string, p: string) -> bool",
        "True when the string ends with the suffix.",
        "endsWith(${1:s}, ${2:p})",
    ),
    w(
        "contains",
        Class::Function,
        "contains(s: string, p: string) -> bool",
        "True when the string contains the other string.",
        "contains(${1:s}, ${2:p})",
    ),
    w(
        "days",
        Class::Function,
        "days(n: int) -> duration",
        "A duration of n days.",
        "days(${1:n})",
    ),
    w(
        "hours",
        Class::Function,
        "hours(n: int) -> duration",
        "A duration of n hours.",
        "hours(${1:n})",
    ),
    w(
        "minutes",
        Class::Function,
        "minutes(n: int) -> duration",
        "A duration of n minutes.",
        "minutes(${1:n})",
    ),
    w(
        "seconds",
        Class::Function,
        "seconds(n: int) -> duration",
        "A duration of n seconds.",
        "seconds(${1:n})",
    ),
    w(
        "ms",
        Class::Function,
        "ms(n: int) -> duration",
        "A duration of n milliseconds.",
        "ms(${1:n})",
    ),
    w(
        "all",
        Class::Function,
        "all(list, x => bool) -> bool",
        "True when the condition holds for every element. True for an empty list.",
        "all(${1:list}, ${2:x} => ${3:cond})",
    ),
    w(
        "any",
        Class::Function,
        "any(list, x => bool) -> bool",
        "True when the condition holds for some element. False for an empty list.",
        "any(${1:list}, ${2:x} => ${3:cond})",
    ),
    w(
        "count",
        Class::Function,
        "count(list, x => bool) -> int",
        "The number of elements for which the condition holds.",
        "count(${1:list}, ${2:x} => ${3:cond})",
    ),
    w(
        "sum",
        Class::Function,
        "sum(list, x => n) -> number",
        "The sum of a number computed from each element. Zero for an empty list.",
        "sum(${1:list}, ${2:x} => ${3:n})",
    ),
    w(
        "old",
        Class::Function,
        "old(e) -> type of e",
        "In an `ensure`: the value of e before the handler ran. A fact read in an `ensure` must be inside `old(...)`.",
        "old(${1:e})",
    ),
];

/// Names that are in scope in some places, and the words of statements and declarations.
pub const WORDS: &[Word] = &[
    w(
        "now",
        Class::Name,
        "now: datetime",
        "One reading of the server's clock, taken once per request before the facts load. Not available in an invariant.",
        "now",
    ),
    w(
        "result",
        Class::Name,
        "result",
        "In an `ensure`: the success value of the endpoint.",
        "result",
    ),
    w(
        "rule",
        Class::Keyword,
        "rule name: expr [message \"text\"]",
        "An invariant of a model or error: a boolean expression that every value of the type must satisfy.",
        "rule ${1:name}: $0",
    ),
    w(
        "fact",
        Class::Keyword,
        "fact name[?]: Type [= key]",
        "A value the rules need and only the application can load. It loads once, before any rule runs.",
        "fact ${1:name}: ${2:Type} = ${3:key}",
    ),
    w(
        "require",
        Class::Keyword,
        "require [name:] expr [else Error [{ payload }]] [message \"text\"]",
        "A precondition. The first one that fails ends the request: with `else E` the answer is E, otherwise `422 ValidationError`.",
        "require ${1:expr} else ${2:Error}",
    ),
    w(
        "ensure",
        Class::Keyword,
        "ensure [name:] expr [message \"text\"]",
        "A postcondition, checked on the success value of the handler. It may read `result` and `old(...)`.",
        "ensure ${1:expr}",
    ),
    w(
        "allow",
        Class::Keyword,
        "allow Policy(args)",
        "Asks a policy whether the caller may do this. A denial is `403 Forbidden`.",
        "allow ${1:Policy}($0)",
    ),
    w(
        "transition",
        Class::Keyword,
        "transition fact.path: a, b -> c",
        "Guards a state change: the field of the fact must hold one of the sources, and the handler must leave it in the target.",
        "transition ${1:fact}.${2:field}: ${3:from} -> ${4:to}",
    ),
    w(
        "flow",
        Class::Keyword,
        "flow a -> b -> c",
        "Declares the legal moves of a state machine: a -> b and b -> c.",
        "flow ${1:a} -> ${2:b}",
    ),
    w(
        "final",
        Class::Keyword,
        "final a, b",
        "States that have no way out.",
        "final ${1:state}",
    ),
    w(
        "policy",
        Class::Keyword,
        "policy Name(param: Type, ..) { permit when .. forbid when .. }",
        "Decides whether a caller may act: it permits when some `permit` is true and no `forbid` is.",
        "policy ${1:Name}(${2:user}: ${3:User}) {\n  permit when $0\n}",
    ),
    w(
        "permit",
        Class::Keyword,
        "permit when expr [message \"text\"]",
        "A condition under which the policy permits.",
        "permit when $0",
    ),
    w(
        "forbid",
        Class::Keyword,
        "forbid when expr [message \"text\"]",
        "A condition under which the policy refuses, whatever the `permit` lines say.",
        "forbid when ${1:expr}\n    message \"${2:text}\"",
    ),
    w(
        "principal",
        Class::Keyword,
        "principal Type [as name]",
        "The caller of the package: a value of this model, handed in by the auth hook. In an endpoint block it is called `user`, or the name after `as`.",
        "principal ${1:User}",
    ),
    w(
        "example",
        Class::Keyword,
        "example \"name\" { given .. [as v] [at \"t\"] call }",
        "Documentation that runs: what one call does, checked by `veld test`.",
        "example \"${1:what it shows}\" {\n  $0\n}",
    ),
    w(
        "given",
        Class::Keyword,
        "given fact = value | absent",
        "Sets the fact the loader returns, or `absent` when it finds nothing.",
        "given ${1:fact} = $0",
    ),
    w(
        "as",
        Class::Keyword,
        "as { principal value }",
        "The caller of the example.",
        "as { $0 }",
    ),
    w(
        "at",
        Class::Keyword,
        "at \"2026-01-01T00:00:00Z\"",
        "The clock of the example.",
        "at \"${1:2026-01-01T00:00:00Z}\"",
    ),
    w(
        "returns",
        Class::Keyword,
        "returns value",
        "What the handler returns. Without it an offline run stops before the handler.",
        "returns { $0 }",
    ),
    w(
        "ifMatch",
        Class::Keyword,
        "ifMatch 3 | ifMatch any",
        "The version the example sends in `If-Match`.",
        "ifMatch ${1:1}",
    ),
    w(
        "key",
        Class::Keyword,
        "key \"k\" [stored outcome | other | running]",
        "The idempotency key the example sends, and what the store holds for it.",
        "key \"${1:k}\"",
    ),
    w(
        "ok",
        Class::Keyword,
        "-> ok [value]",
        "The call succeeds.",
        "ok",
    ),
    w(
        "absent",
        Class::Keyword,
        "given fact = absent",
        "The loader finds nothing: the answer is `404 NotFound`.",
        "absent",
    ),
    w(
        "else",
        Class::Operator,
        "require expr else Error [{ field: expr }]",
        "The error that answers when the `require` fails, and its payload.",
        "else ${1:Error}",
    ),
    w(
        "message",
        Class::Operator,
        "message \"text\"",
        "The text that a violation carries.",
        "message \"${1:text}\"",
    ),
    w(
        "when",
        Class::Operator,
        "permit when expr",
        "Introduces the condition of a `permit` or a `forbid`.",
        "when $0",
    ),
    w(
        "in",
        Class::Operator,
        "x in list | key in map",
        "True when the list holds the value, or the map has the key.",
        "in $0",
    ),
    w(
        "implies",
        Class::Operator,
        "a implies b",
        "`!a || b`: if a holds, b must hold.",
        "implies $0",
    ),
    w(
        "not",
        Class::Operator,
        "not expr",
        "The same as `!`.",
        "not $0",
    ),
    w("true", Class::Operator, "true", "The boolean true.", "true"),
    w(
        "false",
        Class::Operator,
        "false",
        "The boolean false.",
        "false",
    ),
    w(
        "expect",
        Class::Keyword,
        "expect Service.handler [not] called | expect rejected",
        "What an event example expects.",
        "expect ${1:Service}.${2:handler} called",
    ),
];

/// A built-in function by name.
#[must_use]
pub fn builtin(name: &str) -> Option<&'static Word> {
    BUILTINS.iter().find(|b| b.word == name)
}

/// Any word of the tables by name.
#[must_use]
pub fn word(name: &str) -> Option<&'static Word> {
    WORDS.iter().chain(BUILTINS.iter()).find(|b| b.word == name)
}

/// The words that start a statement or a declaration of the rules language, in the places where
/// the parser reads them as keywords.
#[must_use]
pub fn statement_words(names: &[&str]) -> Vec<&'static Word> {
    names.iter().filter_map(|n| word(n)).collect()
}

/// The markdown of a word.
#[must_use]
pub fn describe(w: &Word) -> String {
    format!("```veld\n{}\n```\n{}", w.signature, w.meaning)
}

/// The errors that every endpoint may answer with, without declaring them (`spec/rules.md`
/// section 12.1).
pub const IMPLICIT_ERRORS: [&str; 6] = [
    "ValidationError",
    "NotFound",
    "Unauthorized",
    "Forbidden",
    "InvalidTransition",
    "Internal",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_names_every_builtin_of_the_spec_once() {
        for name in [
            "has",
            "len",
            "matches",
            "startsWith",
            "endsWith",
            "contains",
            "days",
            "hours",
            "minutes",
            "seconds",
            "ms",
            "all",
            "any",
            "count",
            "sum",
            "old",
        ] {
            assert_eq!(
                BUILTINS.iter().filter(|b| b.word == name).count(),
                1,
                "{name}"
            );
        }
        let mut all: Vec<&str> = WORDS
            .iter()
            .chain(BUILTINS.iter())
            .map(|w| w.word)
            .collect();
        all.sort_unstable();
        let n = all.len();
        all.dedup();
        assert_eq!(all.len(), n, "a word is in the table twice");
    }
}

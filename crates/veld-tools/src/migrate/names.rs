//! Naming helpers: casing, reserved words and module paths.

use veld_compiler::language::is_reserved;

/// The prelude error names and their default statuses.
pub(super) const PRELUDE_ERRORS: [(&str, u16); 8] = [
    ("BadRequest", 400),
    ("Unauthorized", 401),
    ("Forbidden", 403),
    ("NotFound", 404),
    ("Conflict", 409),
    ("ValidationError", 422),
    ("TooManyRequests", 429),
    ("Internal", 500),
];

pub(super) fn prelude_status(name: &str) -> Option<u16> {
    PRELUDE_ERRORS
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, s)| *s)
}

/// Names that the prelude provides, errors and the one model.
pub(super) fn is_prelude_name(name: &str) -> bool {
    prelude_status(name).is_some() || name == "ValidationIssue"
}

pub(super) fn is_ident(s: &str) -> bool {
    let mut chars = s.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Turns `s` into a valid, non-reserved identifier.
pub(super) fn sanitize_ident(s: &str) -> String {
    let mut out: String = s
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if out.is_empty() || out.starts_with(|c: char| c.is_ascii_digit()) {
        out.insert(0, '_');
    }
    if is_reserved(&out) {
        out.push('_');
    }
    out
}

pub(super) fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    c.next().map_or_else(String::new, |f| {
        f.to_uppercase().collect::<String>() + c.as_str()
    })
}

/// `ListAccounts` -> `listAccounts`, `IAMLogin` -> `iamLogin`, `GET` -> `get`.
pub(super) fn lower_camel(name: &str) -> String {
    let chars: Vec<char> = name.chars().collect();
    let upper_run = chars.iter().take_while(|c| c.is_ascii_uppercase()).count();
    let lower_to = match upper_run {
        0 => 0,
        1 => 1,
        n if n == chars.len() => n,
        n => n - 1,
    };
    chars
        .iter()
        .enumerate()
        .map(|(i, c)| {
            if i < lower_to {
                c.to_ascii_lowercase()
            } else {
                *c
            }
        })
        .collect()
}

/// A parameter name for a header: `Idempotency-Key` -> `idempotencyKey`.
pub(super) fn header_param_name(header: &str) -> String {
    let words: Vec<&str> = header
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();
    let mut out = String::new();
    for (i, w) in words.iter().enumerate() {
        if i == 0 {
            out.push_str(&lower_camel(w));
        } else {
            out.push_str(&capitalize(&w.to_lowercase()));
        }
    }
    sanitize_ident(&out)
}

/// A v1 path in v2 form: `:id` segments become `{id}`.
pub(super) fn v2_path(path: &str) -> String {
    let mut out = String::new();
    let mut chars = path.chars().peekable();
    let mut at_segment_start = false;
    while let Some(c) = chars.next() {
        if c == '/' {
            out.push(c);
            at_segment_start = true;
        } else if c == ':' && at_segment_start {
            let mut name = String::new();
            while let Some(n) = chars
                .peek()
                .copied()
                .filter(|n| n.is_ascii_alphanumeric() || *n == '_')
            {
                name.push(n);
                chars.next();
            }
            out.push('{');
            out.push_str(&name);
            out.push('}');
            at_segment_start = false;
        } else {
            out.push(c);
            at_segment_start = false;
        }
    }
    out
}

/// Names of the `{param}` segments of a v2 path, in order.
pub(super) fn path_params(path: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = path;
    while let Some(s) = rest.find('{') {
        let after = &rest[s + 1..];
        let Some(e) = after.find('}') else { break };
        out.push(after[..e].to_string());
        rest = &after[e + 1..];
    }
    out
}

/// Joins an app-level prefix and a module prefix into one service prefix.
pub(super) fn join_prefix(app: Option<&str>, module: Option<&str>) -> String {
    let a = app.unwrap_or("").trim_end_matches('/');
    let m = module.unwrap_or("");
    let joined = format!("{a}{m}");
    let joined = v2_path(&joined);
    if joined.is_empty() {
        "/".to_string()
    } else {
        joined
    }
}

/// The module path and output path of a source file.
pub(super) fn module_of(rel: &str) -> (String, String, bool) {
    let stem = rel.strip_suffix(".veld").unwrap_or(rel);
    let mut changed = false;
    let segs: Vec<String> = stem
        .split('/')
        .filter(|s| !s.is_empty() && *s != "." && *s != "..")
        .map(|s| {
            let clean = sanitize_ident(s);
            changed |= clean != s;
            clean
        })
        .collect();
    (segs.join("."), format!("{}.veld", segs.join("/")), changed)
}

/// A status for a well-known HTTP error name that the prelude lacks.
pub(super) fn inferred_status(name: &str) -> Option<u16> {
    let known = [
        ("PaymentRequired", 402),
        ("MethodNotAllowed", 405),
        ("NotAcceptable", 406),
        ("RequestTimeout", 408),
        ("Gone", 410),
        ("PayloadTooLarge", 413),
        ("UnsupportedMediaType", 415),
        ("UnprocessableEntity", 422),
        ("Locked", 423),
        ("InternalServerError", 500),
        ("NotImplemented", 501),
        ("BadGateway", 502),
        ("ServiceUnavailable", 503),
        ("GatewayTimeout", 504),
    ];
    if let Some((_, s)) = known.iter().find(|(n, _)| *n == name) {
        return Some(*s);
    }
    let lower = name.to_lowercase();
    if lower.contains("exists") || lower.contains("duplicate") || lower.contains("conflict") {
        return Some(409);
    }
    if lower.contains("notfound") {
        return Some(404);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camel_casing_handles_acronyms() {
        assert_eq!(lower_camel("ListAccounts"), "listAccounts");
        assert_eq!(lower_camel("IAMLogin"), "iamLogin");
        assert_eq!(lower_camel("GET"), "get");
        assert_eq!(lower_camel("banaccount"), "banaccount");
        assert_eq!(lower_camel("X"), "x");
    }

    #[test]
    fn header_names_become_camel_case_parameters() {
        assert_eq!(header_param_name("Idempotency-Key"), "idempotencyKey");
        assert_eq!(header_param_name("Authorization"), "authorization");
        assert_eq!(header_param_name("X-API-Key"), "xApiKey");
        assert_eq!(header_param_name("stream"), "stream_");
    }

    #[test]
    fn paths_convert_colon_params() {
        assert_eq!(v2_path("/:id/freeze"), "/{id}/freeze");
        assert_eq!(v2_path("/a/{x}/b/:y"), "/a/{x}/b/{y}");
        assert_eq!(v2_path("/"), "/");
        assert_eq!(path_params("/a/{x}/b/{y}"), ["x", "y"]);
    }

    #[test]
    fn prefixes_join_and_default_to_root() {
        assert_eq!(join_prefix(Some("/v1"), Some("/api/x")), "/v1/api/x");
        assert_eq!(join_prefix(None, Some("/api")), "/api");
        assert_eq!(join_prefix(None, None), "/");
    }

    #[test]
    fn module_paths_are_sanitized() {
        assert_eq!(
            module_of("models/account.veld"),
            ("models.account".into(), "models/account.veld".into(), false)
        );
        let (m, p, changed) = module_of("my-dir/error.veld");
        assert_eq!(
            (m.as_str(), p.as_str(), changed),
            ("my_dir.error_", "my_dir/error_.veld", true)
        );
    }

    #[test]
    fn identifiers_are_sanitized() {
        assert_eq!(sanitize_ident("in-progress"), "in_progress");
        assert_eq!(sanitize_ident("1st"), "_1st");
        assert_eq!(sanitize_ident("error"), "error_");
        assert!(is_ident("ok_1") && !is_ident("1ok") && !is_ident("a-b"));
    }
}

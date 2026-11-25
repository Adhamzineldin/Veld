//! Policy ids (`spec/rules.md` section 11.2).

use super::*;
use crate::testing::ir;

const SRC: &str = "model U { x: int }\npolicy P(u: U) {\n  permit when u.x > 1\n  forbid when u.x > 9 message \"no\"\n}";

fn clauses(p: &veld_ir::Policy) -> Vec<(PolicyEffect, &Expr)> {
    p.clauses.iter().map(|c| (c.effect, &c.expr)).collect()
}

#[test]
fn the_id_has_the_documented_form() {
    let ir = ir(SRC);
    let id = &ir.policies[0].id;
    assert!(id.starts_with("veld:app/P@"), "{id}");
    let hex = &id["veld:app/P@".len()..];
    assert_eq!(hex.len(), 16);
    assert!(hex.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f')));
}

#[test]
fn the_hashed_bytes_are_the_qualified_name_and_the_clause_shapes() {
    let ir = ir(SRC);
    let p = &ir.policies[0];
    assert_eq!(
        policy_bytes(&ir.decls, &p.qualified, &clauses(p)),
        concat!(
            r#"{"c":[{"e":{"e":{"bin":{"l":{"e":{"path":{"root":{"param":"u"},"steps":[{"w":"x"}]}},"t":"int"},"#,
            r#""op":"gt","r":{"e":{"lit":{"int":"1"}},"t":"int"}}},"t":"bool"},"k":"permit"},"#,
            r#"{"e":{"e":{"bin":{"l":{"e":{"path":{"root":{"param":"u"},"steps":[{"w":"x"}]}},"t":"int"},"#,
            r#""op":"gt","r":{"e":{"lit":{"int":"9"}},"t":"int"}}},"t":"bool"},"k":"forbid"}],"#,
            r#""d":"app:app.P"}"#
        )
    );
    assert_eq!(
        policy_id(&ir.decls, "app", &p.name, &p.qualified, &clauses(p)),
        p.id
    );
}

#[test]
fn a_message_is_not_part_of_the_hashed_bytes() {
    let ir = ir(SRC);
    let p = &ir.policies[0];
    assert!(!policy_bytes(&ir.decls, &p.qualified, &clauses(p)).contains("\"no\""));
}

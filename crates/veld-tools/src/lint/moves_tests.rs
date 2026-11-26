//! `post-moves-state-as-created` (`L0018`).

use std::collections::BTreeMap;

use super::*;
use crate::testutil::ir;

const HEAD: &str = "enum Status { open, done, flow open -> done }\n\
model Task { id: uuid, status: Status = .open }\n\
model Other { id: uuid }\n";

fn found(body: &str) -> Vec<LintIssue> {
    let src = format!("{HEAD}service S at \"/s\" {{\n{body}\n}}");
    lint(&ir(&src), &LintConfig::default())
        .into_iter()
        .filter(|i| i.rule == "post-moves-state-as-created")
        .collect()
}

#[test]
fn a_post_that_moves_a_state_and_answers_201_is_reported_with_the_fix() {
    let issues = found(
        "  post \"/{id}/done\" finish(id: uuid) -> Task {\n    fact task: Task = id\n    transition task.status: open -> done\n  }",
    );
    assert_eq!(issues.len(), 1);
    let i = &issues[0];
    assert_eq!(
        (i.code, i.level, i.subject.as_str()),
        ("L0018", LintLevel::Warn, "S.finish")
    );
    assert!(i.message.contains("-> 200 Task"), "{}", i.message);
}

#[test]
fn writing_200_silences_it() {
    let issues = found(
        "  post \"/{id}/done\" finish(id: uuid) -> 200 Task {\n    fact task: Task = id\n    transition task.status: open -> done\n  }",
    );
    assert!(issues.is_empty(), "{issues:?}");
}

#[test]
fn another_answer_type_another_method_or_no_transition_is_not_reported() {
    for body in [
        "  post \"/{id}/done\" finish(id: uuid) -> Other {\n    fact task: Task = id\n    transition task.status: open -> done\n  }",
        "  post \"/{id}/done\" finish(id: uuid) -> 204 {\n    fact task: Task = id\n    transition task.status: open -> done\n  }",
        "  put \"/{id}/done\" finish(id: uuid) -> Task {\n    fact task: Task = id\n    transition task.status: open -> done\n  }",
        "  post \"/{id}\" make(id: uuid) -> Task {\n    fact task: Task = id\n  }",
        "  post \"/make\" make() -> Task",
    ] {
        assert!(found(body).is_empty(), "{body}");
    }
}

#[test]
fn the_level_comes_from_the_lint_table() {
    let src = format!(
        "{HEAD}service S at \"/s\" {{\n  post \"/{{id}}/done\" finish(id: uuid) -> Task {{\n    fact task: Task = id\n    transition task.status: open -> done\n  }}\n}}"
    );
    let levels = BTreeMap::from([("post-moves-state-as-created".to_owned(), LintLevel::Error)]);
    let config = LintConfig::from_levels(&levels).unwrap();
    let issues = lint(&ir(&src), &config);
    assert!(
        issues
            .iter()
            .any(|i| i.rule == "post-moves-state-as-created" && i.level == LintLevel::Error)
    );
}

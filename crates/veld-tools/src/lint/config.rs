use std::collections::BTreeMap;
use std::fmt;

use veld_codegen::LintLevel;
use veld_compiler::codes::LINT_RULES;

/// A `[lint]` key that is not a rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownRule {
    pub name: String,
    /// All valid rule names.
    pub valid: Vec<&'static str>,
}

impl fmt::Display for UnknownRule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "unknown lint rule `{}` in `[lint]`; expected one of: {}",
            self.name,
            self.valid.join(", ")
        )
    }
}

impl std::error::Error for UnknownRule {}

/// Rule levels. A rule that is not set is `warn`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LintConfig {
    levels: BTreeMap<&'static str, LintLevel>,
}

impl LintConfig {
    /// Builds the config from the `[lint]` table (`Project::lint`).
    ///
    /// # Errors
    /// The first key that is not a rule name.
    pub fn from_levels<'a>(
        levels: impl IntoIterator<Item = (&'a String, &'a LintLevel)>,
    ) -> Result<Self, UnknownRule> {
        let mut out = BTreeMap::new();
        for (name, level) in levels {
            let Some((_, rule)) = LINT_RULES.iter().find(|(_, n)| n == name) else {
                return Err(UnknownRule {
                    name: name.clone(),
                    valid: LINT_RULES.iter().map(|(_, n)| *n).collect(),
                });
            };
            out.insert(*rule, *level);
        }
        Ok(Self { levels: out })
    }

    /// The level of `rule`.
    #[must_use]
    pub fn level(&self, rule: &str) -> LintLevel {
        self.levels.get(rule).copied().unwrap_or(LintLevel::Warn)
    }

    /// Every rule at `error` (what `veld check --strict` uses).
    #[must_use]
    pub fn strict(mut self) -> Self {
        for (_, name) in LINT_RULES {
            if self.level(name) != LintLevel::Off {
                self.levels.insert(name, LintLevel::Error);
            }
        }
        self
    }
}

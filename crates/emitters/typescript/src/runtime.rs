//! The embedded runtime. One real `.ts` file, byte-identical in every role.
//!
//! `runtime/veld.ts` has regions between `// #if <name>` and `// #endif` marker lines
//! (`veld_codegen::gate_regions`). A contract without rules gets the file without them, exactly
//! as it was before rules existed; a contract with invariants also gets the `rules` regions and
//! `runtime/rules.ts` (`spec/targets/typescript.md` section 5b). A contract with endpoint rules
//! gets the `endpoint_rules` regions as well (section 5c); a contract with invariants only never
//! sees them. A contract with a `transition` or an `allow` (or a read of the principal) also gets
//! the `transitions` and `policies` regions (sections 5d and 5e); the rest never sees them.

use std::sync::LazyLock;

use veld_codegen::{FileSet, GateError, GenDiagnostic, gate_regions};

use crate::util::add_file;

/// The embedded `runtime/veld.ts`, markers and all.
const RAW: &str = include_str!("../runtime/veld.ts");

/// The embedded `runtime/rules.ts`: the rule helpers, emitted only for a contract with rules.
pub const RULES_SOURCE: &str = include_str!("../runtime/rules.ts");

/// The embedded `runtime/rollout.ts`: the sink audited rules report to, emitted only for a
/// contract with an audited rule (`@rollout(audit)`, `spec/rules.md` section 17.1).
pub const ROLLOUT_SOURCE: &str = include_str!("../runtime/rollout.ts");

/// The embedded `runtime/flags.ts`: the readings of the flags a request has, emitted only for a
/// contract with a flagged statement (`spec/platform.md` section 11).
pub const FLAGS_SOURCE: &str = include_str!("../runtime/flags.ts");

/// Source of `runtime/veld.ts` for a contract without rules.
pub static SOURCE: LazyLock<String> = LazyLock::new(|| source(false).unwrap_or_default());

/// Root-relative module path of the runtime, without extension.
pub const MODULE: &str = "runtime/veld";

/// Root-relative module path of the rule helpers, without extension.
pub const RULES_MODULE: &str = "runtime/rules";

/// `runtime/veld.ts` with the `rules` regions kept (`rules`) or dropped.
///
/// # Errors
/// The embedded file has markers that do not pair up.
pub fn source(rules: bool) -> Result<String, GateError> {
    source_with(rules, false)
}

/// Which regions of `runtime/veld.ts` a contract needs.
///
/// A state machine or a policy is an endpoint rule, and an endpoint rule needs the invariant
/// machinery, so [`Gates::normalised`] switches on what each flag builds on.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Gates {
    /// Invariants: the `rules` regions and `runtime/rules.ts`.
    pub rules: bool,
    /// Facts, `require` and `ensure`: the `endpoint_rules` regions.
    pub endpoint_rules: bool,
    /// A `transition`: the `transitions` regions.
    pub transitions: bool,
    /// An `allow` or a read of the principal: the `policies` regions.
    pub policies: bool,
    /// An audited rule: the `rollout` regions and `runtime/rollout.ts`.
    pub rollout: bool,
    /// A `@version` guard or a response that carries an `ETag`: the `version` regions
    /// (`spec/rules.md` section 11.7).
    pub version: bool,
    /// An `@idempotent` endpoint: the `idempotency` regions (`spec/rules.md` section 11.8).
    pub idempotency: bool,
    /// A derived field (`spec/rules.md` section 17.2): the `derived` regions. They need the
    /// invariant machinery (`rules`), because the expressions call the helpers of
    /// `runtime/rules.ts` and a derived value is read by invariants.
    pub derived: bool,
    /// A timed `transition` (`spec/platform.md` section 10): the `timed` regions. They need the
    /// transition guard (`transitions`).
    pub timed: bool,
    /// An endpoint whose move emits an event (`spec/events.md` section 11): the `outbox` regions.
    /// They need the transition guard (`transitions`).
    pub outbox: bool,
    /// An endpoint with a `@timeout` (`spec/platform.md` section 12): the `timeouts` regions (the
    /// server's deadline, the client's default timeout).
    pub timeouts: bool,
    /// An endpoint with an `@slo` in a server tree (`spec/platform.md` section 13): the `slo`
    /// regions and the `metrics` hook.
    pub slo: bool,
    /// A flagged statement (`spec/platform.md` section 11): the `flags` regions and
    /// `runtime/flags.ts`.
    pub flags: bool,
    /// The target has a typed runtime config (`spec/runtime-config.md`): a client takes its
    /// `baseUrl` from the options alone (no environment variable, no compiled-in address).
    pub runtime_config: bool,
}

impl Gates {
    /// The same gates with everything they build on switched on.
    #[must_use]
    pub fn normalised(self) -> Self {
        let transitions = self.transitions || self.timed || self.outbox;
        let endpoint_rules = self.endpoint_rules || transitions || self.policies;
        Self {
            rules: self.rules || endpoint_rules || self.rollout || self.derived,
            endpoint_rules,
            transitions,
            ..self
        }
    }

    /// The gates two trees need together (the server's own tree and its consumed clients).
    #[must_use]
    pub fn union(self, other: Self) -> Self {
        Self {
            rules: self.rules || other.rules,
            endpoint_rules: self.endpoint_rules || other.endpoint_rules,
            transitions: self.transitions || other.transitions,
            timed: self.timed || other.timed,
            outbox: self.outbox || other.outbox,
            timeouts: self.timeouts || other.timeouts,
            slo: self.slo || other.slo,
            flags: self.flags || other.flags,
            policies: self.policies || other.policies,
            rollout: self.rollout || other.rollout,
            version: self.version || other.version,
            idempotency: self.idempotency || other.idempotency,
            derived: self.derived || other.derived,
            runtime_config: self.runtime_config || other.runtime_config,
        }
    }
}

/// `runtime/veld.ts` with the `rules` and `endpoint_rules` regions kept or dropped. Endpoint rules
/// need the invariant machinery, so `endpoint_rules` keeps the `rules` regions too.
///
/// # Errors
/// The embedded file has markers that do not pair up.
pub fn source_with(rules: bool, endpoint_rules: bool) -> Result<String, GateError> {
    source_gated(Gates {
        rules,
        endpoint_rules,
        ..Gates::default()
    })
}

/// `runtime/veld.ts` with the regions `gates` asks for.
///
/// # Errors
/// The embedded file has markers that do not pair up.
pub fn source_gated(gates: Gates) -> Result<String, GateError> {
    let g = gates.normalised();
    gate_regions(RAW, &|name| match name {
        "rules" => g.rules,
        "rollout" => g.rollout,
        // Audited `require` and `ensure` need both.
        "rollout_endpoint" => g.rollout && g.endpoint_rules,
        "endpoint_rules" => g.endpoint_rules,
        "transitions" => g.transitions,
        // Timed moves (`spec/platform.md` section 10) and what a build without them has.
        "timed" => g.timed,
        "not_timed" => !g.timed,
        // Deadlines (`spec/platform.md` section 12), service levels (section 13) and the code that
        // wraps a request for either.
        "timeouts" => g.timeouts,
        "not_timeouts" => !g.timeouts,
        "timeouts_idempotency" => g.timeouts && g.idempotency,
        "timeouts_rules" => g.timeouts && g.rules,
        "not_timeouts_rules" => g.timeouts && !g.rules,
        "slo" => g.slo,
        "bounded" => g.timeouts || g.slo,
        "not_bounded" => !(g.timeouts || g.slo),
        "timeouts_only" => g.timeouts && !g.slo,
        "slo_only" => g.slo && !g.timeouts,
        "timeouts_slo" => g.timeouts && g.slo,
        // Flags (`spec/platform.md` section 11), alone and inside the regions they work in.
        "flags" => g.flags,
        "not_flags" => !g.flags,
        "flags_rules" => g.flags && g.rules,
        "not_flags_rules" => g.rules && !g.flags,
        "flags_policies" => g.flags && g.policies,
        "not_flags_policies" => g.policies && !g.flags,
        "flags_transitions" => g.flags && g.transitions,
        "not_flags_transitions" => g.transitions && !g.flags,
        "flags_endpoint_rules" => g.flags && g.endpoint_rules,
        "not_flags_endpoint_rules" => !g.flags && g.endpoint_rules,
        "timed_plain" => g.timed && !(g.timeouts || g.slo),
        "timed_bounded" => g.timed && (g.timeouts || g.slo),
        "policies" => g.policies,
        "outbox" => g.outbox,
        // The code the transition guard and `allow` share.
        "guards" => g.transitions || g.policies || (g.version && g.endpoint_rules),
        "version" => g.version,
        // The server's guard needs the endpoint machinery (a guard has a fact).
        "version_guard" => g.version && g.endpoint_rules,
        "idempotency" => g.idempotency,
        // `EarlyResponse` for a contract with `@idempotent` and no endpoint rules: the test handler needs it.
        "idempotency_only" => g.idempotency && !g.endpoint_rules,
        // The two header checks of step 5.
        "headers" => g.version || g.idempotency,
        // The part of `headerRefusal` that merges the issues of a rule.
        "headers_rules" => (g.version || g.idempotency) && g.rules,
        // What a build without them has in the same place.
        "not_version" => !g.version,
        "not_idempotency" => !g.idempotency,
        // Derived fields (`spec/rules.md` section 17.2) and what a build without them has.
        "derived" => g.derived,
        "not_derived" => !g.derived,
        // `EarlyResponse` exists when there are endpoint rules or an `@idempotent` endpoint.
        "derived_early" => g.derived && (g.endpoint_rules || g.idempotency),
        "derived_plain" => g.derived && !(g.endpoint_rules || g.idempotency),
        // The same, inside a region of the invariants or of endpoint rules.
        "rules_not_derived" => g.rules && !g.derived,
        "endpoint_rules_derived" => g.endpoint_rules && g.derived,
        "endpoint_rules_plain" => g.endpoint_rules && !g.derived,
        // The text of the rule values: endpoint rules print them, and so do derived fields.
        "text_forms" => g.endpoint_rules || g.derived,
        // The client's address: the option alone, or the option, the environment and the contract.
        "runtime_config" => g.runtime_config,
        "not_runtime_config" => !g.runtime_config,
        _ => false,
    })
}

/// Adds `runtime/veld.ts` to `files`, and `runtime/rules.ts` when the contract has rules.
pub fn emit(files: &mut FileSet, rules: bool) -> Result<(), Vec<GenDiagnostic>> {
    emit_with(files, rules, false)
}

/// Like [`emit`], with the regions of endpoint rules when `endpoint_rules` is set.
pub fn emit_with(
    files: &mut FileSet,
    rules: bool,
    endpoint_rules: bool,
) -> Result<(), Vec<GenDiagnostic>> {
    emit_gated(
        files,
        Gates {
            rules,
            endpoint_rules,
            ..Gates::default()
        },
    )
}

/// Like [`emit`], with the regions `gates` asks for.
pub fn emit_gated(files: &mut FileSet, gates: Gates) -> Result<(), Vec<GenDiagnostic>> {
    let gates = gates.normalised();
    let text = source_gated(gates)
        .map_err(|e| vec![GenDiagnostic::new("E0805", format!("runtime/veld.ts: {e}"))])?;
    add_file(files, "runtime/veld.ts", text)?;
    if gates.rules {
        add_file(files, "runtime/rules.ts", RULES_SOURCE.to_string())?;
    }
    if gates.rollout {
        add_file(files, "runtime/rollout.ts", ROLLOUT_SOURCE.to_string())?;
    }
    if gates.flags {
        add_file(files, "runtime/flags.ts", FLAGS_SOURCE.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_embedded_markers_pair_up_in_every_mode() {
        assert!(source(false).is_ok());
        assert!(source(true).is_ok());
        assert!(source_with(true, true).is_ok());
        for transitions in [false, true] {
            for policies in [false, true] {
                let gates = Gates {
                    rules: true,
                    endpoint_rules: true,
                    transitions,
                    policies,
                    ..Gates::default()
                };
                assert!(source_gated(gates).is_ok(), "{gates:?}");
            }
        }
    }

    #[test]
    fn a_contract_without_rules_gets_a_file_with_no_markers() {
        assert!(!SOURCE.is_empty());
        assert!(!SOURCE.contains("// #if"));
        assert!(!SOURCE.contains("// #endif"));
    }

    #[test]
    fn rules_are_a_second_file_and_extra_lines_only_when_used() {
        let mut off = FileSet::new();
        emit(&mut off, false).unwrap();
        assert_eq!(off.len(), 1);
        let mut on = FileSet::new();
        emit(&mut on, true).unwrap();
        assert_eq!(on.len(), 2);
        assert!(on.get("runtime/rules.ts").is_some());
    }

    #[test]
    fn endpoint_rules_add_regions_and_invariants_only_never_see_them() {
        let invariants = source(true).unwrap();
        let endpoint = source_with(true, true).unwrap();
        assert!(!invariants.contains("EndpointRuleSet"));
        // The digest belongs to the invariants: a service that reaches one has it (section 9.7).
        assert!(invariants.contains("onContractMismatch"));
        assert!(!source(false).unwrap().contains("onContractMismatch"));
        assert!(endpoint.contains("EndpointRuleSet"));
        assert!(endpoint.contains("onContractMismatch"));
        // Endpoint rules alone switch the invariant machinery on.
        assert_eq!(source_with(false, true).unwrap(), endpoint);
        let mut files = FileSet::new();
        emit_with(&mut files, false, true).unwrap();
        assert!(files.get("runtime/rules.ts").is_some());
    }

    #[test]
    fn version_and_idempotency_add_their_regions_and_nothing_else_sees_them() {
        let plain = source_with(true, true).unwrap();
        for word in [
            "parseIfMatch",
            "withEtag",
            "checkVersion",
            "IdempotencyStore",
            "MemoryIdempotencyStore",
            "sha256Hex",
            "headerIssues",
            "runCore",
        ] {
            assert!(!plain.contains(word), "`{word}` leaked into the runtime");
        }
        let version = source_gated(Gates {
            version: true,
            endpoint_rules: true,
            ..Gates::default()
        })
        .unwrap();
        for word in [
            "parseIfMatch",
            "withEtag",
            "checkVersion",
            "headerIssues",
            "refusal",
        ] {
            assert!(version.contains(word), "`{word}` is missing");
        }
        assert!(!version.contains("IdempotencyStore") && !version.contains("runCore"));
        let idempotency = source_gated(Gates {
            idempotency: true,
            ..Gates::default()
        })
        .unwrap();
        for word in [
            "IdempotencyStore",
            "MemoryIdempotencyStore",
            "sha256Hex",
            "runCore",
            "headerIssues",
        ] {
            assert!(idempotency.contains(word), "`{word}` is missing");
        }
        assert!(!idempotency.contains("parseIfMatch") && !idempotency.contains("checkVersion"));
    }

    #[test]
    fn the_new_regions_pair_up_in_every_combination() {
        for bits in 0..256u32 {
            let gates = Gates {
                rules: bits & 1 != 0,
                endpoint_rules: bits & 2 != 0,
                transitions: bits & 4 != 0,
                policies: bits & 8 != 0,
                version: bits & 16 != 0,
                idempotency: bits & 32 != 0,
                derived: bits & 64 != 0,
                rollout: bits & 128 != 0,
                ..Gates::default()
            };
            assert!(source_gated(gates).is_ok(), "{gates:?}");
        }
    }

    #[test]
    fn derived_fields_add_their_regions_and_nothing_else_sees_them() {
        let words = [
            "deriveNode",
            "deriveValue",
            "deriveSent",
            "deriveInput",
            "toWireSend",
            "DeriveFn",
            "withDerived",
            "principalSchema",
        ];
        let plain = source_with(true, true).unwrap();
        for word in words {
            assert!(!plain.contains(word), "`{word}` leaked into the runtime");
        }
        let derived = source_gated(Gates {
            derived: true,
            ..Gates::default()
        })
        .unwrap();
        for word in words {
            assert!(derived.contains(word), "`{word}` is missing");
        }
        // A derived field needs the invariant machinery, and nothing of endpoint rules.
        assert!(derived.contains("checkValue") && !derived.contains("EndpointRuleSet"));
        // The same functions keep working for a server with endpoint rules.
        let all = source_gated(Gates {
            derived: true,
            endpoint_rules: true,
            policies: true,
            ..Gates::default()
        })
        .unwrap();
        // The principal is completed in `guard` (every endpoint), so even a server with no endpoint
        // rule has the schema member and the failure answer.
        assert!(all.contains("loadedFailed") && all.contains("principalSchema"));
        assert!(derived.contains("loadedFailed") && derived.contains("principalSchema"));
        assert!(!plain.contains("loadedFailed"));
    }

    #[test]
    fn every_combination_with_derived_fields_pairs_up_and_keeps_no_marker() {
        for bits in 0..16384u32 {
            let gates = Gates {
                rules: bits & 1 != 0,
                endpoint_rules: bits & 2 != 0,
                transitions: bits & 4 != 0,
                policies: bits & 8 != 0,
                version: bits & 16 != 0,
                idempotency: bits & 32 != 0,
                rollout: bits & 64 != 0,
                runtime_config: bits & 128 != 0,
                derived: bits & 256 != 0,
                timed: bits & 512 != 0,
                timeouts: bits & 1024 != 0,
                slo: bits & 2048 != 0,
                flags: bits & 4096 != 0,
                outbox: bits & 8192 != 0,
            };
            let text = source_gated(gates).unwrap();
            assert!(!text.contains("// #"), "{gates:?}");
        }
    }

    #[test]
    fn a_version_only_contract_has_the_etag_and_no_guard() {
        let etag = source_gated(Gates {
            version: true,
            ..Gates::default()
        })
        .unwrap();
        assert!(etag.contains("withEtag") && etag.contains("ifMatchText"));
        // The guard needs the endpoint machinery; an `ETag` alone does not.
        assert!(!etag.contains("checkVersion") && !etag.contains("EndpointRuleSet"));
    }

    #[test]
    fn timed_moves_add_their_regions_and_nothing_else_sees_them() {
        let words = [
            "runDueMoves",
            "checkDue",
            "timerCalls",
            "DueStep",
            "TimerHooks",
        ];
        let plain = source_gated(Gates {
            transitions: true,
            ..Gates::default()
        })
        .unwrap();
        for word in words {
            assert!(!plain.contains(word), "`{word}` leaked into the runtime");
        }
        let timed = source_gated(Gates {
            timed: true,
            ..Gates::default()
        })
        .unwrap();
        for word in words {
            assert!(timed.contains(word), "`{word}` is missing");
        }
        // A timed move needs the transition guard.
        assert!(timed.contains("checkTransition"));
    }

    #[test]
    fn the_outbox_adds_its_regions_and_nothing_else_sees_them() {
        let words = [
            "EmitSession",
            "emitByRequest",
            "checkWritten",
            "beginEmit",
            "checkEmitted",
            "skipEmitted",
            "emittedOf",
        ];
        let plain = source_gated(Gates {
            transitions: true,
            ..Gates::default()
        })
        .unwrap();
        for word in words {
            assert!(!plain.contains(word), "`{word}` leaked into the runtime");
        }
        let outbox = source_gated(Gates {
            outbox: true,
            ..Gates::default()
        })
        .unwrap();
        for word in words {
            assert!(outbox.contains(word), "`{word}` is missing");
        }
        // The outbox needs the transition guard, and a server without it fails when it is created.
        assert!(outbox.contains("checkTransition"));
        assert!(outbox.contains("hooks.outbox is required"));
    }

    #[test]
    fn deadlines_and_service_levels_add_their_regions_and_nothing_else_sees_them() {
        let plain = source_gated(Gates::default()).unwrap();
        for word in [
            "withDeadline",
            "deadlineAnswer",
            "withMetrics",
            "reportMetric",
            "callLimit",
        ] {
            assert!(!plain.contains(word), "`{word}` leaked into the runtime");
        }
        let timeouts = source_gated(Gates {
            timeouts: true,
            ..Gates::default()
        })
        .unwrap();
        for word in ["withDeadline", "deadlineAnswer", "callLimit", "timeoutMs"] {
            assert!(timeouts.contains(word), "`{word}` is missing");
        }
        assert!(!timeouts.contains("withMetrics"));
        let slo = source_gated(Gates {
            slo: true,
            ..Gates::default()
        })
        .unwrap();
        assert!(slo.contains("withMetrics") && slo.contains("reportMetric"));
        assert!(!slo.contains("withDeadline"));
        let both = source_gated(Gates {
            slo: true,
            timeouts: true,
            idempotency: true,
            ..Gates::default()
        })
        .unwrap();
        assert!(both.contains("withMetrics((r2, m2, q2, s2, h2) => withDeadline"));
        // The deadline settles the lease of an idempotent request.
        assert!(both.contains("lease.closed") && !timeouts.contains("lease.closed"));
    }

    #[test]
    fn flags_add_their_regions_and_nothing_else_sees_them() {
        let words = [
            "FlagProvider",
            "MemoryFlagProvider",
            "readFlags",
            "withFlagState",
            "statementOn",
            "liveSources",
            "withReadings",
        ];
        let all = source_gated(Gates {
            rules: true,
            policies: true,
            transitions: true,
            ..Gates::default()
        })
        .unwrap();
        for word in words {
            assert!(!all.contains(word), "`{word}` leaked into the runtime");
        }
        assert!(!all.contains("checkInputRulesIn") && !all.contains("checkAllowsIn"));
        let flagged = source_gated(Gates {
            flags: true,
            rules: true,
            policies: true,
            transitions: true,
            ..Gates::default()
        })
        .unwrap();
        for word in words {
            assert!(flagged.contains(word), "`{word}` is missing");
        }
        // The windows: every sync walk of the rules runs with the readings of its request.
        for word in [
            "checkInputRulesIn",
            "checkMessageRulesIn",
            "scanOutputIn",
            "checkAllowsIn",
        ] {
            assert!(flagged.contains(word), "`{word}` is missing");
        }
        // Without policies and transitions there is nothing of theirs.
        let bare = source_gated(Gates {
            flags: true,
            ..Gates::default()
        })
        .unwrap();
        assert!(
            bare.contains("readFlags")
                && !bare.contains("liveSources")
                && !bare.contains("checkAllowsIn")
        );
    }

    #[test]
    fn machines_and_policies_add_their_regions_and_endpoint_rules_never_see_them() {
        let endpoint = source_with(true, true).unwrap();
        for word in ["checkAllows", "checkTransition", "refusal", "PolicyVerdict"] {
            assert!(
                !endpoint.contains(word),
                "`{word}` leaked into endpoint rules"
            );
        }
        let machine = source_gated(Gates {
            transitions: true,
            ..Gates::default()
        })
        .unwrap();
        assert!(machine.contains("checkTransition") && machine.contains("refusal"));
        assert!(!machine.contains("checkAllows"));
        let policies = source_gated(Gates {
            policies: true,
            ..Gates::default()
        })
        .unwrap();
        assert!(policies.contains("checkAllows") && policies.contains("refusal"));
        assert!(!policies.contains("checkTransition"));
        // Each switches the endpoint machinery on.
        assert!(machine.contains("EndpointRuleSet") && policies.contains("EndpointRuleSet"));
    }
}

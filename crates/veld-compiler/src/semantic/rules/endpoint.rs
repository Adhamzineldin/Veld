//! Endpoint blocks (`spec/rules.md` section 9): scope, typing and the diagnostics `E0920` to
//! `E0924`, plus the existing `E0901` to `E0908` inside the statements.
//!
//! The checker is the one of the invariants. An endpoint block only changes what a name means:
//! parameters and facts are roots (`Root::Param`, `Root::Fact`), `now` is the clock, `result` and
//! `old(...)` exist in `ensure`. Facts are checked first, in order, so a fact key sees the facts
//! declared before it; `require` and `ensure` then see every fact.

use std::collections::{BTreeMap, BTreeSet};

use super::checker::{Checker, EpScope, Phase, PrincipalScope};
use super::depth;
use super::transition::{self, RTransition};
use super::types::of;
use crate::diagnostics::{Diagnostic, Span};
use crate::language::Target;
use crate::semantic::{DKind, Program, REndpoint, RField, ServiceInfo, annotations};
use crate::syntax::ast::{self, Stmt, doc_text};
use veld_ir::rules::{Expr, ExprType};
use veld_ir::{Constraints, Rollout, Type};

/// Containers above an expression in the IR of an endpoint rule, counted for the deepest
/// place an expression can sit (the value of a payload field of an `else`): the document,
/// `services`, the service, `endpoints`, the endpoint, `rules`, `requires`, the require, `else`,
/// `fields` and the field.
const ENDPOINT_PREFIX_DEPTH: usize = 11;

/// The most containers the JSON of one expression of an endpoint block may nest.
const MAX_JSON_DEPTH: usize =
    depth::SERDE_JSON_MAX_DEPTH - depth::PLUGIN_ENVELOPE_DEPTH - ENDPOINT_PREFIX_DEPTH;

/// The checked statements of one endpoint block.
#[derive(Debug, Clone, Default)]
pub struct RBlock {
    pub facts: Vec<RFact>,
    pub allows: Vec<RAllow>,
    pub transition: Option<RTransition>,
    pub requires: Vec<RRequire>,
    pub ensures: Vec<REnsure>,
    /// The `If-Match` guard (`spec/rules.md` section 11.7).
    pub version: Option<super::RVersion>,
}

#[derive(Debug, Clone)]
pub struct RFact {
    pub name: String,
    pub ty: Type,
    pub optional: bool,
    pub key: Option<Expr>,
    pub key_ty: Option<Type>,
    pub span: Span,
}

/// A checked `allow` (`spec/rules.md` section 11.3).
#[derive(Debug, Clone)]
pub struct RAllow {
    /// Index into `Program::policies`.
    pub policy: usize,
    pub args: Vec<Expr>,
    /// Names of the parameters the arguments read, first use first.
    pub reads: Vec<String>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct RRequire {
    pub name: Option<String>,
    pub expr: Expr,
    pub message: Option<String>,
    pub reads: Vec<String>,
    pub otherwise: Option<RElse>,
    pub docs: Option<String>,
    pub rollout: Option<Rollout>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct RElse {
    pub decl: usize,
    pub status: u16,
    pub fields: Vec<RElseField>,
}

#[derive(Debug, Clone)]
pub struct RElseField {
    pub name: String,
    pub wire: String,
    pub value: Expr,
}

#[derive(Debug, Clone)]
pub struct REnsure {
    pub name: Option<String>,
    pub expr: Expr,
    pub message: Option<String>,
    pub reads: Vec<String>,
    pub docs: Option<String>,
    pub rollout: Option<Rollout>,
    pub span: Span,
}

fn statements(e: &ast::Endpoint) -> &[Stmt] {
    e.block.as_ref().map_or(&[], |b| &b.stmts)
}

/// True when the block has a required fact: the endpoint then implicitly declares `NotFound`.
pub(crate) fn has_required_fact(e: &ast::Endpoint) -> bool {
    statements(e)
        .iter()
        .any(|s| matches!(s, Stmt::Fact(f) if !f.optional))
}

/// True when the block has a `transition`: the endpoint then implicitly declares `InvalidTransition`.
pub(crate) fn has_transition(e: &ast::Endpoint) -> bool {
    statements(e)
        .iter()
        .any(|s| matches!(s, Stmt::Transition(_)))
}

/// True when the block has an `allow`: the endpoint then implicitly declares `Forbidden`.
pub(crate) fn has_allow(e: &ast::Endpoint) -> bool {
    statements(e).iter().any(|s| matches!(s, Stmt::Allow(_)))
}

/// True when a `require` has no `else`: its answer is `ValidationError`.
pub(crate) fn has_bare_require(e: &ast::Endpoint) -> bool {
    statements(e)
        .iter()
        .any(|s| matches!(s, Stmt::Require(r) if r.otherwise.is_none()))
}

/// A parameter or fact as the checker's local scope sees it.
fn scope_field(name: &str, ty: &Type, optional: bool, constraints: Constraints) -> RField {
    RField {
        name: name.to_string(),
        wire: name.to_string(),
        ty: ty.clone(),
        optional,
        default: None,
        derived: false,
        readonly: false,
        version: false,
        inherited_from: None,
        constraints,
        example: None,
        docs: None,
        deprecated: None,
        span: Span::none(),
    }
}

/// The principal of the package of `module`, as the block of endpoint `ep` sees it.
fn principal_scope(prog: &Program<'_>, module: usize, ep: &REndpoint) -> Option<PrincipalScope> {
    let p = prog.principals.get(&prog.modules[module].pkg)?;
    Some(PrincipalScope {
        field: scope_field(
            &p.name,
            &Type::Decl(veld_ir::DeclId(u32::try_from(p.decl).unwrap_or(u32::MAX))),
            false,
            Constraints::default(),
        ),
        available: ep.auth.kind != veld_ir::AuthKind::None,
    })
}

/// `E0920`: on an endpoint that has the principal, a parameter is not called like it.
fn principal_param_clash(
    e: &ast::Endpoint,
    principal: Option<&PrincipalScope>,
    diags: &mut Vec<Diagnostic>,
) {
    let Some(p) = principal.filter(|p| p.available) else {
        return;
    };
    for param in e.params.iter().filter(|x| x.name.name == p.field.name) {
        diags.push(
            Diagnostic::error(
                "E0920",
                format!(
                    "parameter `{}` has the name of the principal",
                    param.name.name
                ),
                param.name.span,
            )
            .with_help("rename it, or name the principal differently: `principal User as caller`"),
        );
    }
}

/// Checks the block of `e`, whose endpoint `ep` was analysed already.
pub(crate) fn check_block(
    prog: &Program<'_>,
    module: usize,
    e: &ast::Endpoint,
    ep: &REndpoint,
    diags: &mut Vec<Diagnostic>,
) -> RBlock {
    let params: Vec<RField> = ep
        .params
        .iter()
        .filter(|p| !p.stream)
        .map(|p| scope_field(&p.name, &p.ty, p.optional, p.constraints.clone()))
        .collect();
    let poisoned = e
        .params
        .iter()
        .map(|p| p.name.name.clone())
        .filter(|n| !ep.params.iter().any(|p| &p.name == n))
        .collect();
    let result = if ep.success_stream {
        None
    } else {
        ep.success_ty.clone()
    };
    transition::reserved_name(e, diags);
    super::version::reserved_name(prog, module, e, diags);
    let principal = principal_scope(prog, module, ep);
    principal_param_clash(e, principal.as_ref(), diags);
    let mut bc = BlockChecker {
        prog,
        module,
        ep,
        diags,
        params,
        facts: Vec::new(),
        principal,
        poisoned,
        result,
        names: BTreeMap::new(),
    };
    let mut block = bc.run(statements(e));
    block.version = super::version_guard(prog, module, e);
    block
}

struct BlockChecker<'p, 'a, 'd> {
    prog: &'p Program<'a>,
    module: usize,
    ep: &'p REndpoint,
    diags: &'d mut Vec<Diagnostic>,
    params: Vec<RField>,
    facts: Vec<RField>,
    principal: Option<PrincipalScope>,
    poisoned: BTreeSet<String>,
    result: Option<Type>,
    /// Names of `require` and `ensure` statements.
    names: BTreeMap<String, Span>,
}

impl BlockChecker<'_, '_, '_> {
    fn run(&mut self, stmts: &[Stmt]) -> RBlock {
        let mut out = RBlock::default();
        for s in stmts {
            if let Stmt::Fact(f) = s {
                out.facts.extend(self.fact(f));
            }
        }
        out.transition = self.transitions(stmts);
        for s in stmts {
            match s {
                Stmt::Fact(_) | Stmt::Transition(_) => {}
                Stmt::Allow(a) => out.allows.extend(self.allow(a)),
                Stmt::Require(r) => out.requires.extend(self.require(r)),
                Stmt::Ensure(r) => out.ensures.extend(self.ensure(r)),
            }
        }
        out
    }

    /// The first `transition` is checked; any other is `E0934`.
    fn transitions(&mut self, stmts: &[Stmt]) -> Option<RTransition> {
        let all = transition::transitions(stmts);
        for extra in all.iter().skip(1) {
            transition::extra_transition(all[0].span, extra, self.diags);
        }
        let scope = transition::Scope {
            prog: self.prog,
            facts: &self.facts,
            poisoned: &self.poisoned,
        };
        scope.check(all.first()?, self.diags)
    }

    /// Runs `f` with a checker for the statement of kind `phase`.
    fn with_checker<T>(
        &mut self,
        phase: Phase,
        lenient: bool,
        f: impl FnOnce(&mut Checker<'_, '_, '_>) -> T,
    ) -> T {
        let mut c = Checker {
            prog: self.prog,
            module: self.module,
            fields: &[],
            poisoned: self.poisoned.clone(),
            diags: &mut *self.diags,
            lambdas: Vec::new(),
            known: BTreeSet::new(),
            bounded: Vec::new(),
            reads: Vec::new(),
            reads_readonly: false,
            ep: Some(EpScope {
                params: self.params.clone(),
                facts: self.facts.clone(),
                result: self.result.clone(),
                phase,
                in_old: false,
                lenient,
                principal: self.principal.clone(),
            }),
        };
        f(&mut c)
    }

    /// `E0005` when the typed expression would not read back from its IR JSON.
    fn depth_ok(&mut self, e: &Expr, span: Span) -> bool {
        if depth::json_depth(e) <= MAX_JSON_DEPTH {
            return true;
        }
        self.diags.push(Diagnostic::error(
            "E0005",
            "this expression nests too deeply to be written as IR JSON",
            span,
        ));
        false
    }

    /// `E0907`: statement names are unique in an endpoint.
    fn name_is_new(&mut self, name: Option<&ast::Ident>) -> bool {
        let Some(n) = name else { return true };
        if let Some(first) = self.names.get(&n.name) {
            self.diags.push(
                Diagnostic::error(
                    "E0907",
                    format!(
                        "statement `{}` is already declared in endpoint `{}`",
                        n.name, self.ep.name
                    ),
                    n.span,
                )
                .with_label(*first, "first declared here"),
            );
            return false;
        }
        self.names.insert(n.name.clone(), n.span);
        true
    }

    // ---- facts -------------------------------------------------------------------

    fn fact(&mut self, f: &ast::FactStmt) -> Option<RFact> {
        let name = f.name.name.as_str();
        if self.fact_name_taken(f) {
            self.poisoned.insert(name.to_string());
            return None;
        }
        let Some(ty) = self
            .prog
            .resolve_type(self.module, &f.ty, false, self.diags)
        else {
            self.poisoned.insert(name.to_string());
            return None;
        };
        let (key, key_ty) = f
            .key
            .as_ref()
            .and_then(|k| self.fact_key(k, f.optional))
            .unzip();
        self.facts
            .push(scope_field(name, &ty, f.optional, Constraints::default()));
        Some(RFact {
            name: name.to_string(),
            ty,
            optional: f.optional,
            key,
            key_ty,
            span: f.name.span,
        })
    }

    /// `E0920`: a fact named like a parameter, another fact, `now` or `result`.
    fn fact_name_taken(&mut self, f: &ast::FactStmt) -> bool {
        let name = f.name.name.as_str();
        let clash = if self.params.iter().any(|p| p.name == name) {
            "a parameter"
        } else if self.facts.iter().any(|x| x.name == name) {
            "another fact"
        } else if name == "now" {
            "the clock"
        } else if name == "result" {
            "the success value"
        } else if self
            .principal
            .as_ref()
            .is_some_and(|p| p.available && p.field.name == name)
        {
            "the principal"
        } else {
            return false;
        };
        self.diags.push(
            Diagnostic::error(
                "E0920",
                format!("fact `{name}` has the name of {clash}"),
                f.name.span,
            )
            .with_help("every name in an endpoint block must be different"),
        );
        true
    }

    fn fact_key(&mut self, k: &ast::Expr, optional: bool) -> Option<(Expr, Type)> {
        let typed = self.with_checker(Phase::Fact, optional, |c| c.expr(k, None))?;
        if !self.depth_ok(&typed, k.span) {
            return None;
        }
        match &typed.t {
            ExprType::Type(t) => {
                let t = t.clone();
                Some((typed, t))
            }
            ExprType::Duration => {
                self.diags.push(Diagnostic::error(
                    "E0902",
                    "the key of a fact cannot be a `duration`",
                    k.span,
                ));
                None
            }
        }
    }

    // ---- allow -------------------------------------------------------------------

    /// `allow Policy(args)` (`spec/rules.md` section 11.3). `E0942` for an unknown policy, a
    /// wrong number of arguments or an argument of the wrong type.
    fn allow(&mut self, a: &ast::AllowStmt) -> Option<RAllow> {
        let policy = self.policy_named(&a.policy)?;
        let params: Vec<(String, Type)> = {
            let res = self.prog.policies[policy].res.as_ref()?;
            res.params
                .iter()
                .map(|p| (p.name.clone(), p.ty.clone()))
                .collect()
        };
        let name = &a.policy.name;
        if params.len() != a.args.len() {
            self.diags.push(
                Diagnostic::error(
                    "E0942",
                    format!(
                        "policy `{name}` takes {} argument{}, found {}",
                        params.len(),
                        if params.len() == 1 { "" } else { "s" },
                        a.args.len()
                    ),
                    a.span,
                )
                .with_label(self.prog.policies[policy].ast.name.span, "declared here"),
            );
            return None;
        }
        let (args, reads) = self.with_checker(Phase::Require, false, |c| {
            let args: Vec<Option<Expr>> = a
                .args
                .iter()
                .zip(&params)
                .map(|(arg, (pname, ty))| {
                    let want = of(ty.clone());
                    let typed = c.expr(arg, Some(&want))?;
                    c.fit_code("E0942", typed, &want, arg.span, |found, expected| {
                        format!(
                            "argument `{pname}` of policy `{name}` is `{expected}`, found `{found}`"
                        )
                    })
                })
                .collect();
            (args, std::mem::take(&mut c.reads))
        });
        let args: Option<Vec<Expr>> = args.into_iter().collect();
        let args = args?;
        if !args.iter().all(|x| self.depth_ok(x, a.span)) {
            return None;
        }
        Some(RAllow {
            policy,
            args,
            reads,
            span: a.span,
        })
    }

    /// The policy `name` as seen from this module; `E0942` when there is none.
    fn policy_named(&mut self, name: &ast::Ident) -> Option<usize> {
        if let Some(p) = self.prog.lookup_policy(self.module, &name.name) {
            return Some(p);
        }
        if self.prog.scopes[self.module].poisoned.contains(&name.name) {
            return None;
        }
        let known: Vec<&str> = self.prog.scopes[self.module]
            .policy_names
            .keys()
            .map(String::as_str)
            .collect();
        let hint = if known.is_empty() {
            "no policy is declared or imported in this module".to_string()
        } else {
            format!("policies in scope: {}", known.join(", "))
        };
        self.diags.push(
            Diagnostic::error(
                "E0942",
                format!("unknown policy `{}`", name.name),
                name.span,
            )
            .did_you_mean(&name.name, known.iter().copied(), Some(hint)),
        );
        None
    }

    // ---- require -----------------------------------------------------------------

    fn require(&mut self, r: &ast::RequireStmt) -> Option<RRequire> {
        let anns = annotations::parse(&r.annotations, Target::Require, None, self.diags);
        let fresh = self.name_is_new(r.name.as_ref());
        let (typed, reads) = self.with_checker(Phase::Require, false, |c| {
            let typed = c.bool_expr(&r.expr, "a `require`");
            (typed, std::mem::take(&mut c.reads))
        });
        let clause = r.otherwise.as_ref().map(|o| self.else_clause(o));
        let broken = matches!(clause, Some(None));
        let expr = typed.filter(|t| self.depth_ok(t, r.expr.span))?;
        if !fresh || broken {
            return None;
        }
        Some(RRequire {
            name: r.name.as_ref().map(|n| n.name.clone()),
            expr,
            message: r.message.as_ref().map(|m| m.value.clone()),
            reads,
            otherwise: clause.flatten(),
            docs: doc_text(&r.docs),
            rollout: anns.rollout,
            span: r.name.as_ref().map_or(r.span, |n| n.span),
        })
    }

    /// The error a failed `require` answers with (`spec/rules.md` section 9.3). The outer
    /// `Option` of the caller is `None` when the clause is broken; a missing clause is `Some(None)`.
    fn else_clause(&mut self, o: &ast::Else) -> Option<RElse> {
        let ty = self
            .prog
            .resolve_type(self.module, &o.error, true, self.diags)?;
        let decl = self.declared_error(&ty, &o.error)?;
        let status = self
            .ep
            .errors
            .iter()
            .find(|(_, d)| *d == decl)
            .map(|(s, _)| *s)?;
        let fields = self.payload(decl, o)?;
        Some(RElse {
            decl,
            status,
            fields,
        })
    }

    /// `E0921`: the error must be one the endpoint declares.
    fn declared_error(&mut self, ty: &Type, written: &ast::TypeExpr) -> Option<usize> {
        let found = match ty {
            Type::Decl(d) if self.prog.decls[d.index()].kind == DKind::Error => Some(d.index()),
            Type::Decl(_) | Type::Scalar(_) | Type::List(_) | Type::Map(_) | Type::Page(_) => None,
        };
        let Some(d) = found else {
            self.diags.push(Diagnostic::error(
                "E0921",
                format!("`{}` is not an error", self.prog.type_name(ty)),
                written.span,
            ));
            return None;
        };
        if self.ep.errors.iter().any(|(_, x)| *x == d) {
            return Some(d);
        }
        let declared: Vec<&str> = self
            .ep
            .errors
            .iter()
            .map(|(_, x)| self.prog.decls[*x].name.as_str())
            .collect();
        self.diags.push(
            Diagnostic::error(
                "E0921",
                format!(
                    "`{}` is not one of the errors of endpoint `{}`",
                    self.prog.decls[d].name, self.ep.name
                ),
                written.span,
            )
            .with_help(format!(
                "add it to the responses (`-> ... | {}`); declared: {}",
                self.prog.decls[d].name,
                declared.join(", ")
            )),
        );
        None
    }

    /// The payload of `else`: every field of the error that is neither optional nor defaulted is
    /// given, nothing else is (`E0922`), and each value has its field's type (`E0902`).
    fn payload(&mut self, decl: usize, o: &ast::Else) -> Option<Vec<RElseField>> {
        let info = &self.prog.decls[decl];
        let given = o.payload.as_deref().unwrap_or(&[]);
        let mut out = Vec::new();
        let mut ok = true;
        let mut seen: BTreeSet<&str> = BTreeSet::new();
        for g in given {
            let Some(field) = info.res.fields.iter().find(|f| f.name == g.name.name) else {
                ok = false;
                self.unknown_payload_field(decl, &g.name);
                continue;
            };
            if !seen.insert(g.name.name.as_str()) {
                ok = false;
                self.diags.push(Diagnostic::error(
                    "E0922",
                    format!("field `{}` is given twice", g.name.name),
                    g.name.span,
                ));
                continue;
            }
            match self.payload_value(&info.name, field, &g.value) {
                Some(value) => out.push(RElseField {
                    name: field.name.clone(),
                    wire: field.wire.clone(),
                    value,
                }),
                None => ok = false,
            }
        }
        let missing: Vec<&str> = info
            .res
            .fields
            .iter()
            .filter(|f| {
                !f.optional && f.default.is_none() && !f.derived && !seen.contains(f.name.as_str())
            })
            .map(|f| f.name.as_str())
            .collect();
        if !missing.is_empty() {
            ok = false;
            self.diags.push(Diagnostic::error(
                "E0922",
                format!(
                    "`else {}` does not give the field{} {}",
                    info.name,
                    if missing.len() == 1 { "" } else { "s" },
                    missing
                        .iter()
                        .map(|m| format!("`{m}`"))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                o.error.span,
            ));
        }
        ok.then_some(out)
    }

    fn unknown_payload_field(&mut self, decl: usize, name: &ast::Ident) {
        let info = &self.prog.decls[decl];
        let names: Vec<&str> = info.res.fields.iter().map(|f| f.name.as_str()).collect();
        let diag = Diagnostic::error(
            "E0922",
            format!("error `{}` has no field `{}`", info.name, name.name),
            name.span,
        )
        .did_you_mean(
            &name.name,
            names.iter().copied(),
            Some(format!("fields: {}", names.join(", "))),
        );
        self.diags.push(diag);
    }

    fn payload_value(&mut self, error: &str, field: &RField, value: &ast::Expr) -> Option<Expr> {
        let want = of(field.ty.clone());
        let typed = self.with_checker(Phase::Require, false, |c| {
            let typed = c.expr(value, Some(&want))?;
            c.fit(typed, &want, value.span, |found, expected| {
                format!(
                    "field `{}` of `{error}` is `{expected}`, found `{found}`",
                    field.name
                )
            })
        })?;
        self.depth_ok(&typed, value.span).then_some(typed)
    }

    // ---- ensure ------------------------------------------------------------------

    fn ensure(&mut self, r: &ast::EnsureStmt) -> Option<REnsure> {
        let anns = annotations::parse(&r.annotations, Target::Ensure, None, self.diags);
        let fresh = self.name_is_new(r.name.as_ref());
        if self.ep.success_stream {
            self.diags.push(
                Diagnostic::error(
                    "E0924",
                    "`ensure` is not allowed on an endpoint whose success is a stream",
                    r.span,
                )
                .with_help("a stream has no single result to check"),
            );
            return None;
        }
        let (typed, reads) = self.with_checker(Phase::Ensure, false, |c| {
            let typed = c.bool_expr(&r.expr, "an `ensure`");
            (typed, std::mem::take(&mut c.reads))
        });
        let expr = typed.filter(|t| self.depth_ok(t, r.expr.span))?;
        if !fresh {
            return None;
        }
        Some(REnsure {
            name: r.name.as_ref().map(|n| n.name.clone()),
            expr,
            message: r.message.as_ref().map(|m| m.value.clone()),
            reads,
            docs: doc_text(&r.docs),
            rollout: anns.rollout,
            span: r.name.as_ref().map_or(r.span, |n| n.span),
        })
    }
}

/// `E0922`: facts of one name must agree in a service, so one loader serves every endpoint.
pub(crate) fn check_service_facts(svc: &ServiceInfo, diags: &mut Vec<Diagnostic>) {
    let mut first: BTreeMap<&str, (&RFact, &str)> = BTreeMap::new();
    for ep in &svc.endpoints {
        for f in &ep.rules.facts {
            let Some((a, at)) = first.get(f.name.as_str()) else {
                first.insert(f.name.as_str(), (f, ep.name.as_str()));
                continue;
            };
            let same = a.ty == f.ty && a.key_ty == f.key_ty && a.optional == f.optional;
            if !same {
                diags.push(
                    Diagnostic::error(
                        "E0922",
                        format!(
                            "fact `{}` differs from the fact of the same name in `{at}`: one loader serves both, so type, key type and optionality must match",
                            f.name
                        ),
                        f.span,
                    )
                    .with_label(a.span, "first declared here"),
                );
            }
        }
    }
}

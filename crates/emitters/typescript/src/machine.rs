//! State machines in rule code (`spec/rules.md` section 10, `spec/targets/typescript.md` section
//! 5d): what the guard of a `transition`, its target check and `can<Endpoint>` share.
//!
//! The states are the wire strings of the machine's enum, which is also the developer-facing
//! representation (`Status` is a union of string literals).

use veld_ir::{
    DeclKind, Endpoint, EndpointFact, EndpointTransition, Enum, Ir, Literal, Root, Step, Type,
};

use crate::expr::EndpointPrinter;
use crate::util::ts_str;

/// The pieces of a `transition` the generators print.
#[derive(Debug)]
pub struct Move<'a> {
    pub transition: &'a EndpointTransition,
    pub machine: &'a Enum,
    pub fact: &'a EndpointFact,
    /// The wire values of the sources, in source order.
    pub sources: Vec<String>,
    /// The wire value of the target.
    pub target: String,
    /// The wire path of the state field inside the fact, segments joined by `.`.
    pub field: String,
    /// The wire value of the field's default, when it has one.
    pub default: Option<String>,
}

/// Why a transition cannot be printed (`E0801`).
pub type Refusal = &'static str;

impl<'a> Move<'a> {
    /// Looks up everything the printers need; says what it cannot represent otherwise.
    ///
    /// # Errors
    /// The reason, for an `E0801`.
    pub fn of(ir: &'a Ir, ep: &'a Endpoint) -> Result<Option<Move<'a>>, Refusal> {
        let Some(t) = &ep.rules.transition else {
            return Ok(None);
        };
        let DeclKind::Enum(machine) = &ir.decl(t.decl).kind else {
            return Err("a transition on something that is not an enum");
        };
        let fact = ep
            .rules
            .facts
            .iter()
            .find(|f| f.name == t.fact)
            .filter(|f| !f.optional)
            .ok_or("a transition on an optional fact")?;
        let wire = |name: &str| {
            machine
                .variants
                .iter()
                .find(|v| v.name == name)
                .map(|v| v.wire.clone())
                .ok_or("a transition to a state the enum does not have")
        };
        let sources = t
            .from
            .iter()
            .map(|s| wire(s))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Some(Move {
            transition: t,
            machine,
            fact,
            sources,
            target: wire(&t.to)?,
            field: t
                .steps
                .iter()
                .map(|s| s.wire.as_str())
                .collect::<Vec<_>>()
                .join("."),
            default: state_default(ir, &fact.ty, &t.steps),
        }))
    }

    /// The array of source states: `['pending', 'paid']`.
    #[must_use]
    pub fn sources_array(&self) -> String {
        let items: Vec<String> = self.sources.iter().map(|s| ts_str(s)).collect();
        format!("[{}]", items.join(", "))
    }

    /// The state a value holds: what `read` reads, or the field's default when it leaves it out.
    #[must_use]
    pub fn state(&self, read: &str) -> String {
        match &self.default {
            Some(d) => format!("{read} ?? {}", ts_str(d)),
            None => read.to_owned(),
        }
    }

    /// The code that reads the state the fact holds.
    #[must_use]
    pub fn current(&self, printer: &EndpointPrinter<'_>) -> Option<String> {
        let read = printer.field_code(
            &Root::Fact(self.transition.fact.clone()),
            &self.transition.steps,
        )?;
        Some(self.state(&read))
    }

    /// The code that reads the state the result holds, when the success type is the fact's own
    /// declaration (the target check exists only then).
    #[must_use]
    pub fn found(&self, printer: &EndpointPrinter<'_>, ep: &Endpoint) -> Option<String> {
        let same = matches!(
            (&self.fact.ty, ep.success.ty.as_ref()),
            (Type::Decl(a), Some(Type::Decl(b))) if a == b
        );
        if !same || ep.success.stream {
            return None;
        }
        let read = printer.field_code(&Root::Result, &self.transition.steps)?;
        Some(self.state(&read))
    }
}

/// The wire value of the default variant of the field the `steps` reach below a value of type
/// `start`, if the field has one: a value read from storage may leave such a field out.
fn state_default(ir: &Ir, start: &Type, steps: &[Step]) -> Option<String> {
    let mut ty = start.clone();
    let mut default = None;
    for step in steps {
        let Type::Decl(id) = &ty else {
            return None;
        };
        let field = ir.decl(*id).fields().iter().find(|f| f.name == step.name)?;
        default = match &field.default {
            Some(Literal::EnumVariant { wire, .. }) => Some(wire.clone()),
            Some(_) | None => None,
        };
        ty = field.ty.clone();
    }
    default
}

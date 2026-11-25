//! Builders for hand-made services, endpoints with rules, and examples.

use veld_ir::{
    Auth, Binding, Constraints, DeclId, ElseError, ElseField, Endpoint, EndpointEnsure,
    EndpointFact, EndpointRequire, EndpointRules, ErrorResponse, Example, ExampleArg,
    ExampleErrorOutcome, ExampleField, ExampleGiven, ExampleOutcome, ExampleRef, ExampleValue,
    Expr, ExprKind, Ir, Method, Origin, PackageId, Param, Root, Service, ServiceId, Success, Type,
};

use super::kit::{Kit, boolean, et, mk, path_ref};

/// A path with several steps from any root; each step is `(name, optional)`.
pub fn rp(root: Root, steps: &[(&str, bool)], ty: Type) -> Expr {
    mk(et(&ty), ExprKind::Path(path_ref(root, steps)))
}
/// `has(..)` from any root.
pub fn hasr(root: Root, steps: &[(&str, bool)]) -> Expr {
    mk(et(&boolean()), ExprKind::Has(path_ref(root, steps)))
}
pub fn old(x: Expr) -> Expr {
    mk(x.t.clone(), ExprKind::Old(Box::new(x)))
}

pub fn param(name: &str, binding: Binding, ty: Type) -> Param {
    Param {
        name: name.to_owned(),
        binding,
        header: None,
        ty,
        optional: false,
        stream: false,
        constraints: Constraints::default(),
    }
}

pub fn decl_id(t: &Type) -> DeclId {
    match t {
        Type::Decl(id) => *id,
        Type::Scalar(_) | Type::List(_) | Type::Map(_) | Type::Page(_) => DeclId(u32::MAX),
    }
}

pub fn fact(name: &str, ty: Type, optional: bool, key: Option<Expr>) -> EndpointFact {
    EndpointFact {
        name: name.to_owned(),
        ty,
        optional,
        key,
        origin: Origin::default(),
    }
}

pub fn else_err(error: &Type, status: u16, fields: Vec<(&str, Expr)>) -> ElseError {
    ElseError {
        decl: decl_id(error),
        status,
        fields: fields
            .into_iter()
            .map(|(n, value)| ElseField {
                name: n.to_owned(),
                wire: n.to_owned(),
                value,
            })
            .collect(),
    }
}

pub fn require(
    name: Option<&str>,
    expr: Expr,
    reads: &[&str],
    else_error: Option<ElseError>,
    message: Option<&str>,
) -> EndpointRequire {
    let id_name = name.unwrap_or("require");
    EndpointRequire {
        name: name.map(str::to_owned),
        id: format!("veld:t/S.ep.{id_name}@0000000000000000"),
        expr,
        message: message.map(str::to_owned),
        reads: reads.iter().map(|r| (*r).to_owned()).collect(),
        else_error,
        docs: None,
        origin: Origin::default(),
    }
}

pub fn ensure(name: Option<&str>, expr: Expr, reads: &[&str]) -> EndpointEnsure {
    let id_name = name.unwrap_or("ensure");
    EndpointEnsure {
        name: name.map(str::to_owned),
        id: format!("veld:t/S.ep.{id_name}@0000000000000000"),
        expr,
        message: None,
        reads: reads.iter().map(|r| (*r).to_owned()).collect(),
        docs: None,
        origin: Origin::default(),
    }
}

/// An endpoint with a success body type (or none) and the listed errors `(status, error)`.
pub fn endpoint(
    name: &str,
    params: Vec<Param>,
    success: Option<Type>,
    errors: &[(u16, &Type)],
    rules: EndpointRules,
) -> Endpoint {
    Endpoint {
        name: name.to_owned(),
        method: Method::Post,
        path: format!("/{name}"),
        sig: format!("veld:t/S.{name}@0000000000000000"),
        docs: None,
        deprecated: None,
        auth: Auth::none(),
        middleware: Vec::new(),
        params,
        success: Success {
            status: 201,
            ty: success,
            stream: false,
        },
        errors: errors
            .iter()
            .map(|(status, t)| ErrorResponse {
                status: *status,
                decl: decl_id(t),
            })
            .collect(),
        rules,
        origin: Origin::default(),
    }
}

pub fn service(endpoints: Vec<Endpoint>) -> Service {
    Service {
        id: ServiceId(0),
        package: PackageId(0),
        name: "S".to_owned(),
        prefix: "/".to_owned(),
        base_url: None,
        docs: None,
        auth: Auth::none(),
        middleware: Vec::new(),
        endpoints,
        facts: Vec::new(),
        rules_digest: None,
        origin: Origin::default(),
    }
}

impl Kit {
    /// The IR with one service.
    pub fn ir_with(self, service: Service) -> Ir {
        let mut ir = self.ir();
        ir.services.push(service);
        ir
    }
}

// ---- example values ------------------------------------------------------------------------

pub fn jv(v: serde_json::Value) -> ExampleValue {
    ExampleValue::Json(v)
}
pub fn obj(fields: Vec<(&str, ExampleValue)>) -> ExampleValue {
    ExampleValue::Object(
        fields
            .into_iter()
            .map(|(n, value)| ExampleField {
                name: n.to_owned(),
                wire: n.to_owned(),
                value,
            })
            .collect(),
    )
}
pub fn eref(given: &str, path: &[&str]) -> ExampleValue {
    ExampleValue::Ref(ExampleRef {
        given: given.to_owned(),
        path: path.iter().map(|p| (*p).to_owned()).collect(),
    })
}

pub fn example(name: &str, endpoint: &str, outcome: ExampleOutcome) -> Example {
    Example {
        name: name.to_owned(),
        package: PackageId(0),
        service: ServiceId(0),
        endpoint: endpoint.to_owned(),
        givens: Vec::new(),
        principal: None,
        now: None,
        args: Vec::new(),
        returns: None,
        outcome,
        docs: None,
        origin: Origin::default(),
    }
}

/// Chained setters for an [`Example`].
pub trait ExampleExt: Sized {
    fn given(self, fact: &str, value: Option<ExampleValue>) -> Self;
    fn arg(self, param: &str, value: ExampleValue) -> Self;
    fn returning(self, value: ExampleValue) -> Self;
    fn at(self, clock: &str) -> Self;
}

impl ExampleExt for Example {
    fn given(mut self, fact: &str, value: Option<ExampleValue>) -> Example {
        self.givens.push(ExampleGiven {
            fact: fact.to_owned(),
            value,
        });
        self
    }
    fn arg(mut self, param: &str, value: ExampleValue) -> Example {
        self.args.push(ExampleArg {
            param: param.to_owned(),
            value,
        });
        self
    }
    fn returning(mut self, value: ExampleValue) -> Example {
        self.returns = Some(value);
        self
    }
    fn at(mut self, clock: &str) -> Example {
        self.now = Some(clock.to_owned());
        self
    }
}

pub fn expect_ok(pattern: Option<ExampleValue>) -> ExampleOutcome {
    ExampleOutcome::Ok(pattern)
}
pub fn expect_error(error: &Type, pattern: Option<ExampleValue>) -> ExampleOutcome {
    ExampleOutcome::Error(ExampleErrorOutcome {
        decl: decl_id(error),
        pattern,
    })
}

//! Builders for hand-made IR: types, fields, declarations and typed expressions.

use veld_ir::{
    Always, BinExpr, Builtin, CallExpr, Constraints, Decl, DeclId, DeclKind, Enum, EnumLit,
    ErrorDecl, Expr, ExprKind, ExprLit, ExprType, Field, Ir, Literal, Model, Op, Origin, PackageId,
    PathRef, QuantExpr, Quantifier, Root, Rule, Scalar, Step, Type, Union, UnionArm, Variant,
};

// ---- types -------------------------------------------------------------------------------

pub fn sc(s: Scalar) -> Type {
    Type::Scalar(s)
}
pub fn int() -> Type {
    sc(Scalar::Int)
}
pub fn long() -> Type {
    sc(Scalar::Long)
}
pub fn float() -> Type {
    sc(Scalar::Float)
}
pub fn dec() -> Type {
    sc(Scalar::Decimal)
}
pub fn string() -> Type {
    sc(Scalar::String)
}
pub fn boolean() -> Type {
    sc(Scalar::Bool)
}
pub fn date() -> Type {
    sc(Scalar::Date)
}
pub fn datetime() -> Type {
    sc(Scalar::Datetime)
}
pub fn time() -> Type {
    sc(Scalar::Time)
}
pub fn uuid() -> Type {
    sc(Scalar::Uuid)
}
pub fn bytes() -> Type {
    sc(Scalar::Bytes)
}
pub fn list_of(t: Type) -> Type {
    Type::List(Box::new(t))
}
pub fn map_of(t: Type) -> Type {
    Type::Map(Box::new(t))
}
pub fn page_of(t: Type) -> Type {
    Type::Page(Box::new(t))
}

// ---- fields ------------------------------------------------------------------------------

pub fn field(name: &str, ty: Type) -> Field {
    Field {
        name: name.to_owned(),
        wire: name.to_owned(),
        ty,
        optional: false,
        default: None,
        readonly: false,
        version: false,
        derived: None,
        derived_id: None,
        pii: false,
        secret: false,
        visible: None,
        visible_principal: None,
        inherited_from: None,
        constraints: Constraints::default(),
        example: None,
        docs: None,
        deprecated: None,
        origin: Origin::default(),
    }
}
pub fn opt(mut f: Field) -> Field {
    f.optional = true;
    f
}
pub fn ro(mut f: Field) -> Field {
    f.readonly = true;
    f
}
pub fn dflt(mut f: Field, lit: Literal) -> Field {
    f.default = Some(lit);
    f
}
pub fn wired(mut f: Field, wire: &str) -> Field {
    f.wire = wire.to_owned();
    f
}

// ---- expressions -------------------------------------------------------------------------

pub const DUR: ExprType = ExprType::Duration;

pub fn et(t: &Type) -> ExprType {
    ExprType::Type(t.clone())
}
pub fn mk(t: ExprType, e: ExprKind) -> Expr {
    Expr { t, e }
}

fn lit(t: Type, l: ExprLit) -> Expr {
    mk(ExprType::Type(t), ExprKind::Lit(l))
}
pub fn i(n: i64) -> Expr {
    lit(int(), ExprLit::Int(n))
}
pub fn lng(n: i64) -> Expr {
    lit(long(), ExprLit::Long(n))
}
pub fn fl(x: f64) -> Expr {
    lit(float(), ExprLit::Float(x))
}
pub fn dc(text: &str) -> Expr {
    lit(dec(), ExprLit::Decimal(text.to_owned()))
}
pub fn s(text: &str) -> Expr {
    lit(string(), ExprLit::String(text.to_owned()))
}
pub fn b(v: bool) -> Expr {
    lit(boolean(), ExprLit::Bool(v))
}
pub fn variant(decl: &Type, name: &str, wire: &str) -> Expr {
    let Type::Decl(id) = decl else {
        panic!("an enum literal needs a declaration type");
    };
    let e = EnumLit {
        decl: *id,
        name: name.to_owned(),
        wire: wire.to_owned(),
    };
    lit(decl.clone(), ExprLit::Enum(e))
}

fn step(name: &str, optional: bool) -> Step {
    Step {
        name: name.to_owned(),
        wire: name.to_owned(),
        optional,
    }
}
pub fn path_ref(root: Root, steps: &[(&str, bool)]) -> PathRef {
    PathRef {
        root,
        steps: steps.iter().map(|(n, o)| step(n, *o)).collect(),
    }
}
/// `name` on `self`, of type `ty`.
pub fn p(name: &str, ty: Type) -> Expr {
    mk(
        et(&ty),
        ExprKind::Path(path_ref(Root::SelfValue, &[(name, false)])),
    )
}
/// An optional field of `self`.
pub fn po(name: &str, ty: Type) -> Expr {
    mk(
        et(&ty),
        ExprKind::Path(path_ref(Root::SelfValue, &[(name, true)])),
    )
}
/// A longer path from `self`; each step is `(name, optional)`.
pub fn pp(steps: &[(&str, bool)], ty: Type) -> Expr {
    mk(et(&ty), ExprKind::Path(path_ref(Root::SelfValue, steps)))
}
/// A lambda variable, optionally followed by field steps.
pub fn v(var: &str, steps: &[&str], ty: Type) -> Expr {
    let steps: Vec<(&str, bool)> = steps.iter().map(|n| (*n, false)).collect();
    mk(
        et(&ty),
        ExprKind::Path(path_ref(Root::Var(var.to_owned()), &steps)),
    )
}
pub fn root(root: Root, name: &str, ty: Type) -> Expr {
    mk(et(&ty), ExprKind::Path(path_ref(root, &[(name, false)])))
}
pub fn has(steps: &[(&str, bool)]) -> Expr {
    mk(
        et(&boolean()),
        ExprKind::Has(path_ref(Root::SelfValue, steps)),
    )
}

/// A binary node whose type follows the operator: `bool` for tests, the left type otherwise.
pub fn bin(op: Op, l: Expr, r: Expr) -> Expr {
    let t = match op {
        Op::Add | Op::Sub | Op::Mul | Op::Div | Op::Rem => l.t.clone(),
        Op::And
        | Op::Or
        | Op::Implies
        | Op::Eq
        | Op::Ne
        | Op::Lt
        | Op::Le
        | Op::Gt
        | Op::Ge
        | Op::In => et(&boolean()),
    };
    bin_t(op, l, r, t)
}
pub fn bin_t(op: Op, l: Expr, r: Expr, t: ExprType) -> Expr {
    mk(t, ExprKind::Bin(Box::new(BinExpr { op, l, r })))
}
pub fn and(l: Expr, r: Expr) -> Expr {
    bin(Op::And, l, r)
}
pub fn or(l: Expr, r: Expr) -> Expr {
    bin(Op::Or, l, r)
}
pub fn implies(l: Expr, r: Expr) -> Expr {
    bin(Op::Implies, l, r)
}
pub fn eq(l: Expr, r: Expr) -> Expr {
    bin(Op::Eq, l, r)
}
pub fn ne(l: Expr, r: Expr) -> Expr {
    bin(Op::Ne, l, r)
}
pub fn lt(l: Expr, r: Expr) -> Expr {
    bin(Op::Lt, l, r)
}
pub fn le(l: Expr, r: Expr) -> Expr {
    bin(Op::Le, l, r)
}
pub fn gt(l: Expr, r: Expr) -> Expr {
    bin(Op::Gt, l, r)
}
pub fn ge(l: Expr, r: Expr) -> Expr {
    bin(Op::Ge, l, r)
}
pub fn within(l: Expr, r: Expr) -> Expr {
    bin(Op::In, l, r)
}
pub fn add(l: Expr, r: Expr) -> Expr {
    bin(Op::Add, l, r)
}
pub fn sub(l: Expr, r: Expr) -> Expr {
    bin(Op::Sub, l, r)
}
pub fn mul(l: Expr, r: Expr) -> Expr {
    bin(Op::Mul, l, r)
}
pub fn div(l: Expr, r: Expr) -> Expr {
    bin(Op::Div, l, r)
}
pub fn rem(l: Expr, r: Expr) -> Expr {
    bin(Op::Rem, l, r)
}
pub fn not(x: Expr) -> Expr {
    mk(et(&boolean()), ExprKind::Not(Box::new(x)))
}
pub fn neg(x: Expr) -> Expr {
    mk(x.t.clone(), ExprKind::Neg(Box::new(x)))
}
pub fn widen(x: Expr, to: Type) -> Expr {
    mk(et(&to), ExprKind::Widen(Box::new(x)))
}
pub fn call(f: Builtin, args: Vec<Expr>, t: ExprType) -> Expr {
    mk(t, ExprKind::Call(CallExpr { f, args }))
}
pub fn len(x: Expr) -> Expr {
    call(Builtin::Len, vec![x], et(&int()))
}
pub fn dur(f: Builtin, n: Expr) -> Expr {
    call(f, vec![n], DUR)
}
pub fn text_fn(f: Builtin, a: Expr, b: Expr) -> Expr {
    call(f, vec![a, b], et(&boolean()))
}
pub fn quant(q: Quantifier, list: Expr, var: &str, body: Expr, t: ExprType) -> Expr {
    let node = QuantExpr {
        q,
        list,
        var: var.to_owned(),
        body,
    };
    mk(t, ExprKind::Quant(Box::new(node)))
}
pub fn list(items: Vec<Expr>, elem: &Type) -> Expr {
    mk(et(&list_of(elem.clone())), ExprKind::List(items))
}
pub fn now() -> Expr {
    mk(et(&datetime()), ExprKind::Now(Always))
}

// ---- declarations ------------------------------------------------------------------------

/// The source names of the fields of `self` that an expression reads, first use first.
pub fn reads_of(e: &Expr, out: &mut Vec<String>) {
    fn note(p: &PathRef, out: &mut Vec<String>) {
        if let (Root::SelfValue, Some(first)) = (&p.root, p.steps.first())
            && !out.contains(&first.name)
        {
            out.push(first.name.clone());
        }
    }
    match &e.e {
        ExprKind::Path(p) | ExprKind::Has(p) => note(p, out),
        ExprKind::Lit(_) | ExprKind::Now(_) => {}
        ExprKind::Not(x) | ExprKind::Neg(x) | ExprKind::Widen(x) | ExprKind::Old(x) => {
            reads_of(x, out);
        }
        ExprKind::Bin(b) => {
            reads_of(&b.l, out);
            reads_of(&b.r, out);
        }
        ExprKind::Call(c) => c.args.iter().for_each(|a| reads_of(a, out)),
        ExprKind::List(items) => items.iter().for_each(|a| reads_of(a, out)),
        ExprKind::Quant(q) => {
            reads_of(&q.list, out);
            reads_of(&q.body, out);
        }
    }
}

/// A rule under construction: its name and expression.
pub fn rule(name: &str, expr: Expr) -> (String, Expr) {
    (name.to_owned(), expr)
}

#[derive(Default)]
pub struct Kit {
    pub(super) decls: Vec<Decl>,
}

impl Kit {
    pub fn new() -> Kit {
        Kit::default()
    }

    fn push(&mut self, name: &str, kind: DeclKind) -> Type {
        let id = DeclId(self.decls.len() as u32);
        self.decls.push(Decl {
            id,
            package: PackageId(0),
            module: "t".to_owned(),
            name: name.to_owned(),
            qualified: format!("t.{name}"),
            sig: String::new(),
            docs: None,
            deprecated: None,
            origin: Origin::default(),
            kind,
        });
        Type::Decl(id)
    }

    pub fn enum_(&mut self, name: &str, variants: &[(&str, &str)]) -> Type {
        let variants = variants
            .iter()
            .map(|(n, w)| Variant {
                name: (*n).to_owned(),
                wire: (*w).to_owned(),
                docs: None,
                deprecated: None,
            })
            .collect();
        self.push(name, DeclKind::Enum(Enum::plain(variants)))
    }

    pub fn union(&mut self, name: &str, disc: &str, arms: &[(&str, &Type)]) -> Type {
        let arms = arms
            .iter()
            .filter_map(|(label, t)| match t {
                Type::Decl(id) => Some(UnionArm {
                    label: (*label).to_owned(),
                    decl: *id,
                }),
                Type::Scalar(_) | Type::List(_) | Type::Map(_) | Type::Page(_) => None,
            })
            .collect();
        let u = Union {
            discriminator: disc.to_owned(),
            arms,
        };
        self.push(name, DeclKind::Union(u))
    }

    fn model_of(
        &self,
        name: &str,
        parent: Option<&Type>,
        own: Vec<Field>,
        rules: Vec<(String, Expr)>,
    ) -> Model {
        let mut fields = Vec::new();
        let mut all_rules = Vec::new();
        let mut parent_id = None;
        if let Some(Type::Decl(pid)) = parent {
            parent_id = Some(*pid);
            if let Some(d) = self.decls.get(pid.index()) {
                fields = d
                    .fields()
                    .iter()
                    .cloned()
                    .map(|mut f| {
                        f.inherited_from.get_or_insert(*pid);
                        f
                    })
                    .collect();
                all_rules = d
                    .rules()
                    .iter()
                    .cloned()
                    .map(|mut r| {
                        r.inherited_from.get_or_insert(*pid);
                        r
                    })
                    .collect();
            }
        }
        fields.extend(own);
        for (rname, expr) in rules {
            let mut reads = Vec::new();
            reads_of(&expr, &mut reads);
            let reads_readonly = reads
                .iter()
                .any(|r| fields.iter().any(|f| &f.name == r && f.readonly));
            all_rules.push(Rule {
                flag: None,
                id: format!("veld:t/{name}.{rname}@0000000000000000"),
                name: rname,
                expr,
                message: None,
                reads,
                reads_readonly,
                inherited_from: None,
                docs: None,
                deprecated: None,
                rollout: None,
                origin: Origin::default(),
            });
        }
        Model {
            parent: parent_id,
            fields,
            rules: all_rules,
            retain: None,
        }
    }

    pub fn model(&mut self, name: &str, fields: Vec<Field>, rules: Vec<(String, Expr)>) -> Type {
        let m = self.model_of(name, None, fields, rules);
        self.push(name, DeclKind::Model(m))
    }

    pub fn child(
        &mut self,
        name: &str,
        parent: &Type,
        fields: Vec<Field>,
        rules: Vec<(String, Expr)>,
    ) -> Type {
        let m = self.model_of(name, Some(parent), fields, rules);
        self.push(name, DeclKind::Model(m))
    }

    pub fn error(&mut self, name: &str, fields: Vec<Field>, rules: Vec<(String, Expr)>) -> Type {
        let m = self.model_of(name, None, fields, rules);
        let e = ErrorDecl {
            default_status: Some(422),
            fields: m,
        };
        self.push(name, DeclKind::Error(e))
    }

    pub fn ir(self) -> Ir {
        Ir {
            ir_version: veld_ir::IR_VERSION.to_owned(),
            packages: Vec::new(),
            decls: self.decls,
            services: Vec::new(),
            consumes: std::collections::BTreeMap::new(),
            examples: Vec::new(),
            channels: Vec::new(),
            policies: Vec::new(),
        }
    }
}

/// A path step whose wire name differs from its source name.
pub fn pw(name: &str, wire: &str, ty: Type) -> Expr {
    let step = Step {
        name: name.to_owned(),
        wire: wire.to_owned(),
        optional: false,
    };
    let path = PathRef {
        root: Root::SelfValue,
        steps: vec![step],
    };
    mk(et(&ty), ExprKind::Path(path))
}

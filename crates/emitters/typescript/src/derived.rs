//! Derived fields (`spec/rules.md` section 17.2, `spec/targets/typescript.md` section 5i): for
//! every declaration with a derived field, one plain function that computes the derived fields of
//! one value in dependency order. The declaration's schema node carries it as `derive`, and the
//! runtime's walk (`deriveNode` in `runtime/veld.ts`) calls it after the values nested in the
//! value are computed; the companion's `compute(draft)` is the public face of the same walk.

use veld_codegen::CodeWriter;
use veld_ir::{Decl, Field, Scalar, Type};

use crate::expr::{print, wire_name};
use crate::tree::FileCtx;
use crate::types::ts_type;
use crate::util::{ts_str, type_name};

/// The name of the compute function of `d`.
#[must_use]
pub fn fn_name(d: &Decl) -> String {
    format!("compute{}Derived", type_name(d))
}

/// The code that turns the rule value `code` (the representation of `runtime/rules.ts`) of type
/// `ty` into the source form of a field: a decimal and a long as their text, a date, a time and a
/// datetime as their wire text. Everything else is the same in both.
fn to_source(fc: &mut FileCtx<'_, '_>, ty: &Type, code: &str) -> String {
    match ty {
        Type::Scalar(Scalar::Long) => format!("{code}.toString()"),
        Type::Scalar(Scalar::Decimal) => format!("{}({code})", fc.rt("decimalText")),
        Type::Scalar(Scalar::Date) => format!("{}({code})", fc.rt("dateText")),
        Type::Scalar(Scalar::Time) => format!("{}({code})", fc.rt("timeText")),
        Type::Scalar(Scalar::Datetime) => format!("{}({code})", fc.rt("datetimeText")),
        Type::List(inner) if converts(inner) => {
            let item = to_source(fc, inner, "x");
            format!("{code}.map((x) => {item})")
        }
        Type::Map(inner) if converts(inner) => {
            let item = to_source(fc, inner, "x");
            format!("Object.fromEntries(Object.entries({code}).map(([k, x]) => [k, {item}]))")
        }
        Type::Scalar(_) | Type::List(_) | Type::Map(_) | Type::Page(_) | Type::Decl(_) => {
            code.to_owned()
        }
    }
}

/// Whether a value of `ty` changes its representation between the rules and the source form.
fn converts(ty: &Type) -> bool {
    match ty {
        Type::Scalar(s) => matches!(
            s,
            Scalar::Long | Scalar::Decimal | Scalar::Date | Scalar::Time | Scalar::Datetime
        ),
        Type::List(t) | Type::Map(t) => converts(t),
        Type::Page(_) | Type::Decl(_) => false,
    }
}

fn write_field(w: &mut CodeWriter, fc: &mut FileCtx<'_, '_>, d: &Decl, f: &Field) {
    let Some(expr) = &f.derived else {
        return;
    };
    let ns = fc.rules_ns();
    let printed = print(fc.tree.ir, &ns, expr);
    let value = to_source(fc, &f.ty, &printed);
    let wires: Vec<String> = f
        .derived_reads()
        .iter()
        .map(|n| ts_str(&wire_name(d.fields(), n)))
        .collect();
    let id = ts_str(f.derived_id.as_deref().unwrap_or_default());
    let target = crate::expr::member("self", &f.name);
    w.open("try");
    w.line(format!("{target} = {value};"));
    w.close_with("} catch (e) {");
    w.indent();
    w.open(format!("if (e instanceof {ns}.RuleEvalError)"));
    w.line(format!(
        "out.push({{ rule: {id}, path, fields: [{}], message: {}, error: e.kind }});",
        wires.join(", "),
        ts_str(&format!("field {} could not be computed", f.name))
    ));
    w.close_with(format!("}} else if (!(e instanceof {ns}.AbsentField)) {{"));
    w.indent();
    w.line("throw e;");
    w.close();
    w.close();
}

/// Writes the compute function of `d`. `self_ty` is the TypeScript type of the value.
pub fn write_compute_fn(w: &mut CodeWriter, fc: &mut FileCtx<'_, '_>, d: &Decl, self_ty: &str) {
    let violation = fc.rt("Violation");
    let partial = fc.rt("DeepPartial");
    w.line(
        "// Computes the derived fields of one value, in dependency order (spec/rules.md 17.2).",
    );
    w.open(format!(
        "function {}(self: {partial}<{self_ty}>, path: string, out: {violation}[]): void",
        fn_name(d)
    ));
    for f in d.derived_fields() {
        write_field(w, fc, d, f);
    }
    w.close();
}

/// The three positions of a value that has derived fields (`spec/rules.md` section 17.2): the
/// handler receives it as the interface says (the field is there), returns it as `Output` (the
/// field is optional, whatever it holds is replaced) and a client sends it as `Input` (no derived
/// field at any depth).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    Output,
    Input,
}

impl Shape {
    fn suffix(self) -> &'static str {
        match self {
            Shape::Output => "Output",
            Shape::Input => "Input",
        }
    }
}

/// The name of the type of `d` in `shape`: `OrderOutput`, `OrderInput`, `RoomFullDataOutput`.
#[must_use]
pub fn shape_name(d: &Decl, shape: Shape) -> String {
    let base = type_name(d);
    if d.is_error() {
        format!("{base}Data{}", shape.suffix())
    } else {
        format!("{base}{}", shape.suffix())
    }
}

/// Whether a declaration gets types of the other positions: it has a derived field, or a field
/// whose type reaches one.
#[must_use]
pub fn has_shapes(fc: &FileCtx<'_, '_>, d: &Decl) -> bool {
    fc.tree.ir.type_reaches_derived(&Type::Decl(d.id))
}

/// The TypeScript type of `ty` in `shape`: the types of the declarations that reach a derived
/// field are replaced by their `Output` or `Input` type.
pub fn shaped_type(fc: &mut FileCtx<'_, '_>, ty: &Type, shape: Shape) -> String {
    if !fc.tree.ir.type_reaches_derived(ty) {
        return ts_type(fc, ty);
    }
    match ty {
        Type::Scalar(_) => ts_type(fc, ty),
        Type::List(t) => format!("{}[]", shaped_type(fc, t, shape)),
        Type::Map(t) => format!("Record<string, {}>", shaped_type(fc, t, shape)),
        Type::Page(t) => {
            let page = fc.rt("Page");
            format!("{page}<{}>", shaped_type(fc, t, shape))
        }
        Type::Decl(id) => {
            let d = fc.tree.ir.decl(*id);
            let name = shape_name(d, shape);
            let module = fc.tree.decl_module(d);
            fc.import(&module, &name, false);
            name
        }
    }
}

/// The members of `fields` that change in `shape`: a derived field (optional in `Output`, left
/// out of `Input`) and a field whose type reaches one.
fn changed_members(
    fc: &mut FileCtx<'_, '_>,
    fields: &[Field],
    shape: Shape,
) -> (Vec<String>, Vec<String>) {
    let (mut omit, mut members) = (Vec::new(), Vec::new());
    for f in fields {
        let derived = f.derived.is_some();
        if !derived && !fc.tree.ir.type_reaches_derived(&f.ty) {
            continue;
        }
        omit.push(ts_str(&f.name));
        let key = crate::util::prop_key(&f.name);
        match (derived, shape) {
            (true, Shape::Input) => {}
            (true, Shape::Output) => {
                let ty = ts_type(fc, &f.ty);
                members.push(format!("{key}?: {ty}"));
            }
            (false, _) => {
                let opt = if f.optional { "?" } else { "" };
                let ty = shaped_type(fc, &f.ty, shape);
                members.push(format!("{key}{opt}: {ty}"));
            }
        }
    }
    (omit, members)
}

fn write_alias(
    w: &mut CodeWriter,
    alias: &str,
    base: &str,
    (omit, members): (Vec<String>, Vec<String>),
) {
    let rest = if members.is_empty() {
        String::new()
    } else {
        format!(" & {{ {} }}", members.join("; "))
    };
    w.line(format!(
        "export type {alias} = Omit<{base}, {}>{rest};",
        omit.join(" | ")
    ));
}

/// Writes `<Name>Output` and `<Name>Input` of a model or an event whose values have derived
/// fields, after its interface.
pub fn write_model_shapes(w: &mut CodeWriter, fc: &mut FileCtx<'_, '_>, d: &Decl) {
    if !has_shapes(fc, d) {
        return;
    }
    let base = type_name(d);
    w.blank();
    w.line("/** The value as the application hands it over: a derived field is optional, and what it holds is replaced. */");
    let output = changed_members(fc, d.fields(), Shape::Output);
    write_alias(w, &shape_name(d, Shape::Output), &base, output);
    w.line("/** The value as a client sends it: no derived field, at any depth. */");
    let input = changed_members(fc, d.fields(), Shape::Input);
    write_alias(w, &shape_name(d, Shape::Input), &base, input);
}

/// Writes `<Name>DataOutput` of an error whose data has derived fields.
pub fn write_error_shape(w: &mut CodeWriter, fc: &mut FileCtx<'_, '_>, d: &Decl, data: &str) {
    if !has_shapes(fc, d) {
        return;
    }
    w.line("/** The data as the application hands it over: a derived field is optional, and what it holds is replaced. */");
    let output = changed_members(fc, d.fields(), Shape::Output);
    write_alias(w, &shape_name(d, Shape::Output), data, output);
}

/// Writes `<Name>Output` and `<Name>Input` of a union that has an arm with derived fields.
pub fn write_union_shapes(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    d: &Decl,
    u: &veld_ir::Union,
) {
    if !has_shapes(fc, d) {
        return;
    }
    w.blank();
    for shape in [Shape::Output, Shape::Input] {
        let disc = crate::util::prop_key(&u.discriminator);
        let arms: Vec<String> = u
            .arms
            .iter()
            .map(|a| {
                let ty = shaped_type(fc, &Type::Decl(a.decl), shape);
                format!("({ty} & {{ {disc}: {} }})", ts_str(&a.label))
            })
            .collect();
        w.line(format!(
            "export type {} = {};",
            shape_name(d, shape),
            arms.join(" | ")
        ));
    }
}

//! From the values a solver gave to concrete JSON inputs.
//!
//! The solver models only the parts of a value that a rule reads. Strings and uuids come back as
//! opaque tokens: two tokens are equal exactly when the values are, so a token is turned into a
//! text of the length the solver gave, different for every token. Whatever is wrong with the
//! result, the reference evaluator finds out in the confirmation step.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value as Json};
use veld_ir::{Ir, Scalar, Type};
use veld_rules::{format_date, format_datetime, format_time};

use crate::problem::{Problem, ValueKey};
use crate::sample::{self, MAX_ITEMS};
use crate::sexp::Sexp;
use crate::types::{RootKey, RootSpec, Sort, VarInfo, VarKind};

/// The inputs a model stands for.
#[derive(Debug, Clone, Default)]
pub struct Concrete {
    /// The roots that are present.
    pub roots: BTreeMap<RootKey, Json>,
    /// The reading of the clock, in milliseconds since the epoch.
    pub now: i64,
}

/// The values of a model by key.
#[derive(Debug)]
pub struct Model<'m> {
    by_key: BTreeMap<String, &'m Sexp>,
}

impl<'m> Model<'m> {
    /// Pairs the keys of the problem with the values the solver gave, in order.
    ///
    /// # Errors
    /// The solver gave a different number of values than were asked for.
    pub fn new(problem: &Problem<'_>, values: &'m [Sexp]) -> Result<Model<'m>, String> {
        let keys = problem.value_keys();
        if keys.len() != values.len() {
            return Err("the solver gave a different number of values than were asked for".into());
        }
        Ok(Model {
            by_key: keys.iter().map(ValueKey::id).zip(values.iter()).collect(),
        })
    }

    fn get(&self, key: &str) -> Option<&'m Sexp> {
        self.by_key.get(key).copied()
    }
}

/// The text of each token of a string or uuid sort.
struct Texts {
    strs: BTreeMap<String, String>,
    uuids: BTreeMap<String, String>,
}

fn token(sexp: &Sexp) -> Result<String, String> {
    sexp.atom()
        .map(str::to_owned)
        .ok_or_else(|| "a value of an uninterpreted sort that is not a token".to_owned())
}

pub(crate) fn gen_str(k: usize, len: usize) -> Option<String> {
    if len == 0 {
        return (k == 0).then(String::new);
    }
    let mut digits = Vec::new();
    let mut n = k;
    loop {
        digits.push(char::from(b'a' + u8::try_from(n % 26).unwrap_or(0)));
        n /= 26;
        if n == 0 {
            break;
        }
    }
    (digits.len() <= len).then(|| {
        let tail: String = digits.iter().rev().collect();
        format!("{}{tail}", "a".repeat(len - digits.len()))
    })
}

fn fresh_str(len: usize, used: &BTreeSet<String>) -> Option<String> {
    (0..)
        .map_while(|k| gen_str(k, len))
        .find(|s| !used.contains(s))
}

fn assign_texts(problem: &Problem<'_>, model: &Model<'_>) -> Result<Texts, String> {
    let mut texts = Texts {
        strs: BTreeMap::new(),
        uuids: BTreeMap::new(),
    };
    let mut used = BTreeSet::new();
    for l in problem.literals() {
        let t = token(model.get(&l.name).ok_or("a constant has no value")?)?;
        let map = if l.sort == Sort::Uuid {
            &mut texts.uuids
        } else {
            &mut texts.strs
        };
        map.insert(t, l.text.clone());
        used.insert(l.text.clone());
    }
    let mut uuid_n = 0u64;
    for v in problem.vars().filter(|v| v.kind == VarKind::Value) {
        let Some(t) = model.get(&v.name) else {
            continue;
        };
        match v.sort {
            Sort::Str => {
                if problem.restricted.contains(&v.name) {
                    return Err("a string with a pattern or a format".to_owned());
                }
                let t = token(t)?;
                let len = model
                    .get(&format!("slen:{}", v.name))
                    .and_then(Sexp::as_int)
                    .and_then(|n| usize::try_from(n).ok())
                    .ok_or("a string has no length")?;
                if let std::collections::btree_map::Entry::Vacant(slot) = texts.strs.entry(t) {
                    let s = fresh_str(len, &used).ok_or("no distinct string of that length")?;
                    used.insert(s.clone());
                    slot.insert(s);
                }
            }
            Sort::Uuid => {
                let t = token(t)?;
                texts.uuids.entry(t).or_insert_with(|| {
                    uuid_n += 1;
                    format!("00000000-0000-4000-8000-{uuid_n:012x}")
                });
            }
            Sort::Bool | Sort::Int | Sort::Real => {}
        }
    }
    Ok(texts)
}

/// Exact decimal text of `num / den`, when it terminates.
pub(crate) fn decimal_text(num: i128, den: i128) -> Option<String> {
    let mut scale = 0u32;
    let mut pow = 1i128;
    while pow % den != 0 {
        pow = pow.checked_mul(10)?;
        scale += 1;
        if scale > 30 {
            return None;
        }
    }
    let digits = num.checked_mul(pow / den)?;
    let (neg, abs) = (digits < 0, digits.unsigned_abs());
    let body = if scale == 0 {
        abs.to_string()
    } else {
        let s = format!("{abs:0>width$}", width = scale as usize + 1);
        let (int, frac) = s.split_at(s.len() - scale as usize);
        format!("{int}.{frac}")
    };
    Some(if neg { format!("-{body}") } else { body })
}

fn int_json(var: &VarInfo, n: i128) -> Result<Json, String> {
    let out_of_range = || format!("{n} is out of range");
    let as64 = i64::try_from(n).map_err(|_| out_of_range());
    if !var.variants.is_empty() {
        let idx = usize::try_from(n).map_err(|_| out_of_range())?;
        return var
            .variants
            .get(idx)
            .map(|w| Json::String(w.clone()))
            .ok_or_else(out_of_range);
    }
    Ok(match &var.ty {
        Some(Type::Scalar(Scalar::Int)) => {
            Json::from(i32::try_from(n).map_err(|_| out_of_range())?)
        }
        Some(Type::Scalar(Scalar::Long)) => Json::String(as64?.to_string()),
        Some(Type::Scalar(Scalar::Date)) => Json::String(format_date(as64?)),
        Some(Type::Scalar(Scalar::Datetime)) => Json::String(format_datetime(as64?)),
        Some(Type::Scalar(Scalar::Time)) => Json::String(format_time(as64?)),
        _ => return Err("an integer for a part that is not one".to_owned()),
    })
}

fn real_json(var: &VarInfo, sexp: &Sexp) -> Result<Json, String> {
    let (num, den) = sexp.as_rational().ok_or("a real that cannot be read")?;
    match &var.ty {
        Some(Type::Scalar(Scalar::Decimal)) => decimal_text(num, den)
            .map(Json::String)
            .ok_or_else(|| "a decimal that does not terminate".to_owned()),
        Some(Type::Scalar(Scalar::Float)) => serde_json::Number::from_f64(num as f64 / den as f64)
            .map(Json::Number)
            .ok_or_else(|| "a float that is not finite".to_owned()),
        _ => Err("a real for a part that is not one".to_owned()),
    }
}

fn value_json(var: &VarInfo, sexp: &Sexp, texts: &Texts) -> Result<Json, String> {
    match var.sort {
        Sort::Bool => sexp
            .as_bool()
            .map(Json::Bool)
            .ok_or_else(|| "a Boolean".to_owned()),
        Sort::Int => int_json(var, sexp.as_int().ok_or("an integer that cannot be read")?),
        Sort::Real => real_json(var, sexp),
        Sort::Str => texts
            .strs
            .get(&token(sexp)?)
            .map(|s| Json::String(s.clone()))
            .ok_or_else(|| "a string".to_owned()),
        Sort::Uuid => texts
            .uuids
            .get(&token(sexp)?)
            .map(|s| Json::String(s.clone()))
            .ok_or_else(|| "a uuid".to_owned()),
    }
}

/// The type of the part at `path` below a value of type `ty`.
fn type_at(ir: &Ir, ty: &Type, path: &[String]) -> Option<Type> {
    let mut cur = ty.clone();
    for w in path {
        cur = match &cur {
            Type::Decl(id) => ir
                .decl(*id)
                .fields()
                .iter()
                .find(|f| f.wire == *w)?
                .ty
                .clone(),
            Type::Page(t) => match w.as_str() {
                "items" => Type::List(t.clone()),
                "total" => Type::Scalar(Scalar::Long),
                "page" | "size" => Type::Scalar(Scalar::Int),
                _ => return None,
            },
            Type::Scalar(_) | Type::List(_) | Type::Map(_) => return None,
        };
    }
    Some(cur)
}

fn slot<'j>(root: &'j mut Json, path: &[String]) -> Option<&'j mut Json> {
    path.iter().try_fold(root, |cur, w| cur.get_mut(w))
}

fn put(root: &mut Json, path: &[String], v: Json) -> Result<(), String> {
    let Some((last, parents)) = path.split_last() else {
        *root = v;
        return Ok(());
    };
    for k in 0..parents.len() {
        if slot(root, &parents[..=k]).is_none() {
            let parent = slot(root, &parents[..k]).ok_or("a path with no parent")?;
            if let Some(obj) = parent.as_object_mut() {
                obj.insert(parents[k].clone(), Json::Object(Map::new()));
            }
        }
    }
    let parent = slot(root, parents).ok_or("a path with no parent")?;
    parent
        .as_object_mut()
        .ok_or("a part below something that is not an object")?
        .insert(last.clone(), v);
    Ok(())
}

fn remove(root: &mut Json, path: &[String]) {
    if let Some((last, parents)) = path.split_last()
        && let Some(obj) = slot(root, parents).and_then(Json::as_object_mut)
    {
        obj.remove(last);
    }
}

fn len_json(ir: &Ir, var: &VarInfo, n: i128) -> Result<Json, String> {
    let n = usize::try_from(n).map_err(|_| "a negative length")?;
    if n > MAX_ITEMS {
        return Err("a longer list than the check builds".to_owned());
    }
    let none = veld_ir::Constraints::default();
    Ok(match &var.ty {
        Some(Type::List(t)) => Json::Array(vec![sample::sample(ir, t, &none, 1)?; n]),
        Some(Type::Map(t)) => {
            let item = sample::sample(ir, t, &none, 1)?;
            Json::Object((0..n).map(|i| (format!("k{i}"), item.clone())).collect())
        }
        Some(Type::Scalar(Scalar::Bytes)) => Json::String(sample::zero_bytes(n)),
        _ => return Err("a length of something that is not a list, a map or bytes".to_owned()),
    })
}

fn order(v: &VarInfo) -> (usize, u8) {
    let k = match v.kind {
        VarKind::Has => 0,
        VarKind::Value => 1,
        VarKind::Len => 2,
    };
    (v.path.len(), k)
}

/// The value of one root: a plain value of its type with the parts the solver decided put in.
fn build_root(
    problem: &Problem<'_>,
    key: &RootKey,
    spec: &RootSpec,
    model: &Model<'_>,
    texts: &Texts,
) -> Result<Option<Json>, String> {
    let ir = problem.ir;
    let mut json = sample::sample(ir, &spec.ty, &spec.constraints, 0)?;
    let mut vars: Vec<&VarInfo> = problem.vars().filter(|v| &v.root == key).collect();
    vars.sort_by_key(|v| order(v));
    for v in vars {
        let Some(sexp) = model.get(&v.name) else {
            continue;
        };
        match v.kind {
            VarKind::Has => {
                let present = sexp.as_bool().ok_or("a Boolean that cannot be read")?;
                if v.path.is_empty() {
                    if !present {
                        return Ok(None);
                    }
                } else if !present {
                    remove(&mut json, &v.path);
                } else if slot(&mut json, &v.path).is_none() {
                    let ty = type_at(ir, &spec.ty, &v.path).ok_or("a part of unknown type")?;
                    let filler = sample::sample(ir, &ty, &veld_ir::Constraints::default(), 1)?;
                    put(&mut json, &v.path, filler)?;
                }
            }
            VarKind::Value => put(&mut json, &v.path, value_json(v, sexp, texts)?)?,
            VarKind::Len => {
                let n = sexp.as_int().ok_or("a length that cannot be read")?;
                put(&mut json, &v.path, len_json(ir, v, n)?)?;
            }
        }
    }
    Ok(Some(json))
}

/// Turns the values of a model into the inputs of the question.
///
/// # Errors
/// A text that says why the model could not be turned into a value.
pub fn concretize(problem: &Problem<'_>, values: &[Sexp]) -> Result<Concrete, String> {
    let model = Model::new(problem, values)?;
    let texts = assign_texts(problem, &model)?;
    let mut out = Concrete::default();
    for (key, spec) in &problem.roots {
        if let Some(json) = build_root(problem, key, spec, &model, &texts)? {
            out.roots.insert(key.clone(), json);
        }
    }
    if let Some(n) = model.get("now").and_then(Sexp::as_int) {
        out.now = i64::try_from(n).map_err(|_| "a clock out of range".to_owned())?;
    }
    Ok(out)
}

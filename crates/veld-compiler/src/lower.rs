//! Lowering of an analysed [`Program`] to the Veld IR.
//!
//! Decls are sorted by qualified name and `DeclId` is the index in that order.
//! Services are sorted by (package, name). Prelude declarations are included
//! only when referenced, and package `veld` is listed only when it is used.

use std::collections::{BTreeMap, BTreeSet};

use crate::diagnostics::{FileId, SourceMap, Span};
use crate::semantic::Program;
use crate::semantic::{DKind, DeclInfo, REndpoint, RField, ServiceInfo};
use crate::signature;
use crate::syntax::ast::doc_text;
use veld_ir::{
    Decl, DeclId, DeclKind, Endpoint, Enum, ErrorDecl, ErrorResponse, Field, Ir, Model, Origin,
    Package, PackageId, Param, Service, ServiceId, Success, Type, Union, UnionArm, Variant,
};

struct Lowerer<'p, 'a> {
    prog: &'p Program<'a>,
    sources: &'p SourceMap,
    decl_map: Vec<Option<DeclId>>,
    pkg_map: Vec<Option<PackageId>>,
    file_rel: BTreeMap<FileId, &'p str>,
}

/// Builds the IR. Call only when analysis reported no errors.
#[must_use]
pub fn lower(
    prog: &Program<'_>,
    sources: &SourceMap,
    consumes: &BTreeMap<String, Vec<String>>,
) -> Ir {
    let included = included_decls(prog);
    let mut lw = Lowerer {
        prog,
        sources,
        decl_map: vec![None; prog.decls.len()],
        pkg_map: vec![None; prog.packages.len()],
        file_rel: prog.modules.iter().map(|m| (m.file, m.rel_path)).collect(),
    };
    for (new, &old) in included.iter().enumerate() {
        lw.decl_map[old] = Some(DeclId(u32::try_from(new).unwrap_or(u32::MAX)));
    }
    let packages = lw.packages(&included);
    let mut decls: Vec<Decl> = included.iter().map(|&i| lw.decl(i)).collect();
    let sigs = signature::decl_sigs(&decls, &packages);
    for (d, s) in decls.iter_mut().zip(sigs) {
        d.sig = s;
    }
    let services = lw.services(&decls);
    Ir {
        ir_version: veld_ir::IR_VERSION.to_string(),
        packages,
        decls,
        services,
        consumes: lw.consumes(consumes),
    }
}

/// Indices of declarations to emit, sorted by qualified name.
fn included_decls(prog: &Program<'_>) -> Vec<usize> {
    let mut set: BTreeSet<usize> = BTreeSet::new();
    let mut stack: Vec<usize> = Vec::new();
    for (i, d) in prog.decls.iter().enumerate() {
        if d.pkg != prog.prelude_pkg {
            set.insert(i);
            stack.extend(decl_refs(d));
        }
    }
    for s in &prog.services {
        for e in &s.endpoints {
            stack.extend(endpoint_refs(e));
        }
    }
    while let Some(i) = stack.pop() {
        if prog.is_prelude(i) && set.insert(i) {
            stack.extend(decl_refs(&prog.decls[i]));
        }
    }
    let mut v: Vec<usize> = set.into_iter().collect();
    v.sort_by(|a, b| prog.decls[*a].qualified.cmp(&prog.decls[*b].qualified));
    v
}

fn type_refs(t: &Type) -> Vec<usize> {
    let mut ids = Vec::new();
    t.collect_decls(&mut ids);
    ids.into_iter().map(DeclId::index).collect()
}

fn decl_refs(d: &DeclInfo<'_>) -> Vec<usize> {
    let mut out: Vec<usize> = d.res.parent.into_iter().collect();
    out.extend(d.res.fields.iter().flat_map(|f| type_refs(&f.ty)));
    out.extend(d.res.arms.iter().map(|a| a.decl));
    out.extend(d.res.const_ty.iter().flat_map(type_refs));
    out
}

fn endpoint_refs(e: &REndpoint) -> Vec<usize> {
    let mut out: Vec<usize> = e.params.iter().flat_map(|p| type_refs(&p.ty)).collect();
    out.extend(e.success_ty.iter().flat_map(type_refs));
    out.extend(e.errors.iter().map(|(_, d)| *d));
    out
}

impl Lowerer<'_, '_> {
    fn remap(&self, t: &Type) -> Type {
        match t {
            Type::Scalar(s) => Type::Scalar(*s),
            Type::List(i) => Type::List(Box::new(self.remap(i))),
            Type::Map(i) => Type::Map(Box::new(self.remap(i))),
            Type::Page(i) => Type::Page(Box::new(self.remap(i))),
            Type::Decl(d) => Type::Decl(self.decl_id(d.index())),
        }
    }

    fn decl_id(&self, old: usize) -> DeclId {
        self.decl_map
            .get(old)
            .copied()
            .flatten()
            .unwrap_or(DeclId(u32::MAX))
    }

    fn origin(&self, span: Span, module: usize) -> Origin {
        let (line, col) = self
            .sources
            .line_col(span.file, span.start)
            .unwrap_or((1, 1));
        let file = self
            .file_rel
            .get(&span.file)
            .copied()
            .unwrap_or(self.prog.modules[module].rel_path);
        Origin {
            file: file.to_string(),
            line,
            col,
        }
    }

    fn packages(&mut self, included: &[usize]) -> Vec<Package> {
        let used: BTreeSet<usize> = included.iter().map(|&i| self.prog.decls[i].pkg).collect();
        let mut keep: Vec<usize> = (0..self.prog.packages.len())
            .filter(|&p| p != self.prog.prelude_pkg || used.contains(&p))
            .collect();
        keep.sort_by(|a, b| {
            self.prog.packages[*a]
                .name
                .cmp(&self.prog.packages[*b].name)
        });
        keep.iter()
            .enumerate()
            .map(|(new, &old)| {
                let id = PackageId(u32::try_from(new).unwrap_or(u32::MAX));
                self.pkg_map[old] = Some(id);
                Package {
                    id,
                    name: self.prog.packages[old].name.clone(),
                    kind: self.prog.packages[old].kind,
                }
            })
            .collect()
    }

    fn pkg_id(&self, old: usize) -> PackageId {
        self.pkg_map
            .get(old)
            .copied()
            .flatten()
            .unwrap_or(PackageId(u32::MAX))
    }

    fn field(&self, f: &RField, module: usize) -> Field {
        Field {
            name: f.name.clone(),
            wire: f.wire.clone(),
            ty: self.remap(&f.ty),
            optional: f.optional,
            default: f.default.clone(),
            readonly: f.readonly,
            inherited_from: f.inherited_from.map(|d| self.decl_id(d)),
            constraints: f.constraints.clone(),
            example: f.example.clone(),
            docs: f.docs.clone(),
            deprecated: f.deprecated.clone(),
            origin: self.origin(f.span, module),
        }
    }

    fn decl(&self, old: usize) -> Decl {
        let d = &self.prog.decls[old];
        let fields = || {
            d.res
                .fields
                .iter()
                .map(|f| self.field(f, d.module))
                .collect::<Vec<_>>()
        };
        let kind = match d.kind {
            DKind::Model => DeclKind::Model(Model {
                parent: d.res.parent.map(|p| self.decl_id(p)),
                fields: fields(),
                rules: Vec::new(),
            }),
            DKind::Error => DeclKind::Error(ErrorDecl {
                default_status: d.res.default_status,
                fields: Model {
                    parent: None,
                    fields: fields(),
                    rules: Vec::new(),
                },
            }),
            DKind::Enum => DeclKind::Enum(Enum {
                variants: d
                    .res
                    .variants
                    .iter()
                    .map(|v| Variant {
                        name: v.name.clone(),
                        wire: v.wire.clone(),
                        docs: v.docs.clone(),
                        deprecated: v.deprecated.clone(),
                    })
                    .collect(),
            }),
            DKind::Union => DeclKind::Union(Union {
                discriminator: d.res.discriminator.clone(),
                arms: d
                    .res
                    .arms
                    .iter()
                    .map(|a| UnionArm {
                        label: a.label.clone(),
                        decl: self.decl_id(a.decl),
                    })
                    .collect(),
            }),
            DKind::Const => DeclKind::Const(veld_ir::Const {
                ty: d
                    .res
                    .const_ty
                    .as_ref()
                    .map_or(Type::Scalar(veld_ir::Scalar::String), |t| self.remap(t)),
                value: d.res.const_value.clone().unwrap_or(veld_ir::Literal::Null),
            }),
        };
        Decl {
            id: self.decl_id(old),
            package: self.pkg_id(d.pkg),
            module: self.prog.modules[d.module].path.clone(),
            name: d.name.clone(),
            qualified: d.qualified.clone(),
            sig: String::new(),
            docs: doc_text(&d.ast.docs),
            deprecated: d.res.deprecated.clone(),
            origin: self.origin(d.name_span, d.module),
            kind,
        }
    }

    fn services(&self, decls: &[Decl]) -> Vec<Service> {
        let mut order: Vec<&ServiceInfo> = self.prog.services.iter().collect();
        order.sort_by(|a, b| {
            (&self.prog.packages[a.pkg].name, &a.name)
                .cmp(&(&self.prog.packages[b.pkg].name, &b.name))
        });
        order
            .into_iter()
            .enumerate()
            .map(|(i, s)| self.service(i, s, decls))
            .collect()
    }

    fn service(&self, index: usize, s: &ServiceInfo, decls: &[Decl]) -> Service {
        let pkg = &self.prog.packages[s.pkg];
        Service {
            id: ServiceId(u32::try_from(index).unwrap_or(u32::MAX)),
            package: self.pkg_id(s.pkg),
            name: s.name.clone(),
            prefix: s.prefix.clone(),
            base_url: pkg.base_url.clone(),
            docs: s.docs.clone(),
            auth: s.auth.clone(),
            middleware: s.middleware.clone(),
            endpoints: s
                .endpoints
                .iter()
                .map(|e| self.endpoint(e, s, &pkg.name, decls))
                .collect(),
            origin: self.origin(s.span, s.module),
        }
    }

    fn endpoint(&self, e: &REndpoint, s: &ServiceInfo, pkg: &str, decls: &[Decl]) -> Endpoint {
        let mut errors: Vec<ErrorResponse> = e
            .errors
            .iter()
            .map(|(status, d)| ErrorResponse {
                status: *status,
                decl: self.decl_id(*d),
            })
            .collect();
        errors.sort_by(|a, b| {
            (a.status, &decls[a.decl.index()].sig).cmp(&(b.status, &decls[b.decl.index()].sig))
        });
        Endpoint {
            name: e.name.clone(),
            method: e.method,
            path: e.path.clone(),
            sig: signature::op_sig(pkg, &s.name, &e.name, e.method, &e.path),
            docs: e.docs.clone(),
            deprecated: e.deprecated.clone(),
            auth: e.auth.clone(),
            middleware: e.middleware.clone(),
            params: e
                .params
                .iter()
                .map(|p| Param {
                    name: p.name.clone(),
                    binding: p.binding,
                    header: p.header.clone(),
                    ty: self.remap(&p.ty),
                    optional: p.optional,
                    stream: p.stream,
                    constraints: p.constraints.clone(),
                })
                .collect(),
            success: Success {
                status: e.success_status,
                ty: e.success_ty.as_ref().map(|t| self.remap(t)),
                stream: e.success_stream,
            },
            errors,
            origin: self.origin(e.span, s.module),
        }
    }

    fn consumes(&self, input: &BTreeMap<String, Vec<String>>) -> BTreeMap<String, Vec<String>> {
        let known = |n: &str| {
            self.prog
                .packages
                .iter()
                .any(|p| p.name == n && p.kind != veld_ir::PackageKind::Prelude)
        };
        input
            .iter()
            .filter(|(k, _)| known(k))
            .map(|(k, v)| {
                let set: BTreeSet<String> =
                    v.iter().filter(|t| *t != k && known(t)).cloned().collect();
                (k.clone(), set.into_iter().collect())
            })
            .collect()
    }
}

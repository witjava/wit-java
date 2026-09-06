//! The mapper: WIT resolve → Java declaration IR, implementing the mapping
//! spec. All collisions funnel through a single FQN registry (spec §5.10).

use crate::config::{GenerateOptions, InterfaceStyle, OptionStyle, Role, U64Style};
use crate::diagnostic::{Code, Diagnostic, Diagnostics, Location};
use crate::ir;
use crate::naming::{self, Fqn};
use crate::wit;
use std::collections::HashMap;
use wit_parser::{
    Docs, Flags, Function, FunctionKind, Handle, PackageId, Record, Resolve, Result_, Tuple, Type,
    TypeDef, TypeDefKind, TypeId, Variant, World, WorldItem, WorldKey,
};

const U64_NOTE: &str = "Unsigned 64-bit integer, range 0..2^64-1, stored as a Java {@code long}, which is signed. Use {@link java.lang.Long#compareUnsigned}, {@link java.lang.Long#divideUnsigned} and {@link java.lang.Long#toUnsignedString} for unsigned operations.";
const CHAR_NOTE: &str = "A Unicode scalar value (U+0000..U+10FFFF, excluding the surrogate range U+D800..U+DFFF), stored as a Java {@code int}.";
const BORROW_NOTE: &str = "Borrowed handle: ownership is not transferred by this call.";
const OWN_RETURN_NOTE: &str = "Owned handle: the caller is responsible for closing (dropping) it.";
const OWN_PARAM_NOTE: &str =
    "A parameter contains owned handles; closing them is the caller's decision.";
// Spec §5.11 also defines a nested-owned *return* note; the current mapper
// emits OWN_RETURN_NOTE for any owned return (nested or direct).

/// Position of a type occurrence: return positions keep `Optional` even
/// under the nullable style (spec §5.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pos {
    Return,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RegisteredKind {
    Resource,
    Record,
    Variant,
    Enum,
    Flags,
    Interface,
}

struct RegistryEntry {
    #[allow(dead_code)]
    kind: RegisteredKind,
    /// WIT package identity, for WJ0004-vs-WJ0007 classification.
    wit_pkg: String,
    desc: String,
}

pub fn generate(
    resolve: &Resolve,
    opts: &GenerateOptions,
) -> Result<crate::ir::JavaProject, Diagnostics> {
    let mut m = Mapper::new(resolve, opts);
    m.run();
    if m.diags.is_empty() {
        Ok(ir::JavaProject { files: m.files })
    } else {
        m.diags.sort();
        Err(m.diags)
    }
}

struct Mapper<'a> {
    resolve: &'a Resolve,
    opts: &'a GenerateOptions,
    diags: Diagnostics,
    registry: HashMap<Fqn, RegistryEntry>,
    type_fqns: HashMap<TypeId, Fqn>,
    pkg_java: HashMap<PackageId, String>,
    files: Vec<ir::JavaFile>,
    package_infos: HashMap<String, Vec<String>>,
}

impl<'a> Mapper<'a> {
    fn new(resolve: &'a Resolve, opts: &'a GenerateOptions) -> Self {
        Mapper {
            resolve,
            opts,
            diags: Diagnostics::default(),
            registry: HashMap::new(),
            type_fqns: HashMap::new(),
            pkg_java: HashMap::new(),
            files: Vec::new(),
            package_infos: HashMap::new(),
        }
    }

    fn mangle(
        &mut self,
        name: &str,
        form: naming::NameForm,
        taken: &dyn Fn(&str) -> bool,
    ) -> Result<String, ()> {
        match naming::mangle(name, form, taken) {
            Ok(n) => Ok(n),
            Err(d) => {
                self.push_diag(d);
                Err(())
            }
        }
    }

    fn loc(&self, span: wit_parser::Span) -> Option<Location> {
        wit::locate(self.resolve, span)
    }

    fn push_diag(&mut self, d: Diagnostic) {
        self.diags.push(d);
    }

    fn fail<T>(
        &mut self,
        span: wit_parser::Span,
        code: Code,
        msg: impl Into<String>,
    ) -> Result<T, ()> {
        let mut d = Diagnostic::new(code, msg);
        if let Some(l) = self.loc(span) {
            d = d.at(l);
        }
        self.push_diag(d);
        Err(())
    }

    fn java_pkg(&mut self, pkg_id: PackageId) -> String {
        if let Some(p) = self.pkg_java.get(&pkg_id) {
            return p.clone();
        }
        let pkg = &self.resolve.packages[pkg_id];
        let java = naming::java_package(
            &pkg.name.namespace,
            &pkg.name.name,
            pkg.name.version.as_ref(),
            self.opts.package_root.as_deref(),
            &self.opts.package_map,
        );
        self.pkg_java.insert(pkg_id, java.clone());
        java
    }

    fn register(
        &mut self,
        span: wit_parser::Span,
        fqn: Fqn,
        kind: RegisteredKind,
        wit_pkg: String,
        desc: String,
    ) -> Result<(), ()> {
        if let Some(existing) = self.registry.get(&fqn) {
            let same_pkg = existing.wit_pkg == wit_pkg;
            let code = if same_pkg {
                Code::FqnCollision
            } else {
                Code::PackageCollision
            };
            let mut d = Diagnostic::new(
                code,
                format!(
                    "`{desc}` maps to Java type `{fqn}`, already claimed by `{}`",
                    existing.desc
                ),
            );
            if let Some(l) = self.loc(span) {
                d = d.at(l);
            }
            self.push_diag(d);
            return Err(());
        }
        self.registry.insert(
            fqn.clone(),
            RegistryEntry {
                kind,
                wit_pkg,
                desc,
            },
        );
        Ok(())
    }

    fn run(&mut self) {
        let pkg_ids = wit::sorted_packages(self.resolve);
        let resolve = self.resolve;

        // Registration pass: every named type and world aggregate gets its
        // FQN claimed before any type mapping happens (forward refs).
        for pkg_id in &pkg_ids {
            let pkg = &resolve.packages[*pkg_id];
            let wit_pkg = pkg.name.to_string();
            let pkg_java = self.java_pkg(*pkg_id);
            for iface_id in pkg.interfaces.values() {
                let iface = &resolve.interfaces[*iface_id];
                let Some(name) = &iface.name else { continue };
                let base = self.interface_base(pkg_java.clone(), name);
                for (tname, tid) in &iface.types {
                    let td = &resolve.types[*tid];
                    if td.name.is_none() {
                        continue;
                    }
                    let Some(kind) = registered_kind(&td.kind) else {
                        continue;
                    };
                    let fqn = Fqn::new(base.clone(), naming::to_upper_camel(tname));
                    let desc = format!("{wit_pkg}/{name}.{tname}");
                    if self
                        .register(td.span, fqn.clone(), kind, wit_pkg.clone(), desc)
                        .is_ok()
                    {
                        self.type_fqns.insert(*tid, fqn);
                    }
                }
            }
            for (wname, world_id) in &pkg.worlds {
                let world = &resolve.worlds[*world_id];
                let world_pkg =
                    format!("{pkg_java}.{}", naming::to_lower_camel(world.name.as_str()));
                for item in world.imports.values().chain(world.exports.values()) {
                    if let WorldItem::Type { id, .. } = item {
                        let td = &resolve.types[*id];
                        let Some(tdn) = &td.name else { continue };
                        let Some(kind) = registered_kind(&td.kind) else {
                            continue;
                        };
                        let fqn = Fqn::new(world_pkg.clone(), naming::to_upper_camel(tdn));
                        let desc = format!("{wit_pkg}/{wname}.{tdn}");
                        if self
                            .register(td.span, fqn.clone(), kind, wit_pkg.clone(), desc)
                            .is_ok()
                        {
                            self.type_fqns.insert(*id, fqn);
                        }
                    }
                }
                for role_pkg in self.role_packages(&world_pkg) {
                    for agg in ["Imports", "Exports"] {
                        let fqn = Fqn::new(role_pkg.clone(), agg);
                        let desc = format!("{wit_pkg}/{wname}.{agg}");
                        let _ = self.register(
                            world.span,
                            fqn,
                            RegisteredKind::Interface,
                            wit_pkg.clone(),
                            desc,
                        );
                    }
                }
            }
        }

        // Emission pass.
        for pkg_id in &pkg_ids {
            let pkg = &resolve.packages[*pkg_id];
            let iface_ids: Vec<wit_parser::InterfaceId> =
                pkg.interfaces.values().copied().collect();
            for iface_id in iface_ids {
                self.emit_interface(*pkg_id, iface_id);
            }
            let worlds: Vec<(String, wit_parser::WorldId)> =
                pkg.worlds.iter().map(|(n, w)| (n.clone(), *w)).collect();
            for (wname, world_id) in worlds {
                if !self.opts.worlds.is_empty() && !self.opts.worlds.iter().any(|w| w == &wname) {
                    continue;
                }
                self.emit_world(*pkg_id, &wname, world_id);
            }
        }

        // package-info files: one per package that received types files.
        let mut infos: Vec<(String, Vec<String>)> = self
            .package_infos
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        infos.sort_by(|a, b| a.0.cmp(&b.0));
        for (pkg, doc) in infos {
            self.files.push(ir::JavaFile::package_info(pkg, doc));
        }
    }

    fn interface_base(&self, pkg_java: String, name: &str) -> String {
        match self.opts.interface_style {
            InterfaceStyle::Nested => format!("{pkg_java}.{}", naming::to_lower_camel(name)),
            InterfaceStyle::Flat => pkg_java,
        }
    }

    fn role_packages(&self, world_pkg: &str) -> Vec<String> {
        match self.opts.role {
            Role::Host => vec![format!("{world_pkg}.host")],
            Role::Guest => vec![format!("{world_pkg}.guest")],
            Role::Both => vec![format!("{world_pkg}.host"), format!("{world_pkg}.guest")],
        }
    }

    fn emit_interface(&mut self, pkg_id: PackageId, iface_id: wit_parser::InterfaceId) {
        let resolve = self.resolve;
        let iface = &resolve.interfaces[iface_id];
        let Some(name) = &iface.name else { return };
        let pkg_java = self.java_pkg(pkg_id);
        let base = self.interface_base(pkg_java, name);

        // Collect resource-owned functions before the mutable emission pass.
        let mut resource_funcs: HashMap<TypeId, Vec<&Function>> = HashMap::new();
        let mut freestanding: Vec<&Function> = Vec::new();
        for f in iface.functions.values() {
            match &f.kind {
                FunctionKind::Method(tid)
                | FunctionKind::Static(tid)
                | FunctionKind::Constructor(tid) => {
                    resource_funcs.entry(*tid).or_default().push(f);
                }
                _ => freestanding.push(f),
            }
        }

        let mut decls: Vec<ir::Decl> = Vec::new();
        for (_tname, tid) in &iface.types {
            let td = &resolve.types[*tid];
            let Some(tdn) = &td.name else { continue };
            let result = match &td.kind {
                TypeDefKind::Record(r) => self.emit_record(&base, tdn, td, r),
                TypeDefKind::Variant(v) => self.emit_variant(&base, tdn, td, v),
                TypeDefKind::Enum(e) => self.emit_enum(tdn, td, e),
                TypeDefKind::Flags(fl) => self.emit_flags(&base, tdn, td, fl),
                TypeDefKind::Resource => self.emit_resource(&base, tdn, td, *tid, &resource_funcs),
                TypeDefKind::Type(_) => Ok(None), // alias: no new type (§5.10)
                _ => Ok(None),
            };
            match result {
                Ok(Some(d)) => decls.push(d),
                Ok(None) => {}
                Err(()) => {}
            }
        }

        let mut taken: Vec<String> = Vec::new();
        let mut iface_methods = Vec::new();
        for f in freestanding {
            if let Ok(m) = self.emit_function(f, &mut taken) {
                iface_methods.push(m);
            }
        }

        if decls.is_empty() && iface_methods.is_empty() {
            return;
        }

        let doc = render_doc(&iface.docs);
        let javadoc = if doc.is_empty() {
            vec!["Nothing to see here.".to_string()]
        } else {
            doc
        };
        let iface_decl = ir::Decl::Interface {
            name: naming::to_upper_camel(name),
            javadoc,
            extends: Vec::new(),
            is_sealed: false,
            permits: Vec::new(),
            methods: iface_methods,
            nested: Vec::new(),
        };
        let simple = naming::to_upper_camel(name);
        for d in decls {
            let fname = d.name().to_string();
            self.files
                .push(ir::JavaFile::types_file(base.clone(), fname, d));
        }
        // Always emit the interface declaration: world accessors return the
        // interface type even when the interface has no functions of its own
        // (spec §6).
        self.files
            .push(ir::JavaFile::types_file(base.clone(), simple, iface_decl));
        let doc = render_doc(&iface.docs);
        self.package_infos.entry(base.clone()).or_insert_with(|| {
            if doc.is_empty() {
                vec!["WIT-generated package.".into()]
            } else {
                doc.clone()
            }
        });
    }

    fn type_javadoc(&mut self, td: &TypeDef, fallback: &str) -> Vec<String> {
        let doc = render_doc(&td.docs);
        if doc.is_empty() {
            vec![fallback.to_string()]
        } else {
            doc
        }
    }

    fn emit_record(
        &mut self,
        _base: &str,
        name: &str,
        td: &TypeDef,
        r: &Record,
    ) -> Result<Option<ir::Decl>, ()> {
        let mut taken: Vec<String> = Vec::new();
        let mut components = Vec::new();
        let mut javadoc = self.type_javadoc(td, "WIT-generated record.");
        for field in &r.fields {
            let jname = self.mangle(
                &naming::to_lower_camel(&field.name),
                naming::NameForm::LowerCamel,
                &|c| taken.iter().any(|t| t == c),
            )?;
            taken.push(jname.clone());
            let ty = self.resolve_type(&field.ty)?;
            let mut text = render_doc(&field.docs).join(" ");
            self.append_type_notes(&field.ty, &mut text);
            javadoc.push(format!(
                "@param {jname} {}",
                if text.is_empty() { jname.clone() } else { text }
            ));
            components.push(ir::Param {
                name: jname,
                ty,
                doc: None,
            });
        }
        Ok(Some(ir::Decl::Record {
            name: naming::to_upper_camel(name),
            javadoc,
            components,
            consts: Vec::new(),
            methods: Vec::new(),
            implements: Vec::new(),
        }))
    }

    fn emit_variant(
        &mut self,
        _base: &str,
        name: &str,
        td: &TypeDef,
        v: &Variant,
    ) -> Result<Option<ir::Decl>, ()> {
        let outer = naming::to_upper_camel(name);
        let mut permits = Vec::new();
        let mut nested = Vec::new();
        for case in &v.cases {
            let cname = naming::to_upper_camel(&case.name);
            permits.push(format!("{outer}.{cname}"));
            let mut javadoc = render_doc(&case.docs);
            let components = match &case.ty {
                Some(t) => {
                    let ty = self.resolve_type(t)?;
                    let mut text = String::new();
                    self.append_type_notes(t, &mut text);
                    let text = if text.is_empty() {
                        "value".to_string()
                    } else {
                        text
                    };
                    if javadoc.is_empty() {
                        javadoc.push(format!("WIT case `{}`.", case.name));
                    }
                    javadoc.push(format!("@param value {text}"));
                    vec![ir::Param {
                        name: "value".into(),
                        ty,
                        doc: None,
                    }]
                }
                None => {
                    if javadoc.is_empty() {
                        javadoc.push(format!("WIT case `{}`.", case.name));
                    }
                    Vec::new()
                }
            };
            nested.push(ir::Decl::Record {
                name: cname,
                javadoc,
                components,
                consts: Vec::new(),
                methods: Vec::new(),
                implements: vec![ir::TypeRef::Fqn(Fqn::new(String::new(), outer.clone()))],
            });
        }
        let javadoc = self.type_javadoc(td, "WIT-generated variant.");
        Ok(Some(ir::Decl::Interface {
            name: outer,
            javadoc,
            extends: Vec::new(),
            is_sealed: true,
            permits,
            methods: Vec::new(),
            nested,
        }))
    }

    fn emit_enum(
        &mut self,
        name: &str,
        td: &TypeDef,
        e: &wit_parser::Enum,
    ) -> Result<Option<ir::Decl>, ()> {
        let javadoc = self.type_javadoc(td, "WIT-generated enum.");
        let constants = e
            .cases
            .iter()
            .map(|c| {
                let doc = render_doc(&c.docs).join(" ");
                (
                    naming::to_upper_snake(&c.name),
                    if doc.is_empty() { None } else { Some(doc) },
                )
            })
            .collect();
        Ok(Some(ir::Decl::Enum {
            name: naming::to_upper_camel(name),
            javadoc,
            constants,
        }))
    }

    fn emit_flags(
        &mut self,
        base: &str,
        name: &str,
        td: &TypeDef,
        fl: &Flags,
    ) -> Result<Option<ir::Decl>, ()> {
        if fl.flags.len() > 64 {
            self.fail::<()>(
                td.span,
                Code::FlagsArityExceeded,
                format!(
                    "flags `{name}` has {} members; v1 supports at most 64",
                    fl.flags.len()
                ),
            )
            .ok();
            return Err(());
        }
        let outer = naming::to_upper_camel(name);
        let self_ty = ir::TypeRef::Fqn(Fqn::new(base.to_string(), outer.clone()));
        let javadoc = self.type_javadoc(td, "WIT-generated flags.");
        let mut consts = Vec::new();
        for (n, flag) in fl.flags.iter().enumerate() {
            let doc = render_doc(&flag.docs).join(" ");
            consts.push(ir::Const {
                name: naming::to_upper_snake(&flag.name),
                ty: self_ty.clone(),
                value: format!("new {outer}(1L << {n})"),
                doc: if doc.is_empty() { None } else { Some(doc) },
            });
        }
        let other = || ir::Param {
            name: "other".into(),
            ty: self_ty.clone(),
            doc: None,
        };
        let m = |name: &str, ret, params: Vec<ir::Param>, body: &str| ir::Method {
            name: name.into(),
            javadoc: Vec::new(),
            params,
            ret,
            is_static: name == "empty",
            body: Some(body.to_string().replace("PERMS", &outer)),
            annotations: Vec::new(),
        };
        let methods = vec![
            m(
                "or",
                self_ty.clone(),
                vec![other()],
                "return new PERMS(bits | other.bits);",
            ),
            m(
                "and",
                self_ty.clone(),
                vec![other()],
                "return new PERMS(bits & other.bits);",
            ),
            m(
                "contains",
                ir::TypeRef::Simple("boolean"),
                vec![other()],
                "return (bits & other.bits) == other.bits;",
            ),
            m(
                "empty",
                self_ty.clone(),
                Vec::new(),
                "return new PERMS(0L);",
            ),
        ];
        Ok(Some(ir::Decl::Record {
            name: outer,
            javadoc,
            components: vec![ir::Param {
                name: "bits".into(),
                ty: ir::TypeRef::Simple("long"),
                doc: None,
            }],
            consts,
            methods,
            implements: Vec::new(),
        }))
    }

    #[allow(clippy::too_many_arguments)]
    fn emit_resource(
        &mut self,
        base: &str,
        name: &str,
        td: &TypeDef,
        tid: TypeId,
        owners: &HashMap<TypeId, Vec<&Function>>,
    ) -> Result<Option<ir::Decl>, ()> {
        let simple = naming::to_upper_camel(name);
        let self_fqn = Fqn::new(base.to_string(), simple.clone());
        let mut taken: Vec<String> = Vec::new();
        let mut methods = Vec::new();
        methods.push(ir::Method {
            name: "close".into(),
            javadoc: vec![
                "Close this resource. Idempotent; corresponds to the WIT resource drop.".into(),
            ],
            params: Vec::new(),
            ret: ir::TypeRef::Simple("void"),
            is_static: false,
            body: None,
            annotations: vec!["@Override"],
        });
        let mut nested = Vec::new();
        let mut factory_params: Option<Vec<ir::Param>> = None;
        let mut statics: Vec<ir::Method> = Vec::new();

        if let Some(funcs) = owners.get(&tid) {
            for f in funcs {
                match &f.kind {
                    FunctionKind::Constructor(_) => {
                        factory_params = Some(self.map_params(f)?);
                    }
                    FunctionKind::Static(_) | FunctionKind::AsyncStatic(_) => {
                        statics.push(self.emit_function(f, &mut taken)?);
                    }
                    FunctionKind::Method(_) | FunctionKind::AsyncMethod(_) => {
                        methods.push(self.emit_function(f, &mut taken)?);
                    }
                    _ => {}
                }
            }
        }

        if let Some(params) = factory_params {
            let mut create_javadoc = Vec::new();
            for p in &params {
                create_javadoc.push(format!(
                    "@param {} {}",
                    p.name,
                    p.doc.clone().unwrap_or_else(|| p.name.clone())
                ));
            }
            nested.push(ir::Decl::Interface {
                name: "Factory".into(),
                javadoc: vec![format!("Factory for {{@link {simple}}}.")],
                extends: Vec::new(),
                is_sealed: false,
                permits: Vec::new(),
                methods: vec![ir::Method {
                    name: "create".into(),
                    javadoc: create_javadoc,
                    params,
                    ret: ir::TypeRef::Fqn(self_fqn.clone()),
                    is_static: false,
                    body: None,
                    annotations: Vec::new(),
                }],
                nested: Vec::new(),
            });
        }
        if !statics.is_empty() {
            nested.push(ir::Decl::Interface {
                name: "Statics".into(),
                javadoc: vec![format!("Static methods for {{@link {simple}}}.")],
                extends: Vec::new(),
                is_sealed: false,
                permits: Vec::new(),
                methods: statics,
                nested: Vec::new(),
            });
        }

        let javadoc = self.type_javadoc(td, "WIT resource.");
        Ok(Some(ir::Decl::Interface {
            name: simple,
            javadoc,
            extends: vec![ir::TypeRef::Fqn(Fqn::new(
                "java.lang".to_string(),
                "AutoCloseable".to_string(),
            ))],
            is_sealed: false,
            permits: Vec::new(),
            methods,
            nested,
        }))
    }

    fn emit_function(&mut self, f: &Function, taken: &mut Vec<String>) -> Result<ir::Method, ()> {
        let jname = self.mangle(
            &naming::to_lower_camel(leaf_name(&f.name)),
            naming::NameForm::LowerCamel,
            &|c| taken.iter().any(|t| t == c),
        )?;
        taken.push(jname.clone());
        let params = self.map_params(f)?;
        let ret = match &f.result {
            Some(t) => self.resolve_type_in(t, false, Pos::Return)?,
            None => ir::TypeRef::Simple("void"),
        };
        let mut javadoc = render_doc(&f.docs);
        if javadoc.is_empty() {
            javadoc.push(format!("{}.", jname));
        }
        for p in &params {
            javadoc.push(format!(
                "@param {} {}",
                p.name,
                p.doc.clone().unwrap_or_else(|| p.name.clone())
            ));
        }
        let is_void = matches!(ret, ir::TypeRef::Simple("void"));
        if !is_void {
            javadoc.push(format!("@return {jname}"));
        }
        if let Some(t) = &f.result {
            if self.type_has_owned(t) {
                javadoc.push(OWN_RETURN_NOTE.to_string());
            } else if !is_void {
                let mut text = String::new();
                self.append_type_notes(t, &mut text);
                if !text.is_empty() {
                    javadoc.push(text);
                }
            }
        }
        Ok(ir::Method {
            name: jname,
            javadoc,
            params,
            ret,
            is_static: false,
            body: None,
            annotations: Vec::new(),
        })
    }

    fn map_params(&mut self, f: &Function) -> Result<Vec<ir::Param>, ()> {
        let mut taken: Vec<String> = Vec::new();
        let mut out = Vec::new();
        let skip_self = matches!(
            f.kind,
            FunctionKind::Method(_) | FunctionKind::AsyncMethod(_)
        );
        for p in f.params.iter().skip(if skip_self { 1 } else { 0 }) {
            let jname = self.mangle(
                &naming::to_lower_camel(&p.name),
                naming::NameForm::LowerCamel,
                &|c| taken.iter().any(|t| t == c),
            )?;
            taken.push(jname.clone());
            let ty = self.resolve_type(&p.ty)?;
            let mut doc: Option<String> = None;
            if self.type_is_borrow(&p.ty) {
                doc = Some(BORROW_NOTE.to_string());
            } else if self.type_has_owned(&p.ty) {
                doc = Some(OWN_PARAM_NOTE.to_string());
            } else {
                let mut text = String::new();
                self.append_type_notes(&p.ty, &mut text);
                if !text.is_empty() {
                    doc = Some(text);
                }
            }
            out.push(ir::Param {
                name: jname,
                ty,
                doc,
            });
        }
        Ok(out)
    }

    fn emit_world(&mut self, pkg_id: PackageId, wname: &str, world_id: wit_parser::WorldId) {
        let resolve = self.resolve;
        let world: &World = &resolve.worlds[world_id];
        let pkg_java = self.java_pkg(pkg_id);
        let world_pkg = format!("{pkg_java}.{}", naming::to_lower_camel(wname));
        let role_pkgs = self.role_packages(&world_pkg);
        for role_pkg in role_pkgs {
            let is_host = role_pkg.ends_with(".host");
            let (imports_doc, exports_doc) = if is_host {
                (
                    "World imports. Implemented by the host; called by the component.",
                    "World exports. Called by the host; implemented by the component.",
                )
            } else {
                (
                    "World imports. Called by the guest; provided by the host.",
                    "World exports. Implemented by the guest; called by the host.",
                )
            };
            let imports = self.emit_aggregate(&role_pkg, "Imports", &world.imports, imports_doc);
            let exports = self.emit_aggregate(&role_pkg, "Exports", &world.exports, exports_doc);
            if let (Ok(i), Ok(e)) = (imports, exports) {
                self.package_infos
                    .entry(role_pkg.clone())
                    .or_insert_with(|| {
                        let d = render_doc(&world.docs);
                        if d.is_empty() {
                            vec!["WIT-generated package.".into()]
                        } else {
                            d
                        }
                    });
                self.files.push(i);
                self.files.push(e);
            }
        }
    }

    fn emit_aggregate(
        &mut self,
        role_pkg: &str,
        agg_name: &str,
        items: &indexmap::IndexMap<WorldKey, WorldItem>,
        doc: &str,
    ) -> Result<ir::JavaFile, ()> {
        let mut taken: Vec<String> = Vec::new();
        let mut methods = Vec::new();
        for (key, item) in items {
            match (key, item) {
                (WorldKey::Name(_), WorldItem::Function(f)) => {
                    methods.push(self.emit_function(f, &mut taken)?);
                }
                (WorldKey::Interface(iface_id), WorldItem::Interface { .. }) => {
                    let resolve = self.resolve;
                    let iface = &resolve.interfaces[*iface_id];
                    let Some(iname) = &iface.name else { continue };
                    let jname = self.mangle(
                        &naming::to_lower_camel(iname),
                        naming::NameForm::LowerCamel,
                        &|c| taken.iter().any(|t| t == c),
                    )?;
                    taken.push(jname.clone());
                    let fqn = self.interface_fqn(*iface_id)?;
                    methods.push(ir::Method {
                        name: jname,
                        javadoc: vec!["Access the imported interface.".into()],
                        params: Vec::new(),
                        ret: ir::TypeRef::Fqn(fqn),
                        is_static: false,
                        body: None,
                        annotations: Vec::new(),
                    });
                }
                (WorldKey::Name(_), WorldItem::Type { .. }) => {
                    // world-local types land in the world package (§7.2)
                }
                _ => {}
            }
        }
        let decl = ir::Decl::Interface {
            name: agg_name.into(),
            javadoc: vec![doc.into()],
            extends: Vec::new(),
            is_sealed: false,
            permits: Vec::new(),
            methods,
            nested: Vec::new(),
        };
        Ok(ir::JavaFile::types_file(
            role_pkg.to_string(),
            agg_name,
            decl,
        ))
    }

    fn interface_fqn(&mut self, iface_id: wit_parser::InterfaceId) -> Result<Fqn, ()> {
        let resolve = self.resolve;
        let iface = &resolve.interfaces[iface_id];
        let Some(name) = &iface.name else {
            return Err(());
        };
        let Some(pkg_id) = iface.package else {
            return Err(());
        };
        let pkg_java = self.java_pkg(pkg_id);
        Ok(Fqn::new(
            self.interface_base(pkg_java, name),
            naming::to_upper_camel(name),
        ))
    }

    // ------------------------------------------------------------ type map

    fn resolve_type(&mut self, ty: &Type) -> Result<ir::TypeRef, ()> {
        self.resolve_type_in(ty, false, Pos::Other)
    }

    fn resolve_type_in(
        &mut self,
        ty: &Type,
        inside_option: bool,
        pos: Pos,
    ) -> Result<ir::TypeRef, ()> {
        let simple = |s: &'static str| Ok(ir::TypeRef::Simple(s));
        match ty {
            Type::Bool => simple("boolean"),
            Type::U8 | Type::U16 | Type::Char => simple("int"),
            Type::U32 | Type::S64 => simple("long"),
            Type::U64 => match self.opts.u64_style {
                U64Style::Long => simple("long"),
                U64Style::BigInteger => Ok(ir::TypeRef::Fqn(Fqn::new(
                    "java.math".to_string(),
                    "BigInteger".to_string(),
                ))),
            },
            Type::S8 => simple("byte"),
            Type::S16 => simple("short"),
            Type::S32 => simple("int"),
            Type::F32 => simple("float"),
            Type::F64 => simple("double"),
            Type::String => simple("String"),
            Type::ErrorContext => self.fail_type(ty, "error-context"),
            Type::Id(tid) => {
                let resolve = self.resolve;
                let td = &resolve.types[*tid];
                match &td.kind {
                    TypeDefKind::Record(_)
                    | TypeDefKind::Variant(_)
                    | TypeDefKind::Enum(_)
                    | TypeDefKind::Flags(_)
                    | TypeDefKind::Resource => match self.type_fqns.get(tid) {
                        Some(fqn) => Ok(ir::TypeRef::Fqn(fqn.clone())),
                        None => {
                            let name = td.name.clone().unwrap_or_default();
                            let span = td.span;
                            self.fail::<ir::TypeRef>(
                                span,
                                Code::FqnCollision,
                                format!("internal: type `{name}` was never registered"),
                            )?;
                            unreachable!()
                        }
                    },
                    TypeDefKind::Handle(handle) => {
                        let inner = match handle {
                            Handle::Own(id) | Handle::Borrow(id) => *id,
                        };
                        // the target may itself be a `use` alias chain;
                        // resolve recursively to the underlying resource
                        self.resolve_type_in(&Type::Id(inner), inside_option, pos)
                    }
                    TypeDefKind::Type(inner) => self.resolve_type_in(inner, inside_option, pos),
                    TypeDefKind::Option(inner) => {
                        if matches!(self.opts.option_style, OptionStyle::Nullable)
                            && pos != Pos::Return
                        {
                            if inside_option {
                                return self.fail(
                                    td.span,
                                    Code::NestedOptionUnderNullable,
                                    "nested `option` under --option-style=nullable",
                                );
                            }
                            return self
                                .resolve_type_in(inner, true, Pos::Other)
                                .map(|t| ir::TypeRef::Nullable(Box::new(boxed(t))));
                        }
                        // optional style — and returns under nullable style —
                        // map to Optional (spec §5.4); inner args use the
                        // other-position rules
                        self.resolve_type_in(inner, true, Pos::Other).map(|t| {
                            ir::TypeRef::Generic {
                                fqn: Fqn::new("java.util".to_string(), "Optional".to_string()),
                                args: vec![boxed(t)],
                            }
                        })
                    }
                    TypeDefKind::List(inner) => match inner {
                        Type::U8 | Type::S8 => {
                            Ok(ir::TypeRef::Array(Box::new(ir::TypeRef::Simple("byte"))))
                        }
                        _ => self
                            .resolve_type_in(inner, inside_option, Pos::Other)
                            .map(|t| ir::TypeRef::Generic {
                                fqn: Fqn::new("java.util".to_string(), "List".to_string()),
                                args: vec![boxed(t)],
                            }),
                    },
                    TypeDefKind::Tuple(t) => self.emit_tuple(t),
                    TypeDefKind::Result(r) => self.emit_result(r),
                    TypeDefKind::Future(_) => self.fail_type(ty, "future"),
                    TypeDefKind::Stream(_) => self.fail_type(ty, "stream"),
                    TypeDefKind::FixedLengthList(_, _) => self.fail_type(ty, "fixed-size list"),
                    TypeDefKind::Map(_, _) => self.fail_type(ty, "map"),
                    TypeDefKind::Unknown => Err(()),
                }
            }
        }
    }

    fn fail_type(&mut self, ty: &Type, what: &str) -> Result<ir::TypeRef, ()> {
        if let Type::Id(tid) = ty {
            let span = self.resolve.types[*tid].span;
            return self.fail(
                span,
                Code::UnsupportedTypeConstruct,
                format!("`{what}` is unsupported in mapping v1"),
            );
        }
        self.push_diag(Diagnostic::new(
            Code::UnsupportedTypeConstruct,
            format!("`{what}` is unsupported in mapping v1"),
        ));
        Err(())
    }

    fn emit_tuple(&mut self, t: &Tuple) -> Result<ir::TypeRef, ()> {
        if t.types.len() > 8 {
            self.push_diag(Diagnostic::new(
                Code::TupleArityExceeded,
                format!(
                    "tuple has {} components; v1 supports at most 8",
                    t.types.len()
                ),
            ));
            return Err(());
        }
        if t.types.is_empty() {
            return Ok(ir::TypeRef::Fqn(self.support_fqn("Unit")));
        }
        let args = t
            .types
            .iter()
            .map(|ty| self.resolve_type(ty).map(boxed))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ir::TypeRef::Generic {
            fqn: self.support_fqn(&format!("Tuple{}", t.types.len())),
            args,
        })
    }

    fn emit_result(&mut self, r: &Result_) -> Result<ir::TypeRef, ()> {
        let ok = match &r.ok {
            Some(t) => self.resolve_type(t).map(boxed)?,
            None => ir::TypeRef::Fqn(self.support_fqn("Unit")),
        };
        let err = match &r.err {
            Some(t) => self.resolve_type(t).map(boxed)?,
            None => ir::TypeRef::Fqn(self.support_fqn("Unit")),
        };
        Ok(ir::TypeRef::Generic {
            fqn: self.support_fqn("Result"),
            args: vec![ok, err],
        })
    }

    fn support_fqn(&self, name: &str) -> Fqn {
        Fqn::new(self.opts.support_package.clone(), name)
    }

    // --------------------------------------------------- type introspection

    fn append_type_notes(&self, ty: &Type, text: &mut String) {
        if self.type_walk(ty, &mut |t| matches!(t, Type::U64)) {
            if !text.is_empty() {
                text.push(' ');
            }
            text.push_str(U64_NOTE);
        }
        if self.type_walk(ty, &mut |t| matches!(t, Type::Char)) {
            if !text.is_empty() {
                text.push(' ');
            }
            text.push_str(CHAR_NOTE);
        }
    }

    fn type_is_borrow(&self, ty: &Type) -> bool {
        matches!(ty, Type::Id(tid) if matches!(&self.resolve.types[*tid].kind, TypeDefKind::Handle(Handle::Borrow(_))))
    }

    fn type_has_owned(&self, ty: &Type) -> bool {
        self.type_walk(ty, &mut |t| {
            matches!(t, Type::Id(tid) if matches!(
                &self.resolve.types[*tid].kind,
                TypeDefKind::Handle(Handle::Own(_)) | TypeDefKind::Resource
            ))
        })
    }

    fn type_walk(&self, ty: &Type, pred: &mut dyn FnMut(&Type) -> bool) -> bool {
        let mut visited = std::collections::HashSet::new();
        self.type_walk_inner(ty, pred, &mut visited)
    }

    fn type_walk_inner(
        &self,
        ty: &Type,
        pred: &mut dyn FnMut(&Type) -> bool,
        visited: &mut std::collections::HashSet<TypeId>,
    ) -> bool {
        if pred(ty) {
            return true;
        }
        let Type::Id(tid) = ty else { return false };
        if !visited.insert(*tid) {
            return false;
        }
        let td = &self.resolve.types[*tid];
        match &td.kind {
            TypeDefKind::Type(inner) | TypeDefKind::Option(inner) | TypeDefKind::List(inner) => {
                self.type_walk_inner(inner, pred, visited)
            }
            TypeDefKind::Tuple(t) => t
                .types
                .iter()
                .any(|ty| self.type_walk_inner(ty, pred, visited)),
            TypeDefKind::Result(r) => {
                r.ok.as_ref()
                    .is_some_and(|ty| self.type_walk_inner(ty, pred, visited))
                    || r.err
                        .as_ref()
                        .is_some_and(|ty| self.type_walk_inner(ty, pred, visited))
            }
            TypeDefKind::Handle(Handle::Own(_)) | TypeDefKind::Resource => true,
            TypeDefKind::Record(r) => r
                .fields
                .iter()
                .any(|f| self.type_walk_inner(&f.ty, pred, visited)),
            TypeDefKind::Variant(v) => v.cases.iter().any(|c| {
                c.ty.as_ref()
                    .is_some_and(|ty| self.type_walk_inner(ty, pred, visited))
            }),
            _ => false,
        }
    }
}

/// wit-parser encodes resource methods as `[method]widget.label`,
/// `[static]widget.duplicate`, `[constructor]widget` — take the leaf name.
fn leaf_name(name: &str) -> &str {
    match name.rfind('.') {
        Some(pos) => &name[pos + 1..],
        None => name,
    }
}

/// Java generics need reference types: box primitive mappings when they
/// appear as generic type arguments (spec §5, boxing rule).
fn boxed(ty: ir::TypeRef) -> ir::TypeRef {
    match ty {
        ir::TypeRef::Simple("boolean") => ir::TypeRef::Simple("Boolean"),
        ir::TypeRef::Simple("byte") => ir::TypeRef::Simple("Byte"),
        ir::TypeRef::Simple("short") => ir::TypeRef::Simple("Short"),
        ir::TypeRef::Simple("int") => ir::TypeRef::Simple("Integer"),
        ir::TypeRef::Simple("long") => ir::TypeRef::Simple("Long"),
        ir::TypeRef::Simple("float") => ir::TypeRef::Simple("Float"),
        ir::TypeRef::Simple("double") => ir::TypeRef::Simple("Double"),
        ir::TypeRef::Nullable(inner) => ir::TypeRef::Nullable(Box::new(boxed(*inner))),
        other => other,
    }
}

fn registered_kind(kind: &TypeDefKind) -> Option<RegisteredKind> {
    Some(match kind {
        TypeDefKind::Record(_) => RegisteredKind::Record,
        TypeDefKind::Variant(_) => RegisteredKind::Variant,
        TypeDefKind::Enum(_) => RegisteredKind::Enum,
        TypeDefKind::Flags(_) => RegisteredKind::Flags,
        TypeDefKind::Resource => RegisteredKind::Resource,
        _ => return None,
    })
}

/// Renders WIT docs to final Javadoc lines: HTML-escaped per spec §9
/// (`<`, `>`, `&`; line-leading `@`). Generator-emitted `@param`/`@return`
/// lines are appended later, already literal.
fn render_doc(docs: &Docs) -> Vec<String> {
    match &docs.contents {
        Some(text) => text
            .lines()
            .map(|l| l.trim_end().to_string())
            .filter(|l| !l.trim().is_empty())
            .map(|l| escape_doc_line(&l))
            .collect(),
        None => Vec::new(),
    }
}

fn escape_doc_line(l: &str) -> String {
    let l = l
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    if let Some(rest) = l.strip_prefix('@') {
        format!("&#64;{rest}")
    } else {
        l
    }
}

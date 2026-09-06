//! Java declaration IR (spec: declarations only; no string rendering before
//! the renderer, imports resolved by a dedicated pass).

use crate::naming::Fqn;

#[derive(Debug, Clone)]
pub enum TypeRef {
    /// A primitive or `java.lang` type rendered as a simple name
    /// (`int`, `long`, `String`, `BigInteger`-qualified? no — BigInteger is
    /// `Fqn`). Never imported.
    Simple(&'static str),
    /// `byte[]`-style arrays.
    Array(Box<TypeRef>),
    /// A generic type: `Result<byte[], StreamError>`.
    Generic { fqn: Fqn, args: Vec<TypeRef> },
    /// A plain qualified reference.
    Fqn(Fqn),
    /// `@Nullable T` (nullable option style).
    Nullable(Box<TypeRef>),
}

impl TypeRef {
    /// Visitor over every `Fqn` referenced.
    pub fn visit_fqns<'a>(&'a self, out: &mut Vec<&'a Fqn>) {
        match self {
            TypeRef::Simple(_) => {}
            TypeRef::Array(inner) | TypeRef::Nullable(inner) => inner.visit_fqns(out),
            TypeRef::Generic { fqn, args } => {
                out.push(fqn);
                for a in args {
                    a.visit_fqns(out);
                }
            }
            TypeRef::Fqn(fqn) => out.push(fqn),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub ty: TypeRef,
    pub doc: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Method {
    pub name: String,
    /// First line(s) = summary; then `@param`/`@return`/notes, already
    /// escaped and finalized by the mapper.
    pub javadoc: Vec<String>,
    pub params: Vec<Param>,
    pub ret: TypeRef,
    pub is_static: bool,
    /// Flags-record methods carry bodies; interface methods do not.
    pub body: Option<String>,
    /// e.g. `@Override`.
    pub annotations: Vec<&'static str>,
}

#[derive(Debug, Clone)]
pub struct Const {
    pub name: String,
    pub ty: TypeRef,
    /// Pre-rendered initializer, e.g. `new Perms(1L << 0)`.
    pub value: String,
    pub doc: Option<String>,
}

#[derive(Debug, Clone)]
pub enum Decl {
    Record {
        name: String,
        javadoc: Vec<String>,
        components: Vec<Param>,
        consts: Vec<Const>,
        methods: Vec<Method>,
        implements: Vec<TypeRef>,
    },
    Enum {
        name: String,
        javadoc: Vec<String>,
        constants: Vec<(String, Option<String>)>,
    },
    Interface {
        name: String,
        javadoc: Vec<String>,
        extends: Vec<TypeRef>,
        is_sealed: bool,
        /// Simple names (possibly nested `Shape.Circle`), same file.
        permits: Vec<String>,
        methods: Vec<Method>,
        nested: Vec<Decl>,
    },
}

impl Decl {
    pub fn name(&self) -> &str {
        match self {
            Decl::Record { name, .. } | Decl::Enum { name, .. } | Decl::Interface { name, .. } => {
                name
            }
        }
    }

    pub fn javadoc(&self) -> &[String] {
        match self {
            Decl::Record { javadoc, .. }
            | Decl::Enum { javadoc, .. }
            | Decl::Interface { javadoc, .. } => javadoc,
        }
    }
}

/// One output file = one top-level declaration (Java public-type rule), or a
/// `package-info.java`.
#[derive(Debug, Clone)]
pub struct JavaFile {
    pub package: String,
    /// File name without `.java`.
    pub file_name: String,
    /// `Some` for `package-info.java` files.
    pub package_doc: Option<Vec<String>>,
    pub decls: Vec<Decl>,
}

impl JavaFile {
    pub fn types_file(package: impl Into<String>, name: impl Into<String>, decl: Decl) -> Self {
        JavaFile {
            package: package.into(),
            file_name: name.into(),
            package_doc: None,
            decls: vec![decl],
        }
    }

    pub fn package_info(package: impl Into<String>, doc: Vec<String>) -> Self {
        JavaFile {
            package: package.into(),
            file_name: "package-info".into(),
            package_doc: Some(doc),
            decls: Vec::new(),
        }
    }

    pub fn is_package_info(&self) -> bool {
        self.package_doc.is_some()
    }
}

#[derive(Debug, Clone, Default)]
pub struct JavaProject {
    pub files: Vec<JavaFile>,
}

//! Diagnostics: frozen error codes (mapping spec, `error-codes.md`), source
//! locations, and multi-error collection (spec §10).

use std::fmt;

/// Frozen diagnostic codes. Appending is allowed; changing existing codes is
/// a breaking change (spec §15).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Code {
    UnsupportedTypeConstruct,
    TupleArityExceeded,
    FlagsArityExceeded,
    FqnCollision,
    ManglingCollision,
    NestedOptionUnderNullable,
    PackageCollision,
}

impl Code {
    pub fn as_str(&self) -> &'static str {
        match self {
            Code::UnsupportedTypeConstruct => "WJ0001",
            Code::TupleArityExceeded => "WJ0002",
            Code::FlagsArityExceeded => "WJ0003",
            Code::FqnCollision => "WJ0004",
            Code::ManglingCollision => "WJ0005",
            Code::NestedOptionUnderNullable => "WJ0006",
            Code::PackageCollision => "WJ0007",
        }
    }
}

impl fmt::Display for Code {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A rendered WIT source location (`path:line:col`) or a structural
/// description when no span is available.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Location(pub String);

impl fmt::Display for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub code: Code,
    pub message: String,
    pub location: Option<Location>,
}

impl Diagnostic {
    pub fn new(code: Code, message: impl Into<String>) -> Self {
        Diagnostic {
            code,
            message: message.into(),
            location: None,
        }
    }

    pub fn at(mut self, location: impl Into<Location>) -> Self {
        self.location = Some(location.into());
        self
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.location {
            Some(loc) => write!(f, "error[{}]: {} ({})", self.code, self.message, loc),
            None => write!(f, "error[{}]: {}", self.code, self.message),
        }
    }
}

/// Ordered, deduplicated collection (spec §10: collect and report together;
/// spec §12: diagnostics ordered by (code, location)).
#[derive(Debug, Clone, Default)]
pub struct Diagnostics(pub Vec<Diagnostic>);

impl Diagnostics {
    pub fn push(&mut self, d: Diagnostic) {
        self.0.push(d);
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn sort(&mut self) {
        self.0.sort_by(|a, b| {
            a.code
                .as_str()
                .cmp(b.code.as_str())
                .then_with(|| a.location.cmp(&b.location))
                .then_with(|| a.message.cmp(&b.message))
        });
        self.0.dedup_by(|a, b| {
            a.code == b.code && a.message == b.message && a.location == b.location
        });
    }
}

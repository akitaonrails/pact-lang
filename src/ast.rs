use crate::lexer::{Span, DurationUnit};

/// A complete Pact module
#[derive(Debug, Clone)]
pub struct Module {
    pub name: String,
    pub provenance: Option<Provenance>,
    pub version: Option<i64>,
    pub parent_version: Option<i64>,
    pub delta: Option<Delta>,
    pub types: Vec<TypeDef>,
    pub effect_sets: Vec<EffectSetDef>,
    pub functions: Vec<FnDef>,
    pub extra_meta: Vec<(String, MetaValue)>,
    pub span: Span,
}

/// Provenance metadata — tracks why something exists
#[derive(Debug, Clone)]
pub struct Provenance {
    pub req: Option<String>,
    pub author: Option<String>,
    pub created: Option<String>,
    pub test: Vec<String>,
    pub extra: Vec<(String, MetaValue)>,
    pub span: Span,
}

/// Delta description — what changed from parent version
#[derive(Debug, Clone)]
pub struct Delta {
    pub operation: String,
    pub target: String,
    pub description: Option<String>,
    pub span: Span,
}

/// Type definition with invariants and fields
#[derive(Debug, Clone)]
pub struct TypeDef {
    pub name: String,
    pub invariants: Vec<InvariantExpr>,
    pub fields: Vec<FieldDef>,
    pub extra_meta: Vec<(String, MetaValue)>,
    pub span: Span,
}

/// An invariant expression (stored as raw S-expression text for now)
#[derive(Debug, Clone)]
pub struct InvariantExpr {
    pub raw: String,
    pub span: Span,
}

/// Field definition within a type
#[derive(Debug, Clone)]
pub struct FieldDef {
    pub name: String,
    pub type_expr: TypeExpr,
    pub immutable: bool,
    pub generated: bool,
    pub min_len: Option<i64>,
    pub max_len: Option<i64>,
    pub format: Option<String>,
    pub unique_within: Option<String>,
    pub extra_meta: Vec<(String, MetaValue)>,
    pub span: Span,
}

/// Effect set definition
#[derive(Debug, Clone)]
pub struct EffectSetDef {
    pub name: String,
    pub effects: Vec<Effect>,
    pub span: Span,
}

/// A single effect (reads/writes/sends + target)
#[derive(Debug, Clone)]
pub struct Effect {
    pub kind: EffectKind,
    pub target: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EffectKind {
    Reads,
    Writes,
    Sends,
}

/// Function definition
#[derive(Debug, Clone)]
pub struct FnDef {
    pub name: String,
    pub provenance: Option<Provenance>,
    pub effects: Vec<String>,       // names of effect sets
    pub total: bool,
    pub latency_budget: Option<Duration>,
    pub called_by: Vec<String>,
    pub idempotency_key: Option<Expr>,
    pub params: Vec<ParamDef>,
    pub returns: ReturnsDef,
    pub body: Expr,
    pub extra_meta: Vec<(String, MetaValue)>,
    pub span: Span,
}

/// Duration value
#[derive(Debug, Clone)]
pub struct Duration {
    pub value: u64,
    pub unit: DurationUnit,
}

impl std::fmt::Display for Duration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}{}", self.value, self.unit)
    }
}

/// Parameter definition
#[derive(Debug, Clone)]
pub struct ParamDef {
    pub name: String,
    pub type_expr: TypeExpr,
    pub source: Option<String>,
    pub content_type: Option<String>,
    pub validated_at: Option<String>,
    pub extra_meta: Vec<(String, MetaValue)>,
    pub span: Span,
}

/// Returns definition (wraps a union of variants)
#[derive(Debug, Clone)]
pub struct ReturnsDef {
    pub variants: Vec<Variant>,
    pub span: Span,
}

/// A variant in a union return type
#[derive(Debug, Clone)]
pub struct Variant {
    pub kind: VariantKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum VariantKind {
    Ok {
        type_expr: TypeExpr,
        http_status: Option<i64>,
        serialize: Option<String>,
        extra_meta: Vec<(String, MetaValue)>,
    },
    Err {
        tag: String,               // e.g., "not-found", "invalid-id"
        payload: TypeExpr,         // could be a map type or a named type
        http_status: Option<i64>,
        extra_meta: Vec<(String, MetaValue)>,
    },
}

/// Type expressions
#[derive(Debug, Clone)]
pub enum TypeExpr {
    Named(String),                              // UUID, String, User
    Map(Vec<(String, TypeExpr)>),               // {:name String :email String}
    List(Box<TypeExpr>),                        // (list ValidationError)
    Union(Vec<Variant>),                        // (union ...)
    Enum(Vec<String>),                          // (enum :admin :member :guest)
}

/// Expressions
#[derive(Debug, Clone)]
pub enum Expr {
    /// Symbol reference
    Ref(String, Span),
    /// Keyword literal
    Keyword(String, Span),
    /// String literal
    StringLit(String, Span),
    /// Integer literal
    IntLit(i64, Span),
    /// Boolean literal
    BoolLit(bool, Span),
    /// Let binding: (let [bindings...] body)
    Let {
        bindings: Vec<(String, Expr)>,
        body: Box<Expr>,
        span: Span,
    },
    /// Match expression: (match expr arms...)
    Match {
        expr: Box<Expr>,
        arms: Vec<MatchArm>,
        span: Span,
    },
    /// If expression: (if cond then else)
    If {
        cond: Box<Expr>,
        then_branch: Box<Expr>,
        else_branch: Box<Expr>,
        span: Span,
    },
    /// Function call: (fn-name args...)
    Call {
        name: String,
        args: Vec<Expr>,
        span: Span,
    },
    /// Field access: (. expr field)
    FieldAccess {
        expr: Box<Expr>,
        field: String,
        span: Span,
    },
    /// Ok constructor: (ok value)
    Ok(Box<Expr>, Span),
    /// Err constructor: (err :tag payload)
    Err {
        tag: String,
        payload: Box<Expr>,
        span: Span,
    },
    /// Map literal: {:key value ...}
    MapLit(Vec<(String, Expr)>, Span),
    /// Wildcard pattern `_`
    Wildcard(Span),
}

/// A match arm: pattern → expression
#[derive(Debug, Clone)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub body: Expr,
    pub span: Span,
}

/// Patterns for match expressions
#[derive(Debug, Clone)]
pub enum Pattern {
    /// Wildcard _
    Wildcard(Span),
    /// Variable binding
    Var(String, Span),
    /// Constructor pattern: (ok x), (err tag), (some x), (none)
    Constructor {
        name: String,
        args: Vec<Pattern>,
        span: Span,
    },
    /// Keyword pattern: :not-found, :unique-violation
    Keyword(String, Span),
}

/// Catch-all metadata value
#[derive(Debug, Clone)]
pub enum MetaValue {
    String(String),
    Int(i64),
    Bool(bool),
    Symbol(String),
    Keyword(String),
    List(Vec<MetaValue>),
    Map(Vec<(String, MetaValue)>),
    Duration(u64, DurationUnit),
    Expr(Expr),
}

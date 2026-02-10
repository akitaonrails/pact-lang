/// Typed specification document parsed from YAML.
#[derive(Debug, Clone)]
pub struct SpecDoc {
    pub spec_id: String,
    pub title: String,
    pub owner: String,
    pub domain_types: Vec<DomainType>,
    pub endpoints: Vec<Endpoint>,
    pub quality: Vec<QualityRule>,
    pub traceability: Traceability,
}

/// A domain type with its fields.
#[derive(Debug, Clone)]
pub struct DomainType {
    pub name: String,
    pub fields: Vec<FieldSpec>,
}

/// A field descriptor parsed from natural-language spec.
#[derive(Debug, Clone)]
pub struct FieldSpec {
    pub name: String,
    pub required: bool,
    pub field_type: FieldType,
    pub min_len: Option<usize>,
    pub max_len: Option<usize>,
    pub format: Option<String>,
    pub unique: bool,
    pub auto_generated: bool,
    pub immutable: bool,
}

/// The type of a field.
#[derive(Debug, Clone, PartialEq)]
pub enum FieldType {
    StringType,
    UuidType,
    IntType,
    BoolType,
    Unknown(String),
}

/// An endpoint specification.
#[derive(Debug, Clone)]
pub struct Endpoint {
    pub name: String,
    pub description: String,
    pub input: InputSpec,
    pub outputs: Vec<OutputSpec>,
    pub constraints: Vec<Constraint>,
}

/// Where endpoint input comes from.
#[derive(Debug, Clone)]
pub struct InputSpec {
    pub description: String,
    pub source: InputSource,
    pub fields: Vec<FieldSpec>,
}

/// The source of input data.
#[derive(Debug, Clone, PartialEq)]
pub enum InputSource {
    Url,
    Body,
    Unknown,
}

/// An output variant of an endpoint.
#[derive(Debug, Clone)]
pub struct OutputSpec {
    pub label: String,
    pub description: String,
    pub http_status: Option<u16>,
    pub is_success: bool,
}

/// A constraint on an endpoint.
#[derive(Debug, Clone, PartialEq)]
pub enum Constraint {
    ReadOnly,
    Write,
    MaxResponseTime(String),
    Idempotent(String),
    Other(String),
}

/// Quality rules for the whole spec.
#[derive(Debug, Clone, PartialEq)]
pub enum QualityRule {
    AllFunctionsTotal,
    Other(String),
}

/// Traceability metadata.
#[derive(Debug, Clone)]
pub struct Traceability {
    pub known_dependencies: Vec<String>,
}

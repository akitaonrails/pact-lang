/// A YAML value — our minimal subset of YAML.
#[derive(Debug, Clone, PartialEq)]
pub enum YamlValue {
    /// A scalar string value (plain or quoted).
    Scalar(String),
    /// An ordered mapping of string keys to YAML values.
    Mapping(Vec<(String, YamlValue)>),
    /// A sequence (list) of YAML values.
    Sequence(Vec<YamlValue>),
}

impl YamlValue {
    /// Get as a scalar string reference.
    pub fn as_scalar(&self) -> Option<&str> {
        match self {
            YamlValue::Scalar(s) => Some(s),
            _ => None,
        }
    }

    /// Get as a mapping reference.
    pub fn as_mapping(&self) -> Option<&[(String, YamlValue)]> {
        match self {
            YamlValue::Mapping(m) => Some(m),
            _ => None,
        }
    }

    /// Get as a sequence reference.
    pub fn as_sequence(&self) -> Option<&[YamlValue]> {
        match self {
            YamlValue::Sequence(s) => Some(s),
            _ => None,
        }
    }

    /// Look up a key in a mapping.
    pub fn get(&self, key: &str) -> Option<&YamlValue> {
        match self {
            YamlValue::Mapping(pairs) => {
                pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v)
            }
            _ => None,
        }
    }
}

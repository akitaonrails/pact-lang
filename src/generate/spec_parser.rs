use super::yaml_ast::YamlValue;
use super::spec_ast::*;

/// Parse a YamlValue (top-level mapping) into a typed SpecDoc.
pub fn parse_spec(yaml: &YamlValue) -> Result<SpecDoc, SpecParseError> {
    let mapping = yaml.as_mapping().ok_or_else(|| {
        SpecParseError("Expected top-level YAML mapping".into())
    })?;

    let spec_id = get_scalar(yaml, "spec")
        .unwrap_or_default();
    let title = get_scalar(yaml, "title")
        .unwrap_or_default();
    let owner = get_scalar(yaml, "owner")
        .unwrap_or_default();

    let domain_types = parse_domain(yaml)?;
    let endpoints = parse_endpoints(yaml)?;
    let quality = parse_quality(yaml);
    let traceability = parse_traceability(yaml);

    // Verify we actually consumed the top-level mapping
    let _ = mapping;

    Ok(SpecDoc {
        spec_id,
        title,
        owner,
        domain_types,
        endpoints,
        quality,
        traceability,
    })
}

#[derive(Debug)]
pub struct SpecParseError(pub String);

impl std::fmt::Display for SpecParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Spec parse error: {}", self.0)
    }
}

fn get_scalar(yaml: &YamlValue, key: &str) -> Option<String> {
    yaml.get(key)?.as_scalar().map(|s| s.to_string())
}

fn parse_domain(yaml: &YamlValue) -> Result<Vec<DomainType>, SpecParseError> {
    let domain = match yaml.get("domain") {
        Some(d) => d,
        None => return Ok(Vec::new()),
    };

    let pairs = domain.as_mapping().ok_or_else(|| {
        SpecParseError("'domain' must be a mapping".into())
    })?;

    let mut types = Vec::new();
    for (type_name, type_val) in pairs {
        let fields = parse_fields(type_val)?;
        types.push(DomainType {
            name: type_name.clone(),
            fields,
        });
    }
    Ok(types)
}

fn parse_fields(type_val: &YamlValue) -> Result<Vec<FieldSpec>, SpecParseError> {
    let fields_val = match type_val.get("fields") {
        Some(f) => f,
        None => return Ok(Vec::new()),
    };

    let items = fields_val.as_sequence().ok_or_else(|| {
        SpecParseError("'fields' must be a sequence".into())
    })?;

    let mut fields = Vec::new();
    for item in items {
        let mapping = item.as_mapping().ok_or_else(|| {
            SpecParseError("Each field must be a key: descriptor mapping".into())
        })?;

        if mapping.is_empty() {
            continue;
        }

        let (field_name, descriptor_val) = &mapping[0];
        let descriptor = descriptor_val.as_scalar().unwrap_or("");
        fields.push(parse_field_descriptor(field_name, descriptor));
    }
    Ok(fields)
}

/// Parse a natural-language field descriptor like "required, string, 1-200 chars"
pub fn parse_field_descriptor(name: &str, descriptor: &str) -> FieldSpec {
    let parts: Vec<&str> = descriptor.split(',').map(|s| s.trim()).collect();
    let lower_parts: Vec<String> = parts.iter().map(|s| s.to_lowercase()).collect();

    let mut spec = FieldSpec {
        name: name.to_string(),
        required: false,
        field_type: FieldType::Unknown(String::new()),
        min_len: None,
        max_len: None,
        format: None,
        unique: false,
        auto_generated: false,
        immutable: false,
    };

    for (i, part) in lower_parts.iter().enumerate() {
        let part = part.as_str();
        if part == "required" {
            spec.required = true;
        } else if part == "string" {
            spec.field_type = FieldType::StringType;
        } else if part == "uuid" {
            spec.field_type = FieldType::UuidType;
        } else if part == "int" || part == "integer" {
            spec.field_type = FieldType::IntType;
        } else if part == "bool" || part == "boolean" {
            spec.field_type = FieldType::BoolType;
        } else if part == "unique" {
            spec.unique = true;
        } else if part == "auto-generated" {
            spec.auto_generated = true;
        } else if part == "immutable" {
            spec.immutable = true;
        } else if part.contains("email") && part.contains("format") {
            spec.format = Some("email".into());
        } else if part.contains("chars") || part.contains("len") {
            // Parse "1-200 chars" or "min 1 max 200"
            parse_length_constraint(parts[i], &mut spec);
        } else if !part.is_empty() {
            // Try to infer type from unrecognized part
            if spec.field_type == FieldType::Unknown(String::new()) {
                spec.field_type = FieldType::Unknown(parts[i].to_string());
            }
        }
    }

    // If we have auto-generated + immutable but no type, default to UUID
    if spec.auto_generated && spec.field_type == FieldType::Unknown(String::new()) {
        spec.field_type = FieldType::UuidType;
    }

    spec
}

fn parse_length_constraint(part: &str, spec: &mut FieldSpec) {
    // Match patterns like "1-200 chars", "max 200", "min 1"
    let nums: Vec<usize> = part
        .split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse().ok())
        .collect();

    if nums.len() == 2 {
        spec.min_len = Some(nums[0]);
        spec.max_len = Some(nums[1]);
    } else if nums.len() == 1 {
        if part.contains("min") {
            spec.min_len = Some(nums[0]);
        } else {
            spec.max_len = Some(nums[0]);
        }
    }
}

fn parse_endpoints(yaml: &YamlValue) -> Result<Vec<Endpoint>, SpecParseError> {
    let endpoints = match yaml.get("endpoints") {
        Some(e) => e,
        None => return Ok(Vec::new()),
    };

    let pairs = endpoints.as_mapping().ok_or_else(|| {
        SpecParseError("'endpoints' must be a mapping".into())
    })?;

    let mut result = Vec::new();
    for (ep_name, ep_val) in pairs {
        let description = get_scalar(ep_val, "description")
            .unwrap_or_default();

        let input_str = get_scalar(ep_val, "input")
            .unwrap_or_default();
        let input = parse_input_spec(&input_str);

        let outputs = parse_outputs(ep_val)?;
        let constraints = parse_constraints(ep_val);

        result.push(Endpoint {
            name: ep_name.clone(),
            description,
            input,
            outputs,
            constraints,
        });
    }
    Ok(result)
}

fn parse_input_spec(input_str: &str) -> InputSpec {
    let lower = input_str.to_lowercase();
    let source = if lower.contains("url") || lower.contains("path") {
        InputSource::Url
    } else if lower.contains("body") || lower.contains("json") || lower.contains("payload") {
        InputSource::Body
    } else {
        InputSource::Unknown
    };

    InputSpec {
        description: input_str.to_string(),
        source,
        fields: Vec::new(),
    }
}

fn parse_outputs(ep_val: &YamlValue) -> Result<Vec<OutputSpec>, SpecParseError> {
    let outputs_val = match ep_val.get("outputs") {
        Some(o) => o,
        None => return Ok(Vec::new()),
    };

    let items = outputs_val.as_sequence().ok_or_else(|| {
        SpecParseError("'outputs' must be a sequence".into())
    })?;

    let mut outputs = Vec::new();
    for item in items {
        let mapping = item.as_mapping().ok_or_else(|| {
            SpecParseError("Each output must be a key: descriptor mapping".into())
        })?;

        if mapping.is_empty() {
            continue;
        }

        let (label, desc_val) = &mapping[0];
        let description = desc_val.as_scalar().unwrap_or("").to_string();

        // Extract HTTP status from description like "the user found (200)"
        let http_status = extract_http_status(&description);

        let is_success = label.to_lowercase().contains("success")
            || label.to_lowercase().contains("ok")
            || label.to_lowercase().contains("created");

        outputs.push(OutputSpec {
            label: label.clone(),
            description,
            http_status,
            is_success,
        });
    }
    Ok(outputs)
}

fn extract_http_status(desc: &str) -> Option<u16> {
    // Look for (NNN) pattern at end of description
    if let Some(start) = desc.rfind('(') {
        if let Some(end) = desc.rfind(')') {
            if end > start {
                let num_str = &desc[start + 1..end];
                return num_str.trim().parse().ok();
            }
        }
    }
    None
}

fn parse_constraints(ep_val: &YamlValue) -> Vec<Constraint> {
    let constraints_val = match ep_val.get("constraints") {
        Some(c) => c,
        None => return Vec::new(),
    };

    let items = match constraints_val.as_sequence() {
        Some(s) => s,
        None => return Vec::new(),
    };

    let mut constraints = Vec::new();
    for item in items {
        match item {
            YamlValue::Scalar(s) => {
                let lower = s.to_lowercase();
                if lower == "read-only" || lower == "readonly" {
                    constraints.push(Constraint::ReadOnly);
                } else if lower == "write" || lower.contains("read-write") {
                    constraints.push(Constraint::Write);
                } else {
                    constraints.push(Constraint::Other(s.clone()));
                }
            }
            YamlValue::Mapping(pairs) => {
                for (k, v) in pairs {
                    let lower_k = k.to_lowercase();
                    if lower_k.contains("max response time") || lower_k.contains("latency") {
                        let val = v.as_scalar().unwrap_or("").to_string();
                        constraints.push(Constraint::MaxResponseTime(val));
                    } else if lower_k.contains("idempotent") {
                        let val = v.as_scalar().unwrap_or("").to_string();
                        constraints.push(Constraint::Idempotent(val));
                    } else if lower_k == "read-only" || lower_k == "readonly" {
                        constraints.push(Constraint::ReadOnly);
                    } else if lower_k == "write" {
                        constraints.push(Constraint::Write);
                    } else {
                        let val = v.as_scalar().unwrap_or("").to_string();
                        constraints.push(Constraint::Other(format!("{}: {}", k, val)));
                    }
                }
            }
            _ => {}
        }
    }
    constraints
}

fn parse_quality(yaml: &YamlValue) -> Vec<QualityRule> {
    let quality_val = match yaml.get("quality") {
        Some(q) => q,
        None => return Vec::new(),
    };

    let items = match quality_val.as_sequence() {
        Some(s) => s,
        None => return Vec::new(),
    };

    items
        .iter()
        .map(|item| {
            let s = item.as_scalar().unwrap_or("");
            let lower = s.to_lowercase();
            if lower.contains("total") && lower.contains("function") {
                QualityRule::AllFunctionsTotal
            } else {
                QualityRule::Other(s.to_string())
            }
        })
        .collect()
}

fn parse_traceability(yaml: &YamlValue) -> Traceability {
    let trace = match yaml.get("traceability") {
        Some(t) => t,
        None => {
            return Traceability {
                known_dependencies: Vec::new(),
            }
        }
    };

    let deps_str = get_scalar(trace, "known dependencies")
        .unwrap_or_default();

    let known_dependencies: Vec<String> = if deps_str.is_empty() {
        Vec::new()
    } else {
        deps_str
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    };

    Traceability { known_dependencies }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generate::yaml_parser::YamlParser;

    fn parse_yaml(input: &str) -> YamlValue {
        YamlParser::new(input).parse().unwrap()
    }

    #[test]
    fn test_parse_field_required_string_with_length() {
        let field = parse_field_descriptor("name", "required, string, 1-200 chars");
        assert!(field.required);
        assert_eq!(field.field_type, FieldType::StringType);
        assert_eq!(field.min_len, Some(1));
        assert_eq!(field.max_len, Some(200));
    }

    #[test]
    fn test_parse_field_email_format_unique() {
        let field = parse_field_descriptor("email", "required, email format, unique");
        assert!(field.required);
        assert_eq!(field.format, Some("email".into()));
        assert!(field.unique);
    }

    #[test]
    fn test_parse_field_auto_generated_immutable() {
        let field = parse_field_descriptor("id", "auto-generated, immutable");
        assert!(field.auto_generated);
        assert!(field.immutable);
        assert_eq!(field.field_type, FieldType::UuidType);
    }

    #[test]
    fn test_parse_minimal_spec() {
        let yaml = parse_yaml("\
spec: SPEC-001
title: \"Test\"
owner: test-team
");
        let spec = parse_spec(&yaml).unwrap();
        assert_eq!(spec.spec_id, "SPEC-001");
        assert_eq!(spec.title, "Test");
        assert_eq!(spec.owner, "test-team");
    }

    #[test]
    fn test_parse_domain_types() {
        let yaml = parse_yaml("\
spec: SPEC-001
title: test
owner: team
domain:
  User:
    fields:
      - name: required, string, 1-200 chars
      - id: auto-generated, immutable
");
        let spec = parse_spec(&yaml).unwrap();
        assert_eq!(spec.domain_types.len(), 1);
        assert_eq!(spec.domain_types[0].name, "User");
        assert_eq!(spec.domain_types[0].fields.len(), 2);
        assert_eq!(spec.domain_types[0].fields[0].name, "name");
        assert!(spec.domain_types[0].fields[0].required);
        assert!(spec.domain_types[0].fields[1].auto_generated);
    }

    #[test]
    fn test_parse_endpoint_with_outputs() {
        let yaml = parse_yaml("\
spec: SPEC-001
title: test
owner: team
endpoints:
  get-user:
    description: \"Returns a user by ID\"
    input: user id (from URL)
    outputs:
      - success: the user found (200)
      - not found: when the ID doesn't exist (404)
    constraints:
      - read-only
");
        let spec = parse_spec(&yaml).unwrap();
        assert_eq!(spec.endpoints.len(), 1);
        let ep = &spec.endpoints[0];
        assert_eq!(ep.name, "get-user");
        assert_eq!(ep.input.source, InputSource::Url);
        assert_eq!(ep.outputs.len(), 2);
        assert_eq!(ep.outputs[0].http_status, Some(200));
        assert!(ep.outputs[0].is_success);
        assert_eq!(ep.outputs[1].http_status, Some(404));
        assert!(!ep.outputs[1].is_success);
        assert_eq!(ep.constraints, vec![Constraint::ReadOnly]);
    }

    #[test]
    fn test_parse_max_response_time() {
        let yaml = parse_yaml("\
spec: SPEC-001
title: test
owner: team
endpoints:
  get-user:
    description: test
    input: id
    constraints:
      - max response time: 50ms
");
        let spec = parse_spec(&yaml).unwrap();
        let ep = &spec.endpoints[0];
        assert_eq!(ep.constraints, vec![Constraint::MaxResponseTime("50ms".into())]);
    }

    #[test]
    fn test_parse_quality_rules() {
        let yaml = parse_yaml("\
spec: SPEC-001
title: test
owner: team
quality:
  - all functions must be total
");
        let spec = parse_spec(&yaml).unwrap();
        assert_eq!(spec.quality, vec![QualityRule::AllFunctionsTotal]);
    }

    #[test]
    fn test_parse_traceability() {
        let yaml = parse_yaml("\
spec: SPEC-001
title: test
owner: team
traceability:
  known dependencies: api-router, admin-panel
");
        let spec = parse_spec(&yaml).unwrap();
        assert_eq!(spec.traceability.known_dependencies, vec!["api-router", "admin-panel"]);
    }

    #[test]
    fn test_extract_http_status_numbers() {
        assert_eq!(extract_http_status("the user found (200)"), Some(200));
        assert_eq!(extract_http_status("not found (404)"), Some(404));
        assert_eq!(extract_http_status("created (201)"), Some(201));
        assert_eq!(extract_http_status("no status"), None);
    }

    #[test]
    fn test_parse_input_source_detection() {
        let url_input = parse_input_spec("user id (from URL)");
        assert_eq!(url_input.source, InputSource::Url);

        let body_input = parse_input_spec("user data (from body)");
        assert_eq!(body_input.source, InputSource::Body);

        let unknown_input = parse_input_spec("something");
        assert_eq!(unknown_input.source, InputSource::Unknown);
    }
}

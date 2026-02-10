use super::spec_ast::*;

/// Emits a `.pct` file from a typed SpecDoc.
pub struct PctEmitter {
    output: String,
    indent: usize,
}

impl PctEmitter {
    pub fn new() -> Self {
        PctEmitter {
            output: String::new(),
            indent: 0,
        }
    }

    pub fn emit(mut self, spec: &SpecDoc) -> String {
        self.emit_module(spec);
        self.output
    }

    fn emit_module(&mut self, spec: &SpecDoc) {
        let module_name = self.derive_module_name(&spec.title);
        let store_name = self.derive_store_name(spec);

        self.write(&format!("(module {}", module_name));
        self.indent += 2;
        self.newline();

        // Provenance
        self.write(&format!(
            ":provenance {{req: \"{}\", author: \"agent:pact-generate\", created: \"{}\"}}",
            spec.spec_id,
            current_date()
        ));
        self.newline();

        // Version
        self.write(":version 1");
        self.newline();

        // Emit type definitions
        for dt in &spec.domain_types {
            self.emit_type_def(dt, &store_name);
            self.newline();
        }

        // Emit effect sets based on constraints
        self.emit_effect_sets(spec, &store_name);

        // Emit functions for endpoints
        let all_total = spec.quality.contains(&QualityRule::AllFunctionsTotal);
        for ep in &spec.endpoints {
            self.emit_endpoint_fn(ep, spec, &store_name, all_total);
        }

        // Close module
        self.indent -= 2;
        self.append(")");
        self.newline();
    }

    fn emit_type_def(&mut self, dt: &DomainType, store_name: &str) {
        self.newline();
        self.write(&format!("(type {}", dt.name));
        self.indent += 2;
        self.newline();

        // Emit invariants
        let invariants = self.build_invariants(&dt.fields);
        if !invariants.is_empty() {
            self.write(&format!(":invariants [{}]", invariants.join(" ")));
            self.newline();
        }

        // Emit fields
        for (i, field) in dt.fields.iter().enumerate() {
            self.emit_field(field, store_name);
            if i + 1 < dt.fields.len() {
                self.newline();
            }
        }

        self.indent -= 2;
        self.append(")");
    }

    fn emit_field(&mut self, field: &FieldSpec, store_name: &str) {
        let type_str = match &field.field_type {
            FieldType::StringType => "String",
            FieldType::UuidType => "UUID",
            FieldType::IntType => "Int",
            FieldType::BoolType => "Bool",
            FieldType::Unknown(s) if s.is_empty() => "String",
            FieldType::Unknown(s) => s.as_str(),
        };

        let mut parts = vec![format!("(field {} {}", field.name, type_str)];

        if field.immutable {
            parts.push(":immutable".into());
        }
        if field.auto_generated {
            parts.push(":generated".into());
        }
        if let Some(min) = field.min_len {
            parts.push(format!(":min-len {}", min));
        }
        if let Some(max) = field.max_len {
            parts.push(format!(":max-len {}", max));
        }
        if let Some(ref fmt) = field.format {
            parts.push(format!(":format :{}", fmt));
        }
        if field.unique {
            parts.push(format!(":unique-within {}", store_name));
        }

        let line = format!("{})", parts.join(" "));
        self.write(&line);
    }

    fn build_invariants(&self, fields: &[FieldSpec]) -> Vec<String> {
        let mut invariants = Vec::new();
        for field in fields {
            if let Some(min) = field.min_len {
                if min > 0 {
                    invariants.push(format!("(> (strlen {}) 0)", field.name));
                }
            }
            if let Some(ref fmt) = field.format {
                if fmt == "email" {
                    invariants.push(format!("(matches {} #/.+@.+\\..+/)", field.name));
                }
            }
        }
        invariants
    }

    fn emit_effect_sets(&mut self, spec: &SpecDoc, store_name: &str) {
        let mut has_read = false;
        let mut has_write = false;

        for ep in &spec.endpoints {
            for c in &ep.constraints {
                match c {
                    Constraint::ReadOnly => has_read = true,
                    Constraint::Write => has_write = true,
                    _ => {}
                }
            }
            // Infer from input source if no explicit constraint
            if ep.constraints.iter().all(|c| !matches!(c, Constraint::ReadOnly | Constraint::Write)) {
                if ep.input.source == InputSource::Url {
                    has_read = true;
                } else if ep.input.source == InputSource::Body {
                    has_write = true;
                }
            }
        }

        if has_read {
            self.newline();
            self.write(&format!("(effect-set db-read    [:reads  {}])", store_name));
        }
        if has_write {
            self.newline();
            self.write(&format!(
                "(effect-set db-write   [:writes {} :reads {}])",
                store_name, store_name
            ));
        }
        if has_read || has_write {
            self.newline();
            self.write("(effect-set http-respond [:sends http-response])");
        }
    }

    fn emit_endpoint_fn(
        &mut self,
        ep: &Endpoint,
        spec: &SpecDoc,
        store_name: &str,
        all_total: bool,
    ) {
        self.newline();
        self.newline();
        self.write(&format!("(fn {}", ep.name));
        self.indent += 2;
        self.newline();

        // Provenance
        self.write(&format!(
            ":provenance {{req: \"{}\"}}",
            spec.spec_id
        ));
        self.newline();

        // Effects
        let is_read_only = ep.constraints.contains(&Constraint::ReadOnly)
            || (ep.input.source == InputSource::Url
                && !ep.constraints.contains(&Constraint::Write));
        let is_write = ep.constraints.contains(&Constraint::Write)
            || ep.input.source == InputSource::Body;

        let effects = if is_write {
            "[db-write http-respond]"
        } else if is_read_only {
            "[db-read http-respond]"
        } else {
            "[http-respond]"
        };
        self.write(&format!(":effects    {}", effects));
        self.newline();

        // Total
        if all_total {
            self.write(":total      true");
            self.newline();
        }

        // Latency budget
        for c in &ep.constraints {
            if let Constraint::MaxResponseTime(t) = c {
                self.write(&format!(":latency-budget {}", t));
                self.newline();
            }
        }

        // Idempotency
        for c in &ep.constraints {
            if let Constraint::Idempotent(field) = c {
                self.write(&format!(":idempotency-key (hash (. input {}))", field));
                self.newline();
            }
        }

        // Called-by
        if !spec.traceability.known_dependencies.is_empty() {
            let deps: Vec<String> = spec
                .traceability
                .known_dependencies
                .iter()
                .map(|d| format!("{}/handle-request", d))
                .collect();
            self.write(&format!(":called-by  [{}]", deps.join(" ")));
            self.newline();
        }

        // Params
        let primary_type = spec.domain_types.first().map(|t| t.name.as_str()).unwrap_or("Entity");

        if is_read_only || ep.input.source == InputSource::Url {
            self.emit_url_param();
        } else {
            self.emit_body_param(primary_type, spec);
        }
        self.newline();

        // Returns
        self.emit_returns(ep, primary_type);
        self.newline();

        // Body
        self.newline();
        if is_read_only || ep.input.source == InputSource::Url {
            self.emit_read_body(store_name);
        } else {
            self.emit_write_body(store_name, primary_type, ep);
        }

        self.indent -= 2;
        self.append(")");
    }

    fn emit_url_param(&mut self) {
        self.write("(param id UUID");
        self.indent += 2;
        self.newline();
        self.write(":source http-path-param");
        self.newline();
        self.write(":validated-at boundary)");
        self.indent -= 2;
    }

    fn emit_body_param(&mut self, _type_name: &str, spec: &SpecDoc) {
        // Build input field map from the domain type
        let fields_str = if let Some(dt) = spec.domain_types.first() {
            let field_parts: Vec<String> = dt
                .fields
                .iter()
                .filter(|f| !f.auto_generated && !f.immutable)
                .map(|f| {
                    let t = match &f.field_type {
                        FieldType::StringType => "String",
                        FieldType::UuidType => "UUID",
                        FieldType::IntType => "Int",
                        FieldType::BoolType => "Bool",
                        FieldType::Unknown(s) if s.is_empty() => "String",
                        FieldType::Unknown(s) => s.as_str(),
                    };
                    format!(":{} {}", f.name, t)
                })
                .collect();
            format!("{{{}}}", field_parts.join(" "))
        } else {
            format!("{{{}}}", "")
        };

        self.write(&format!("(param input {}", fields_str));
        self.indent += 2;
        self.newline();
        self.write(":source http-body");
        self.newline();
        self.write(":content-type :json");
        self.newline();
        self.write(":validated-at boundary)");
        self.indent -= 2;
    }

    fn emit_returns(&mut self, ep: &Endpoint, primary_type: &str) {
        self.write("(returns (union");
        self.indent += 2;

        for output in &ep.outputs {
            self.newline();
            if output.is_success {
                let http = output
                    .http_status
                    .map(|s| format!(" :http {}", s))
                    .unwrap_or_default();
                self.write(&format!(
                    "(ok   {}{}  :serialize :json)",
                    primary_type, http
                ));
            } else {
                let tag = self.label_to_tag(&output.label);
                let http = output
                    .http_status
                    .map(|s| format!(" :http {}", s))
                    .unwrap_or_default();
                // Determine payload based on tag
                let payload = self.tag_to_payload(&tag);
                self.write(&format!("(err  :{} {}{})", tag, payload, http));
            }
        }

        self.indent -= 2;
        self.append("))");
    }

    fn emit_read_body(&mut self, store_name: &str) {
        self.write("(let [validated-id (validate-uuid id)]");
        self.indent += 2;
        self.newline();
        self.write("(match validated-id");
        self.indent += 2;
        self.newline();
        self.write("(err _)    (err :invalid-id {:id id})");
        self.newline();
        self.write(&format!(
            "(ok  uuid) (match (query {} {{:id uuid}})",
            store_name
        ));
        self.indent += 2;
        self.newline();
        self.write("(none)   (err :not-found {:id uuid})");
        self.newline();
        self.write("(some u) (ok u))))");
        self.indent -= 6;
    }

    fn emit_write_body(&mut self, store_name: &str, primary_type: &str, ep: &Endpoint) {
        self.write(&format!(
            "(let [errors (validate-against {} input)]",
            primary_type
        ));
        self.indent += 2;
        self.newline();
        self.write("(if (non-empty? errors)");
        self.indent += 2;
        self.newline();
        self.write("(err :validation-failed errors)");
        self.newline();

        // Find unique field for duplicate error
        let unique_field = self.find_unique_field(ep);

        self.write(&format!(
            "(match (insert! {} (build {} input))",
            store_name, primary_type
        ));
        self.indent += 2;
        self.newline();

        if let Some(ref field) = unique_field {
            self.write(&format!(
                "(err :unique-violation) (err :duplicate-{} {{:{} (. input {})}})",
                field, field, field
            ));
        } else {
            self.write("(err :unique-violation) (err :duplicate {:input input})");
        }
        self.newline();
        self.write("(ok entity)             (ok entity))))");
        self.indent -= 6;
    }

    fn find_unique_field(&self, ep: &Endpoint) -> Option<String> {
        // Look for a "duplicate" output to find the unique field name
        for output in &ep.outputs {
            let lower = output.label.to_lowercase();
            if lower.starts_with("duplicate") {
                // "duplicate email" → "email"
                let parts: Vec<&str> = output.label.splitn(2, ' ').collect();
                if parts.len() > 1 {
                    return Some(parts[1].to_lowercase().replace(' ', "-"));
                }
            }
        }
        None
    }

    fn derive_module_name(&self, title: &str) -> String {
        title
            .to_lowercase()
            .replace(' ', "-")
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '-')
            .collect()
    }

    fn derive_store_name(&self, spec: &SpecDoc) -> String {
        if let Some(dt) = spec.domain_types.first() {
            format!("{}-store", dt.name.to_lowercase())
        } else {
            "data-store".into()
        }
    }

    fn label_to_tag(&self, label: &str) -> String {
        label
            .to_lowercase()
            .replace(' ', "-")
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '-')
            .collect()
    }

    fn tag_to_payload(&self, tag: &str) -> String {
        if tag.contains("not-found") || tag.contains("invalid") {
            "{:id id}".into()
        } else if tag.contains("duplicate") {
            let field = tag.strip_prefix("duplicate-").unwrap_or("field");
            format!("{{:{f} (. input {f})}}", f = field)
        } else if tag.contains("validation") {
            "(list ValidationError)".into()
        } else {
            "{}".into()
        }
    }

    /// Write indented text. Only adds indent if we're at the start of a line.
    fn write(&mut self, s: &str) {
        if self.output.is_empty() || self.output.ends_with('\n') {
            for _ in 0..self.indent {
                self.output.push(' ');
            }
        }
        self.output.push_str(s);
    }

    /// Append text without indentation (for closing parens on same line, etc.).
    fn append(&mut self, s: &str) {
        self.output.push_str(s);
    }

    fn newline(&mut self) {
        self.output.push('\n');
    }
}

fn current_date() -> String {
    // Return a fixed format — in production this would use chrono or similar
    "2026-01-01T00:00:00Z".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_simple_spec() -> SpecDoc {
        SpecDoc {
            spec_id: "SPEC-001".into(),
            title: "User service".into(),
            owner: "platform-team".into(),
            domain_types: vec![DomainType {
                name: "User".into(),
                fields: vec![
                    FieldSpec {
                        name: "id".into(),
                        required: false,
                        field_type: FieldType::UuidType,
                        min_len: None,
                        max_len: None,
                        format: None,
                        unique: false,
                        auto_generated: true,
                        immutable: true,
                    },
                    FieldSpec {
                        name: "name".into(),
                        required: true,
                        field_type: FieldType::StringType,
                        min_len: Some(1),
                        max_len: Some(200),
                        format: None,
                        unique: false,
                        auto_generated: false,
                        immutable: false,
                    },
                    FieldSpec {
                        name: "email".into(),
                        required: true,
                        field_type: FieldType::StringType,
                        min_len: None,
                        max_len: None,
                        format: Some("email".into()),
                        unique: true,
                        auto_generated: false,
                        immutable: false,
                    },
                ],
            }],
            endpoints: vec![Endpoint {
                name: "get-user".into(),
                description: "Returns a user by ID".into(),
                input: InputSpec {
                    description: "user id (from URL)".into(),
                    source: InputSource::Url,
                    fields: Vec::new(),
                },
                outputs: vec![
                    OutputSpec {
                        label: "success".into(),
                        description: "the user found (200)".into(),
                        http_status: Some(200),
                        is_success: true,
                    },
                    OutputSpec {
                        label: "not found".into(),
                        description: "when the ID doesn't exist (404)".into(),
                        http_status: Some(404),
                        is_success: false,
                    },
                ],
                constraints: vec![
                    Constraint::MaxResponseTime("50ms".into()),
                    Constraint::ReadOnly,
                ],
            }],
            quality: vec![QualityRule::AllFunctionsTotal],
            traceability: Traceability {
                known_dependencies: vec!["api-router".into(), "admin-panel".into()],
            },
        }
    }

    #[test]
    fn test_emit_module_header() {
        let spec = make_simple_spec();
        let output = PctEmitter::new().emit(&spec);
        assert!(output.starts_with("(module user-service"));
        assert!(output.contains(":provenance {req: \"SPEC-001\""));
        assert!(output.contains(":version 1"));
    }

    #[test]
    fn test_emit_type_def() {
        let spec = make_simple_spec();
        let output = PctEmitter::new().emit(&spec);
        assert!(output.contains("(type User"));
        assert!(output.contains("(field id UUID :immutable :generated)"));
        assert!(output.contains("(field name String :min-len 1 :max-len 200)"));
        assert!(output.contains("(field email String :format :email :unique-within user-store)"));
    }

    #[test]
    fn test_emit_invariants() {
        let spec = make_simple_spec();
        let output = PctEmitter::new().emit(&spec);
        assert!(output.contains(":invariants [(> (strlen name) 0) (matches email #/.+@.+\\..+/)]"));
    }

    #[test]
    fn test_emit_effect_sets() {
        let spec = make_simple_spec();
        let output = PctEmitter::new().emit(&spec);
        assert!(output.contains("(effect-set db-read    [:reads  user-store])"));
        assert!(output.contains("(effect-set http-respond [:sends http-response])"));
    }

    #[test]
    fn test_emit_fn_metadata() {
        let spec = make_simple_spec();
        let output = PctEmitter::new().emit(&spec);
        assert!(output.contains("(fn get-user"));
        assert!(output.contains(":effects    [db-read http-respond]"));
        assert!(output.contains(":total      true"));
        assert!(output.contains(":latency-budget 50ms"));
        assert!(output.contains(":called-by  [api-router/handle-request admin-panel/handle-request]"));
    }

    #[test]
    fn test_emit_param_and_returns() {
        let spec = make_simple_spec();
        let output = PctEmitter::new().emit(&spec);
        assert!(output.contains("(param id UUID"));
        assert!(output.contains(":source http-path-param"));
        assert!(output.contains("(returns (union"));
        assert!(output.contains("(ok   User :http 200  :serialize :json)"));
        assert!(output.contains("(err  :not-found {:id id} :http 404)"));
    }

    #[test]
    fn test_emit_read_body() {
        let spec = make_simple_spec();
        let output = PctEmitter::new().emit(&spec);
        assert!(output.contains("(let [validated-id (validate-uuid id)]"));
        assert!(output.contains("(match (query user-store {:id uuid})"));
        assert!(output.contains("(none)   (err :not-found {:id uuid})"));
        assert!(output.contains("(some u) (ok u))))"));
    }

    #[test]
    fn test_emit_write_endpoint() {
        let mut spec = make_simple_spec();
        spec.endpoints.push(Endpoint {
            name: "create-user".into(),
            description: "Creates a new user".into(),
            input: InputSpec {
                description: "user data (from body)".into(),
                source: InputSource::Body,
                fields: Vec::new(),
            },
            outputs: vec![
                OutputSpec {
                    label: "created".into(),
                    description: "the new user (201)".into(),
                    http_status: Some(201),
                    is_success: true,
                },
                OutputSpec {
                    label: "duplicate email".into(),
                    description: "email already exists (409)".into(),
                    http_status: Some(409),
                    is_success: false,
                },
                OutputSpec {
                    label: "validation failed".into(),
                    description: "invalid input (422)".into(),
                    http_status: Some(422),
                    is_success: false,
                },
            ],
            constraints: vec![Constraint::Write],
        });

        let output = PctEmitter::new().emit(&spec);
        assert!(output.contains("(fn create-user"));
        assert!(output.contains(":effects    [db-write http-respond]"));
        assert!(output.contains("(param input {:name String :email String}"));
        assert!(output.contains("(let [errors (validate-against User input)]"));
        assert!(output.contains("(insert! user-store (build User input))"));
    }

    #[test]
    fn test_emit_module_closes() {
        let spec = make_simple_spec();
        let output = PctEmitter::new().emit(&spec);
        // Module should close with a final )
        let trimmed = output.trim_end();
        assert!(trimmed.ends_with(')'));
    }

    #[test]
    fn test_derive_module_name() {
        let emitter = PctEmitter::new();
        assert_eq!(emitter.derive_module_name("User service"), "user-service");
        assert_eq!(emitter.derive_module_name("Auth Service V2"), "auth-service-v2");
    }
}

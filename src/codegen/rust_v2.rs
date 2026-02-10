use crate::ast::*;

pub struct RustV2Codegen {
    output: String,
    indent: usize,
}

impl RustV2Codegen {
    pub fn new() -> Self {
        RustV2Codegen {
            output: String::new(),
            indent: 0,
        }
    }

    pub fn generate(mut self, module: &Module) -> String {
        self.emit_header(module);
        self.emit_line("");

        // Collect type info for resolving field types in error payloads
        let type_defs: Vec<&TypeDef> = module.types.iter().collect();

        // Generate types
        for typedef in &module.types {
            self.emit_type_def(typedef);
            self.emit_line("");
        }

        // Generate input structs for functions with Map-typed params
        for func in &module.functions {
            self.emit_input_structs(func);
        }

        // Generate return type enums for each function
        for func in &module.functions {
            self.emit_return_enum(func, &type_defs);
            self.emit_line("");
        }

        // Generate functions
        for func in &module.functions {
            self.emit_function(func, module, &type_defs);
            self.emit_line("");
        }

        self.output
    }

    fn emit_header(&mut self, module: &Module) {
        self.emit_line("// ============================================================");
        self.emit_line(&format!(
            "// Generated from Pact module: {}",
            module.name
        ));
        if let Some(v) = module.version {
            self.emit_line(&format!("// Version: {}", v));
        }
        if let Some(ref prov) = module.provenance {
            if let Some(ref req) = prov.req {
                self.emit_line(&format!("// Spec: {}", req));
            }
            if let Some(ref author) = prov.author {
                self.emit_line(&format!("// Author: {}", author));
            }
        }
        self.emit_line("// ============================================================");
        self.emit_line("");
        self.emit_line("use pact_runtime::prelude::*;");
        self.emit_line("use serde::{Serialize, Deserialize};");
        self.emit_line("use std::fmt;");
    }

    fn emit_type_def(&mut self, typedef: &TypeDef) {
        // Doc comment with invariants
        if !typedef.invariants.is_empty() {
            self.emit_line(&format!("/// Type: {}", typedef.name));
            self.emit_line("///");
            self.emit_line("/// Invariants:");
            for inv in &typedef.invariants {
                self.emit_line(&format!("/// - {}", inv.raw));
            }
        }

        self.emit_line("#[derive(Debug, Clone, Serialize, Deserialize)]");
        self.emit_line(&format!("pub struct {} {{", typedef.name));
        self.indent += 1;
        for field in &typedef.fields {
            let rust_type = type_expr_to_rust(&field.type_expr);
            self.emit_line(&format!("pub {}: {},", to_snake(&field.name), rust_type));
        }
        self.indent -= 1;
        self.emit_line("}");

        // Generate HasId impl
        let id_field = typedef.fields.iter().find(|f| f.name == "id");
        if id_field.is_some() {
            self.emit_line("");
            self.emit_line(&format!("impl HasId for {} {{", typedef.name));
            self.indent += 1;
            self.emit_line("fn id(&self) -> Uuid { self.id }");
            self.indent -= 1;
            self.emit_line("}");
        }

        // Generate HasUniqueFields impl
        let unique_fields: Vec<&FieldDef> = typedef
            .fields
            .iter()
            .filter(|f| f.unique_within.is_some())
            .collect();
        if id_field.is_some() {
            self.emit_line("");
            self.emit_line(&format!("impl HasUniqueFields for {} {{", typedef.name));
            self.indent += 1;
            self.emit_line("fn unique_fields(&self) -> Vec<(&'static str, String)> {");
            self.indent += 1;
            if unique_fields.is_empty() {
                self.emit_line("vec![]");
            } else {
                self.emit_indent();
                self.output.push_str("vec![");
                for (i, field) in unique_fields.iter().enumerate() {
                    if i > 0 {
                        self.output.push_str(", ");
                    }
                    self.output.push_str(&format!(
                        "(\"{}\", self.{}.clone())",
                        field.name,
                        to_snake(&field.name)
                    ));
                }
                self.output.push_str("]\n");
            }
            self.indent -= 1;
            self.emit_line("}");
            self.indent -= 1;
            self.emit_line("}");
        }

        // Generate validate method
        self.emit_line("");
        self.emit_line(&format!("impl {} {{", typedef.name));
        self.indent += 1;

        // validate() on instances
        self.emit_line("pub fn validate(&self) -> Vec<ValidationError> {");
        self.indent += 1;
        self.emit_line("let mut errors = Vec::new();");
        for field in &typedef.fields {
            let field_snake = to_snake(&field.name);
            if let Some(min) = field.min_len {
                self.emit_line(&format!(
                    "if self.{}.len() < {} {{ errors.push(ValidationError {{ field: \"{}\".into(), message: \"must be at least {} characters\".into() }}); }}",
                    field_snake, min, field.name, min
                ));
            }
            if let Some(max) = field.max_len {
                self.emit_line(&format!(
                    "if self.{}.len() > {} {{ errors.push(ValidationError {{ field: \"{}\".into(), message: \"must be at most {} characters\".into() }}); }}",
                    field_snake, max, field.name, max
                ));
            }
        }
        self.emit_line("errors");
        self.indent -= 1;
        self.emit_line("}");

        // validate_input() on input struct — checks same constraints as validate
        // but takes a generic input with matching field names
        let input_struct_name = format!("Create{}Input", typedef.name);
        let non_generated_fields: Vec<&FieldDef> = typedef
            .fields
            .iter()
            .filter(|f| !f.generated)
            .collect();
        if !non_generated_fields.is_empty() {
            self.emit_line("");
            self.emit_line(&format!(
                "pub fn validate_input(input: &{}) -> Vec<ValidationError> {{",
                input_struct_name
            ));
            self.indent += 1;
            self.emit_line("let mut errors = Vec::new();");
            for field in &non_generated_fields {
                let field_snake = to_snake(&field.name);
                if let Some(min) = field.min_len {
                    self.emit_line(&format!(
                        "if input.{}.len() < {} {{ errors.push(ValidationError {{ field: \"{}\".into(), message: \"must be at least {} characters\".into() }}); }}",
                        field_snake, min, field.name, min
                    ));
                }
                if let Some(max) = field.max_len {
                    self.emit_line(&format!(
                        "if input.{}.len() > {} {{ errors.push(ValidationError {{ field: \"{}\".into(), message: \"must be at most {} characters\".into() }}); }}",
                        field_snake, max, field.name, max
                    ));
                }
            }
            self.emit_line("errors");
            self.indent -= 1;
            self.emit_line("}");
        }

        // from_input() constructor
        self.emit_line("");
        self.emit_line(&format!(
            "pub fn from_input(input: {}) -> Self {{",
            input_struct_name
        ));
        self.indent += 1;
        self.emit_line(&format!("{} {{", typedef.name));
        self.indent += 1;
        for field in &typedef.fields {
            let field_snake = to_snake(&field.name);
            if field.generated {
                match type_expr_to_rust(&field.type_expr).as_str() {
                    "Uuid" => self.emit_line(&format!("{}: Uuid::new_v4(),", field_snake)),
                    _ => self.emit_line(&format!("{}: Default::default(),", field_snake)),
                }
            } else {
                self.emit_line(&format!("{}: input.{},", field_snake, field_snake));
            }
        }
        self.indent -= 1;
        self.emit_line("}");
        self.indent -= 1;
        self.emit_line("}");

        self.indent -= 1;
        self.emit_line("}");
    }

    fn emit_input_structs(&mut self, func: &FnDef) {
        for param in &func.params {
            if let TypeExpr::Map(fields) = &param.type_expr {
                let struct_name = format!("{}Input", to_pascal(&func.name));
                self.emit_line("#[derive(Debug, Clone, Serialize, Deserialize)]");
                self.emit_line(&format!("pub struct {} {{", struct_name));
                self.indent += 1;
                for (name, type_expr) in fields {
                    let rust_type = type_expr_to_rust(type_expr);
                    self.emit_line(&format!("pub {}: {},", to_snake(name), rust_type));
                }
                self.indent -= 1;
                self.emit_line("}");
                self.emit_line("");
            }
        }
    }

    fn emit_return_enum(&mut self, func: &FnDef, type_defs: &[&TypeDef]) {
        let enum_name = format!("{}Result", to_pascal(&func.name));

        if func.total {
            self.emit_line("/// Total: this function handles all cases exhaustively");
        }

        self.emit_line("#[derive(Debug)]");
        self.emit_line(&format!("pub enum {} {{", enum_name));
        self.indent += 1;

        for variant in &func.returns.variants {
            match &variant.kind {
                VariantKind::Ok {
                    type_expr,
                    http_status,
                    ..
                } => {
                    if let Some(status) = http_status {
                        self.emit_line(&format!("/// HTTP {}", status));
                    }
                    self.emit_line(&format!("Ok({}),", type_expr_to_rust(type_expr)));
                }
                VariantKind::Err {
                    tag,
                    payload,
                    http_status,
                    ..
                } => {
                    if let Some(status) = http_status {
                        self.emit_line(&format!("/// HTTP {}", status));
                    }
                    let variant_name = to_pascal(tag);
                    match payload {
                        TypeExpr::Map(fields) => {
                            // Named fields in enum variant
                            self.emit_indent();
                            self.output.push_str(&format!("{} {{", variant_name));
                            for (i, (name, type_expr)) in fields.iter().enumerate() {
                                if i > 0 {
                                    self.output.push_str(",");
                                }
                                self.output.push_str(&format!(
                                    " {}: {}",
                                    to_snake(name),
                                    resolve_field_type(name, type_expr, type_defs, func)
                                ));
                            }
                            self.output.push_str(" },\n");
                        }
                        TypeExpr::Named(name) if name == "Unit" => {
                            self.emit_line(&format!("{},", variant_name));
                        }
                        _ => {
                            let payload_type = type_expr_to_rust(payload);
                            self.emit_line(&format!("{}({}),", variant_name, payload_type));
                        }
                    }
                }
            }
        }

        self.indent -= 1;
        self.emit_line("}");

        // Generate http_status() method
        self.emit_line("");
        self.emit_line(&format!("impl {} {{", enum_name));
        self.indent += 1;
        self.emit_line("pub fn http_status(&self) -> u16 {");
        self.indent += 1;
        self.emit_line("match self {");
        self.indent += 1;

        for variant in &func.returns.variants {
            match &variant.kind {
                VariantKind::Ok { http_status, .. } => {
                    let status = http_status.unwrap_or(200);
                    self.emit_line(&format!("{}::Ok(_) => {},", enum_name, status));
                }
                VariantKind::Err {
                    tag,
                    payload,
                    http_status,
                    ..
                } => {
                    let status = http_status.unwrap_or(500);
                    let variant_name = to_pascal(tag);
                    match payload {
                        TypeExpr::Map(_) => {
                            self.emit_line(&format!(
                                "{}::{} {{ .. }} => {},",
                                enum_name, variant_name, status
                            ));
                        }
                        TypeExpr::Named(name) if name == "Unit" => {
                            self.emit_line(&format!(
                                "{}::{} => {},",
                                enum_name, variant_name, status
                            ));
                        }
                        _ => {
                            self.emit_line(&format!(
                                "{}::{}(_) => {},",
                                enum_name, variant_name, status
                            ));
                        }
                    }
                }
            }
        }

        self.indent -= 1;
        self.emit_line("}");
        self.indent -= 1;
        self.emit_line("}");
        self.indent -= 1;
        self.emit_line("}");

        // Generate Display impl
        self.emit_line("");
        self.emit_line(&format!("impl fmt::Display for {} {{", enum_name));
        self.indent += 1;
        self.emit_line("fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {");
        self.indent += 1;
        self.emit_line("match self {");
        self.indent += 1;

        for variant in &func.returns.variants {
            match &variant.kind {
                VariantKind::Ok { .. } => {
                    self.emit_line(&format!(
                        "{}::Ok(v) => write!(f, \"Ok: {{:?}}\", v),",
                        enum_name
                    ));
                }
                VariantKind::Err { tag, payload, .. } => {
                    let variant_name = to_pascal(tag);
                    match payload {
                        TypeExpr::Map(_) => {
                            self.emit_line(&format!(
                                "{}::{} {{ .. }} => write!(f, \"Error: {}\"),",
                                enum_name, variant_name, tag
                            ));
                        }
                        TypeExpr::Named(name) if name == "Unit" => {
                            self.emit_line(&format!(
                                "{}::{} => write!(f, \"Error: {}\"),",
                                enum_name, variant_name, tag
                            ));
                        }
                        _ => {
                            self.emit_line(&format!(
                                "{}::{}(v) => write!(f, \"Error({}): {{:?}}\", v),",
                                enum_name, variant_name, tag
                            ));
                        }
                    }
                }
            }
        }

        self.indent -= 1;
        self.emit_line("}");
        self.indent -= 1;
        self.emit_line("}");
        self.indent -= 1;
        self.emit_line("}");
    }

    fn emit_function(&mut self, func: &FnDef, module: &Module, type_defs: &[&TypeDef]) {
        let fn_name = to_snake(&func.name);
        let return_type = format!("{}Result", to_pascal(&func.name));

        // Determine which store types are needed from effects
        let store_types = collect_store_types(func, module);

        // Build parameter list
        let mut params = Vec::new();
        for (store_type, needs_mut) in &store_types {
            if *needs_mut {
                params.push(format!("store: &mut impl Store<{}>", store_type));
            } else {
                params.push(format!("store: &impl Store<{}>", store_type));
            }
        }
        for param in &func.params {
            match &param.type_expr {
                TypeExpr::Map(_) => {
                    let struct_name = format!("{}Input", to_pascal(&func.name));
                    params.push(format!("{}: {}", to_snake(&param.name), struct_name));
                }
                TypeExpr::Named(name) if name == "UUID" => {
                    // Accept as &str for UUID params to match validate_uuid pattern
                    params.push(format!("{}: &str", to_snake(&param.name)));
                }
                _ => {
                    let rust_type = type_expr_to_rust(&param.type_expr);
                    params.push(format!("{}: {}", to_snake(&param.name), rust_type));
                }
            }
        }

        // Doc comments
        if let Some(ref prov) = func.provenance {
            if let Some(ref req) = prov.req {
                self.emit_line(&format!("/// Spec: {}", req));
            }
        }
        if func.total {
            self.emit_line("/// Total: handles all cases exhaustively");
        }

        self.emit_line(&format!(
            "pub fn {}({}) -> {} {{",
            fn_name,
            params.join(", "),
            return_type
        ));
        self.indent += 1;

        let ctx = EmitCtx {
            return_type: &return_type,
            func,
            module,
            type_defs,
        };
        self.emit_expr(&func.body, &ctx);

        self.indent -= 1;
        self.emit_line("}");
    }

    fn emit_expr(&mut self, expr: &Expr, ctx: &EmitCtx) {
        match expr {
            Expr::Let { bindings, body, .. } => {
                for (name, value) in bindings {
                    self.emit_indent();
                    self.output.push_str(&format!("let {} = ", to_snake(name)));
                    self.emit_expr_inline(value, ctx);
                    self.output.push_str(";\n");
                }
                self.emit_expr(body, ctx);
            }
            Expr::Match { expr, arms, .. } => {
                self.emit_indent();
                self.output.push_str("match ");
                self.emit_expr_inline(expr, ctx);
                self.output.push_str(" {\n");
                self.indent += 1;
                for arm in arms {
                    self.emit_indent();
                    self.emit_pattern(&arm.pattern, ctx);
                    self.output.push_str(" => ");
                    self.emit_expr_inline(&arm.body, ctx);
                    self.output.push_str(",\n");
                }
                // Add catch-all arm if any pattern matches on Err with a keyword (StoreError)
                if needs_err_catchall(arms) {
                    self.emit_indent();
                    self.output.push_str("Err(e) => panic!(\"unexpected store error: {:?}\", e),\n");
                }
                self.indent -= 1;
                self.emit_indent();
                self.output.push_str("}\n");
            }
            Expr::If {
                cond,
                then_branch,
                else_branch,
                ..
            } => {
                self.emit_indent();
                self.output.push_str("if ");
                self.emit_expr_inline(cond, ctx);
                self.output.push_str(" {\n");
                self.indent += 1;
                self.emit_expr(then_branch, ctx);
                self.indent -= 1;
                self.emit_indent();
                self.output.push_str("} else {\n");
                self.indent += 1;
                self.emit_expr(else_branch, ctx);
                self.indent -= 1;
                self.emit_indent();
                self.output.push_str("}\n");
            }
            _ => {
                self.emit_indent();
                self.emit_expr_inline(expr, ctx);
                self.output.push('\n');
            }
        }
    }

    fn emit_expr_inline(&mut self, expr: &Expr, ctx: &EmitCtx) {
        match expr {
            Expr::Ref(name, _) => {
                self.output.push_str(&to_snake(name));
            }
            Expr::Keyword(kw, _) => {
                self.output.push_str(&format!("\"{}\"", kw));
            }
            Expr::StringLit(s, _) => {
                self.output.push_str(&format!("\"{}\"", s));
            }
            Expr::IntLit(n, _) => {
                self.output.push_str(&n.to_string());
            }
            Expr::BoolLit(b, _) => {
                self.output.push_str(&b.to_string());
            }
            Expr::Ok(inner, _) => {
                self.output
                    .push_str(&format!("{}::Ok(", ctx.return_type));
                self.emit_expr_inline(inner, ctx);
                self.output.push(')');
            }
            Expr::Err { tag, payload, .. } => {
                let variant = to_pascal(tag);
                // Check if this error variant uses named fields (Map payload in return type)
                let variant_info = ctx.func.returns.variants.iter().find(|v| {
                    matches!(&v.kind, VariantKind::Err { tag: t, .. } if t == tag)
                });
                let uses_named_fields = variant_info.map_or(false, |v| {
                    matches!(&v.kind, VariantKind::Err { payload: TypeExpr::Map(_), .. })
                });

                if uses_named_fields {
                    // Get the variant's field types from the return type definition
                    let variant_fields: Vec<(String, String)> = variant_info
                        .and_then(|v| match &v.kind {
                            VariantKind::Err { payload: TypeExpr::Map(fields), .. } => {
                                Some(fields.iter().map(|(n, t)| {
                                    (n.clone(), resolve_field_type(n, t, ctx.type_defs, ctx.func))
                                }).collect())
                            }
                            _ => None,
                        })
                        .unwrap_or_default();

                    self.output
                        .push_str(&format!("{}::{} {{ ", ctx.return_type, variant));
                    if let Expr::MapLit(entries, _) = payload.as_ref() {
                        for (i, (key, val)) in entries.iter().enumerate() {
                            if i > 0 {
                                self.output.push_str(", ");
                            }
                            self.output.push_str(&format!("{}: ", to_snake(key)));
                            self.emit_expr_inline(val, ctx);

                            // When the target field type is String, add conversions
                            let field_type = variant_fields.iter()
                                .find(|(n, _)| n == key)
                                .map(|(_, t)| t.as_str());
                            if let Some("String") = field_type {
                                match val {
                                    Expr::Ref(ref_name, _) => {
                                        // Check if this ref is a UUID param (passed as &str)
                                        let is_str_param = ctx.func.params.iter().any(|p| {
                                            p.name == *ref_name &&
                                            matches!(&p.type_expr, TypeExpr::Named(n) if n == "UUID")
                                        });
                                        if is_str_param {
                                            self.output.push_str(".to_string()");
                                        } else {
                                            // Other refs may be Uuid or other types — add .to_string()
                                            self.output.push_str(".to_string()");
                                        }
                                    }
                                    Expr::FieldAccess { .. } => {
                                        // Field access on an input param — add .clone()
                                        self.output.push_str(".clone()");
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                    self.output.push_str(" }");
                } else {
                    self.output
                        .push_str(&format!("{}::{}(", ctx.return_type, variant));
                    self.emit_expr_inline(payload, ctx);
                    self.output.push(')');
                }
            }
            Expr::Call { name, args, .. } => {
                self.emit_call(name, args, ctx);
            }
            Expr::FieldAccess { expr, field, .. } => {
                self.emit_expr_inline(expr, ctx);
                self.output.push('.');
                self.output.push_str(&to_snake(field));
            }
            Expr::MapLit(entries, _) => {
                self.output.push_str("{ ");
                for (i, (key, val)) in entries.iter().enumerate() {
                    if i > 0 {
                        self.output.push_str(", ");
                    }
                    self.output.push_str(&to_snake(key));
                    self.output.push_str(": ");
                    self.emit_expr_inline(val, ctx);
                }
                self.output.push_str(" }");
            }
            Expr::Wildcard(_) => {
                self.output.push('_');
            }
            Expr::Let { bindings, body, .. } => {
                self.output.push_str("{\n");
                self.indent += 1;
                for (name, value) in bindings {
                    self.emit_indent();
                    self.output
                        .push_str(&format!("let {} = ", to_snake(name)));
                    self.emit_expr_inline(value, ctx);
                    self.output.push_str(";\n");
                }
                self.emit_indent();
                self.emit_expr_inline(body, ctx);
                self.output.push('\n');
                self.indent -= 1;
                self.emit_indent();
                self.output.push('}');
            }
            Expr::Match { expr, arms, .. } => {
                self.output.push_str("match ");
                self.emit_expr_inline(expr, ctx);
                self.output.push_str(" {\n");
                self.indent += 1;
                for arm in arms {
                    self.emit_indent();
                    self.emit_pattern(&arm.pattern, ctx);
                    self.output.push_str(" => ");
                    self.emit_expr_inline(&arm.body, ctx);
                    self.output.push_str(",\n");
                }
                if needs_err_catchall(arms) {
                    self.emit_indent();
                    self.output.push_str("Err(e) => panic!(\"unexpected store error: {:?}\", e),\n");
                }
                self.indent -= 1;
                self.emit_indent();
                self.output.push('}');
            }
            Expr::If {
                cond,
                then_branch,
                else_branch,
                ..
            } => {
                self.output.push_str("if ");
                self.emit_expr_inline(cond, ctx);
                self.output.push_str(" { ");
                self.emit_expr_inline(then_branch, ctx);
                self.output.push_str(" } else { ");
                self.emit_expr_inline(else_branch, ctx);
                self.output.push_str(" }");
            }
        }
    }

    fn emit_call(&mut self, name: &str, args: &[Expr], ctx: &EmitCtx) {
        let clean_name = name.replace('?', "").replace('!', "");

        match clean_name.as_str() {
            "validate-uuid" | "validate_uuid" => {
                // validate_uuid(id) → validate_uuid(id)
                // The param is already &str in the function signature
                self.output.push_str("validate_uuid(");
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        self.output.push_str(", ");
                    }
                    self.emit_expr_inline(arg, ctx);
                }
                self.output.push(')');
            }
            "query" => {
                // query(user-store, {id: uuid}) → store.query_by_id(&uuid)
                // args[0] = store ref, args[1] = map with query fields
                self.output.push_str("store.query_by_id(");
                if args.len() > 1 {
                    if let Expr::MapLit(entries, _) = &args[1] {
                        // Extract the ID value from the map
                        if let Some((_, val)) = entries.iter().find(|(k, _)| k == "id") {
                            self.output.push('&');
                            self.emit_expr_inline(val, ctx);
                        }
                    }
                }
                self.output.push(')');
            }
            "insert" => {
                // insert!(user-store, build(User, input)) → store.insert(...)
                self.output.push_str("store.insert(");
                if args.len() > 1 {
                    self.emit_expr_inline(&args[1], ctx);
                }
                self.output.push(')');
            }
            "build" => {
                // build(User, input) → User::from_input(input.clone())
                // We clone because the input may be referenced in error branches
                if let Some(Expr::Ref(type_name, _)) = args.first() {
                    self.output.push_str(&format!("{}::from_input(", type_name));
                    if args.len() > 1 {
                        self.emit_expr_inline(&args[1], ctx);
                        self.output.push_str(".clone()");
                    }
                    self.output.push(')');
                } else {
                    // Fallback
                    self.output.push_str("build(");
                    for (i, arg) in args.iter().enumerate() {
                        if i > 0 {
                            self.output.push_str(", ");
                        }
                        self.emit_expr_inline(arg, ctx);
                    }
                    self.output.push(')');
                }
            }
            "validate-against" | "validate_against" => {
                // validate-against(User, input) → User::validate_input(&input)
                if let Some(Expr::Ref(type_name, _)) = args.first() {
                    self.output
                        .push_str(&format!("{}::validate_input(&", type_name));
                    if args.len() > 1 {
                        self.emit_expr_inline(&args[1], ctx);
                    }
                    self.output.push(')');
                } else {
                    self.output.push_str(&to_snake(&clean_name));
                    self.output.push('(');
                    for (i, arg) in args.iter().enumerate() {
                        if i > 0 {
                            self.output.push_str(", ");
                        }
                        self.emit_expr_inline(arg, ctx);
                    }
                    self.output.push(')');
                }
            }
            "non-empty" | "non_empty" => {
                // non-empty?(errors) → non_empty(&errors)
                self.output.push_str("non_empty(&");
                if let Some(arg) = args.first() {
                    self.emit_expr_inline(arg, ctx);
                }
                self.output.push(')');
            }
            _ => {
                // Generic function call
                self.output.push_str(&to_snake(&clean_name));
                self.output.push('(');
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        self.output.push_str(", ");
                    }
                    self.emit_expr_inline(arg, ctx);
                }
                self.output.push(')');
            }
        }
    }

    fn emit_pattern(&mut self, pattern: &Pattern, ctx: &EmitCtx) {
        match pattern {
            Pattern::Wildcard(_) => self.output.push('_'),
            Pattern::Var(name, _) => self.output.push_str(&to_snake(name)),
            Pattern::Constructor { name, args, .. } => {
                match name.as_str() {
                    "ok" => {
                        self.output.push_str("Ok(");
                        if args.is_empty() {
                            self.output.push('_');
                        } else {
                            for (i, arg) in args.iter().enumerate() {
                                if i > 0 {
                                    self.output.push_str(", ");
                                }
                                self.emit_pattern(arg, ctx);
                            }
                        }
                        self.output.push(')');
                    }
                    "some" => {
                        self.output.push_str("Some(");
                        if args.is_empty() {
                            self.output.push('_');
                        } else {
                            for (i, arg) in args.iter().enumerate() {
                                if i > 0 {
                                    self.output.push_str(", ");
                                }
                                self.emit_pattern(arg, ctx);
                            }
                        }
                        self.output.push(')');
                    }
                    "err" => {
                        self.output.push_str("Err(");
                        if args.is_empty() {
                            self.output.push('_');
                        } else {
                            // Check if first arg is a keyword — map to StoreError variant
                            if let Some(Pattern::Keyword(kw, _)) = args.first() {
                                let variant = to_pascal(kw);
                                self.output
                                    .push_str(&format!("StoreError::{} {{ .. }}", variant));
                            } else {
                                for (i, arg) in args.iter().enumerate() {
                                    if i > 0 {
                                        self.output.push_str(", ");
                                    }
                                    self.emit_pattern(arg, ctx);
                                }
                            }
                        }
                        self.output.push(')');
                    }
                    "none" => {
                        self.output.push_str("None");
                    }
                    _ => {
                        self.output.push_str(&to_pascal(name));
                        if !args.is_empty() {
                            self.output.push('(');
                            for (i, arg) in args.iter().enumerate() {
                                if i > 0 {
                                    self.output.push_str(", ");
                                }
                                self.emit_pattern(arg, ctx);
                            }
                            self.output.push(')');
                        }
                    }
                }
            }
            Pattern::Keyword(kw, _) => {
                self.output.push_str(&format!("\"{}\"", kw));
            }
        }
    }

    fn emit_line(&mut self, line: &str) {
        if line.is_empty() {
            self.output.push('\n');
        } else {
            self.emit_indent();
            self.output.push_str(line);
            self.output.push('\n');
        }
    }

    fn emit_indent(&mut self) {
        for _ in 0..self.indent {
            self.output.push_str("    ");
        }
    }
}

#[allow(dead_code)]
struct EmitCtx<'a> {
    return_type: &'a str,
    func: &'a FnDef,
    module: &'a Module,
    type_defs: &'a [&'a TypeDef],
}

/// Check if a match expression has Err patterns with keyword arguments (StoreError variants)
/// and thus needs a catch-all arm for exhaustiveness.
fn needs_err_catchall(arms: &[MatchArm]) -> bool {
    arms.iter().any(|arm| {
        matches!(
            &arm.pattern,
            Pattern::Constructor { name, args, .. }
            if name == "err" && args.iter().any(|a| matches!(a, Pattern::Keyword(_, _)))
        )
    })
}

/// Collect store types needed by a function based on its effects.
/// Returns Vec<(TypeName, needs_mut)>
fn collect_store_types(func: &FnDef, module: &Module) -> Vec<(String, bool)> {
    let mut stores: Vec<(String, bool)> = Vec::new();
    for effect_name in &func.effects {
        if let Some(effect_set) = module.effect_sets.iter().find(|es| &es.name == effect_name) {
            for effect in &effect_set.effects {
                // Skip Sends effects — they don't map to stores
                if matches!(effect.kind, EffectKind::Sends) {
                    continue;
                }

                // Convert store name like "user-store" to type name "User"
                let type_name = store_target_to_type(&effect.target);
                let needs_mut = matches!(effect.kind, EffectKind::Writes);

                // Check if we already have this store type
                if let Some(existing) = stores.iter_mut().find(|(t, _)| t == &type_name) {
                    // Upgrade to mut if any effect needs writes
                    if needs_mut {
                        existing.1 = true;
                    }
                } else {
                    stores.push((type_name, needs_mut));
                }
            }
        }
    }
    stores
}

/// Convert a store target name like "user-store" to a type name like "User"
fn store_target_to_type(target: &str) -> String {
    let name = target
        .strip_suffix("-store")
        .or_else(|| target.strip_suffix("_store"))
        .unwrap_or(target);
    to_pascal(name)
}

fn type_expr_to_rust(type_expr: &TypeExpr) -> String {
    match type_expr {
        TypeExpr::Named(name) => match name.as_str() {
            "UUID" => "Uuid".to_string(),
            "String" => "String".to_string(),
            "Int" => "i64".to_string(),
            "Bool" => "bool".to_string(),
            "Unit" => "()".to_string(),
            "ValidationError" => "ValidationError".to_string(),
            other => other.to_string(),
        },
        TypeExpr::Map(fields) => {
            let field_strs: Vec<String> = fields
                .iter()
                .map(|(name, typ)| format!("{}: {}", to_snake(name), type_expr_to_rust(typ)))
                .collect();
            format!("{{ {} }}", field_strs.join(", "))
        }
        TypeExpr::List(inner) => {
            format!("Vec<{}>", type_expr_to_rust(inner))
        }
        TypeExpr::Union(_variants) => "UnionType".to_string(),
        TypeExpr::Enum(variants) => {
            format!("/* enum: {} */", variants.join(" | "))
        }
    }
}

/// Resolve the type of a field in an error payload for enum variant definitions.
/// For payloads like {:id id} where `id` refers to a variable,
/// we need to figure out the actual Rust type.
///
/// Strategy: check params first (UUID params become &str → use String for the field),
/// then fall back to domain type fields.
fn resolve_field_type(
    field_name: &str,
    type_expr: &TypeExpr,
    type_defs: &[&TypeDef],
    func: &FnDef,
) -> String {
    if let TypeExpr::Named(name) = type_expr {
        match name.as_str() {
            "UUID" => return "Uuid".to_string(),
            "String" => return "String".to_string(),
            "Int" => return "i64".to_string(),
            "Bool" => return "bool".to_string(),
            _ => {}
        }

        // The payload value name (e.g., "id" in {:id id}) may reference a variable.
        // Check function params first — UUID params are passed as &str in v2,
        // so the error field should be String to hold the raw input.
        for param in &func.params {
            if param.name == *name {
                if matches!(&param.type_expr, TypeExpr::Named(n) if n == "UUID") {
                    return "String".to_string();
                }
                return type_expr_to_rust(&param.type_expr);
            }
        }

        // Fall back to domain type fields
        for typedef in type_defs {
            for field in &typedef.fields {
                if field.name == *field_name {
                    return type_expr_to_rust(&field.type_expr);
                }
            }
        }
    }

    type_expr_to_rust(type_expr)
}

/// Convert a kebab-case name to snake_case, stripping ? and ! suffixes
fn to_snake(name: &str) -> String {
    name.replace('-', "_")
        .replace('/', "_")
        .replace('?', "")
        .replace('!', "")
}

/// Convert a kebab-case name to PascalCase
fn to_pascal(name: &str) -> String {
    name.split(|c| c == '-' || c == '_' || c == '/')
        .map(|part| {
            let part = part.replace('?', "").replace('!', "");
            let mut chars = part.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => {
                    let upper = c.to_uppercase().to_string();
                    upper + &chars.as_str().to_lowercase()
                }
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::lower::Lowerer;
    use crate::parser::Parser;

    fn generate(input: &str) -> String {
        let mut lexer = Lexer::new(input);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = Parser::new(tokens);
        let sexprs = parser.parse_program().unwrap();
        let mut lowerer = Lowerer::new();
        let module = lowerer.lower_module(&sexprs[0]).unwrap();
        RustV2Codegen::new().generate(&module)
    }

    #[test]
    fn test_header_uses_pact_runtime() {
        let output = generate("(module test :version 1)");
        assert!(output.contains("use pact_runtime::prelude::*;"));
        assert!(output.contains("use serde::{Serialize, Deserialize};"));
        assert!(output.contains("use std::fmt;"));
    }

    #[test]
    fn test_struct_has_serde_derive() {
        let output = generate(
            "(module test :version 1 (type User (field id UUID :immutable :generated) (field name String :min-len 1 :max-len 200)))",
        );
        assert!(output.contains("#[derive(Debug, Clone, Serialize, Deserialize)]"));
        assert!(output.contains("pub struct User {"));
        assert!(output.contains("pub id: Uuid,"));
        assert!(output.contains("pub name: String,"));
    }

    #[test]
    fn test_generates_has_id_impl() {
        let output = generate(
            "(module test :version 1 (type User (field id UUID :immutable :generated) (field name String)))",
        );
        assert!(output.contains("impl HasId for User {"));
        assert!(output.contains("fn id(&self) -> Uuid { self.id }"));
    }

    #[test]
    fn test_generates_from_input() {
        let output = generate(
            "(module test :version 1 (type User (field id UUID :immutable :generated) (field name String)))",
        );
        assert!(output.contains("pub fn from_input(input: CreateUserInput) -> Self {"));
        assert!(output.contains("id: Uuid::new_v4(),"));
        assert!(output.contains("name: input.name,"));
    }

    #[test]
    fn test_generates_validate() {
        let output = generate(
            "(module test :version 1 (type User (field id UUID :immutable :generated) (field name String :min-len 1 :max-len 200)))",
        );
        assert!(output.contains("pub fn validate(&self) -> Vec<ValidationError>"));
        assert!(output.contains("must be at least 1 characters"));
        assert!(output.contains("must be at most 200 characters"));
    }

    #[test]
    fn test_generates_input_struct_for_map_param() {
        let input = r#"(module test :version 1
            (fn create-thing
                :effects []
                :total true
                (param input {:name String :email String})
                (returns (union (ok Thing :http 201)))
                (ok input)))"#;
        let output = generate(input);
        assert!(output.contains("pub struct CreateThingInput {"));
        assert!(output.contains("pub name: String,"));
        assert!(output.contains("pub email: String,"));
    }

    #[test]
    fn test_enum_named_fields_for_map_payload() {
        let input = r#"(module test :version 1
            (type User (field id UUID :immutable :generated) (field name String))
            (fn get-thing
                :effects []
                :total true
                (param id UUID)
                (returns (union
                    (ok User :http 200)
                    (err :not-found {:id id} :http 404)))
                (ok id)))"#;
        let output = generate(input);
        assert!(output.contains("NotFound { id: String }"));
    }

    #[test]
    fn test_to_snake_strips_suffixes() {
        assert_eq!(to_snake("non-empty?"), "non_empty");
        assert_eq!(to_snake("insert!"), "insert");
        assert_eq!(to_snake("get-user-by-id"), "get_user_by_id");
    }

    #[test]
    fn test_store_trait_bound_in_function() {
        let input = r#"(module test :version 1
            (type User (field id UUID :immutable :generated) (field name String))
            (effect-set db-read [:reads user-store])
            (fn get-user
                :effects [db-read]
                :total true
                (param id UUID)
                (returns (union
                    (ok User :http 200)
                    (err :not-found {:id id} :http 404)))
                (ok id)))"#;
        let output = generate(input);
        assert!(output.contains("store: &impl Store<User>"));
    }

    #[test]
    fn test_mut_store_for_writes() {
        let input = r#"(module test :version 1
            (type User (field id UUID :immutable :generated) (field name String))
            (effect-set db-write [:writes user-store :reads user-store])
            (fn create-user
                :effects [db-write]
                :total true
                (param input {:name String :email String})
                (returns (union (ok User :http 201)))
                (ok input)))"#;
        let output = generate(input);
        assert!(output.contains("store: &mut impl Store<User>"));
    }

    #[test]
    fn test_full_example() {
        let source = std::fs::read_to_string(
            concat!(env!("CARGO_MANIFEST_DIR"), "/examples/user-service.pct"),
        )
        .unwrap();
        let output = generate(&source);

        // Should have pact-runtime imports
        assert!(output.contains("use pact_runtime::prelude::*;"));
        assert!(output.contains("use serde::{Serialize, Deserialize};"));

        // Should have Serialize/Deserialize on structs
        assert!(output.contains("#[derive(Debug, Clone, Serialize, Deserialize)]"));

        // Should generate User struct
        assert!(output.contains("pub struct User {"));

        // Should generate input struct
        assert!(output.contains("pub struct CreateUserInput {"));

        // Should generate result enums
        assert!(output.contains("pub enum GetUserByIdResult {"));
        assert!(output.contains("pub enum CreateUserResult {"));

        // Should use Store<User> trait bound
        assert!(output.contains("Store<User>"));

        // Should use named fields in error variants
        // Both use String since the payload references param id (UUID → &str → String)
        assert!(output.contains("NotFound { id: String }"));

        // Should have proper function signatures
        assert!(output.contains("pub fn get_user_by_id("));
        assert!(output.contains("pub fn create_user("));

        // Should use store methods instead of effect traits
        assert!(output.contains("store.query_by_id("));
        assert!(output.contains("store.insert("));

        // Should map builtins correctly
        assert!(output.contains("validate_uuid("));
        assert!(output.contains("non_empty(&"));
        assert!(output.contains("User::from_input("));
        assert!(output.contains("User::validate_input(&"));

        // Should have StoreError in patterns
        assert!(output.contains("StoreError::UniqueViolation"));

        // Should NOT have old-style effect traits
        assert!(!output.contains("pub trait DbRead"));
        assert!(!output.contains("pub trait DbWrite"));
    }
}

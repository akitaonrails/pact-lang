use crate::ast::*;

pub struct RustCodegen {
    output: String,
    indent: usize,
}

impl RustCodegen {
    pub fn new() -> Self {
        RustCodegen {
            output: String::new(),
            indent: 0,
        }
    }

    pub fn generate(mut self, module: &Module) -> String {
        self.emit_header(module);
        self.emit_line("");

        // Generate types
        for typedef in &module.types {
            self.emit_type_def(typedef);
            self.emit_line("");
        }

        // Generate effect traits
        for effect_set in &module.effect_sets {
            self.emit_effect_trait(effect_set);
            self.emit_line("");
        }

        // Generate return type enums for each function
        for func in &module.functions {
            self.emit_return_enum(func);
            self.emit_line("");
        }

        // Generate functions
        for func in &module.functions {
            self.emit_function(func, module);
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

        self.emit_line("#[derive(Debug, Clone)]");
        self.emit_line(&format!("pub struct {} {{", typedef.name));
        self.indent += 1;
        for field in &typedef.fields {
            let rust_type = type_expr_to_rust(&field.type_expr);
            let mut annotations = Vec::new();
            if field.immutable {
                annotations.push("immutable");
            }
            if field.generated {
                annotations.push("generated");
            }
            if !annotations.is_empty() {
                self.emit_line(&format!("/// {}", annotations.join(", ")));
            }
            if let Some(min) = field.min_len {
                self.emit_line(&format!("/// min_len: {}", min));
            }
            if let Some(max) = field.max_len {
                self.emit_line(&format!("/// max_len: {}", max));
            }
            if let Some(ref fmt) = field.format {
                self.emit_line(&format!("/// format: {}", fmt));
            }
            if let Some(ref uw) = field.unique_within {
                self.emit_line(&format!("/// unique_within: {}", uw));
            }
            self.emit_line(&format!("pub {}: {},", to_snake(&field.name), rust_type));
        }
        self.indent -= 1;
        self.emit_line("}");

        // Generate validation method
        self.emit_line("");
        self.emit_line(&format!("impl {} {{", typedef.name));
        self.indent += 1;
        self.emit_line("pub fn validate(&self) -> Result<(), Vec<String>> {");
        self.indent += 1;
        self.emit_line("let mut errors = Vec::new();");

        for field in &typedef.fields {
            let field_snake = to_snake(&field.name);
            if let Some(min) = field.min_len {
                self.emit_line(&format!(
                    "if self.{}.len() < {} {{ errors.push(format!(\"{{}} must be at least {} characters\", \"{}\")); }}",
                    field_snake, min, min, field.name
                ));
            }
            if let Some(max) = field.max_len {
                self.emit_line(&format!(
                    "if self.{}.len() > {} {{ errors.push(format!(\"{{}} must be at most {} characters\", \"{}\")); }}",
                    field_snake, max, max, field.name
                ));
            }
        }

        self.emit_line("if errors.is_empty() { Ok(()) } else { Err(errors) }");
        self.indent -= 1;
        self.emit_line("}");
        self.indent -= 1;
        self.emit_line("}");
    }

    fn emit_effect_trait(&mut self, effect_set: &EffectSetDef) {
        let trait_name = to_pascal(&effect_set.name);
        self.emit_line(&format!(
            "/// Effect set: {} — {:?}",
            effect_set.name,
            effect_set
                .effects
                .iter()
                .map(|e| format!("{:?}({})", e.kind, e.target))
                .collect::<Vec<_>>()
        ));
        self.emit_line(&format!("pub trait {} {{", trait_name));
        self.indent += 1;

        for effect in &effect_set.effects {
            match effect.kind {
                EffectKind::Reads => {
                    let store_pascal = to_pascal(&effect.target);
                    self.emit_line(&format!(
                        "fn query_{}<Q>(&self, query: Q) -> Option<{}Item> where Q: Into<{}Query>;",
                        to_snake(&effect.target),
                        store_pascal,
                        store_pascal
                    ));
                }
                EffectKind::Writes => {
                    let store_pascal = to_pascal(&effect.target);
                    self.emit_line(&format!(
                        "fn insert_{}(&mut self, item: {}Item) -> Result<{}Item, {}Error>;",
                        to_snake(&effect.target),
                        store_pascal,
                        store_pascal,
                        store_pascal
                    ));
                }
                EffectKind::Sends => {
                    self.emit_line(&format!(
                        "fn send_{}(&mut self, payload: impl Into<Vec<u8>>);",
                        to_snake(&effect.target)
                    ));
                }
            }
        }

        self.indent -= 1;
        self.emit_line("}");
    }

    fn emit_return_enum(&mut self, func: &FnDef) {
        let enum_name = format!("{}Result", to_pascal(&func.name));

        if let Some(ref prov) = func.provenance {
            if let Some(ref req) = prov.req {
                self.emit_line(&format!("/// Spec: {}", req));
            }
            if !prov.test.is_empty() {
                self.emit_line(&format!("/// Tests: {}", prov.test.join(", ")));
            }
        }
        if func.total {
            self.emit_line("/// Total: this function handles all cases exhaustively");
        }
        if let Some(ref budget) = func.latency_budget {
            self.emit_line(&format!("/// Latency budget: {}", budget));
        }
        if !func.called_by.is_empty() {
            self.emit_line(&format!(
                "/// Called by: {}",
                func.called_by.join(", ")
            ));
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
                    let payload_type = type_expr_to_rust(payload);
                    if payload_type == "()" {
                        self.emit_line(&format!("{},", variant_name));
                    } else {
                        self.emit_line(&format!("{}({}),", variant_name, payload_type));
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
                    let payload_type = type_expr_to_rust(payload);
                    if payload_type == "()" {
                        self.emit_line(&format!(
                            "{}::{} => {},",
                            enum_name, variant_name, status
                        ));
                    } else {
                        self.emit_line(&format!(
                            "{}::{}(_) => {},",
                            enum_name, variant_name, status
                        ));
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
                    let payload_type = type_expr_to_rust(payload);
                    if payload_type == "()" {
                        self.emit_line(&format!(
                            "{}::{} => write!(f, \"Error: {}\"),",
                            enum_name, variant_name, tag
                        ));
                    } else {
                        self.emit_line(&format!(
                            "{}::{}(v) => write!(f, \"Error({}): {{:?}}\", v),",
                            enum_name, variant_name, tag
                        ));
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

    fn emit_function(&mut self, func: &FnDef, module: &Module) {
        let fn_name = to_snake(&func.name);
        let return_type = format!("{}Result", to_pascal(&func.name));

        // Collect required trait bounds from effects
        let trait_bounds: Vec<String> = func
            .effects
            .iter()
            .map(|e| to_pascal(e))
            .collect();

        // Build parameter list
        let mut params = Vec::new();
        if !trait_bounds.is_empty() {
            params.push(format!("ctx: &mut Ctx"));
        }
        for param in &func.params {
            let rust_type = type_expr_to_rust(&param.type_expr);
            params.push(format!("{}: {}", to_snake(&param.name), rust_type));
        }

        // Build generic bounds
        let generics = if trait_bounds.is_empty() {
            String::new()
        } else {
            format!(
                "<Ctx: {}>",
                trait_bounds.join(" + ")
            )
        };

        // Doc comment
        if let Some(ref prov) = func.provenance {
            if let Some(ref req) = prov.req {
                self.emit_line(&format!("/// Spec: {}", req));
            }
            if !prov.test.is_empty() {
                self.emit_line(&format!("/// Tests: {}", prov.test.join(", ")));
            }
        }
        if !func.called_by.is_empty() {
            self.emit_line(&format!(
                "/// Called by: {}",
                func.called_by.join(", ")
            ));
        }
        if let Some(ref budget) = func.latency_budget {
            self.emit_line(&format!("/// Latency budget: {}", budget));
        }
        if func.total {
            self.emit_line("/// Total: handles all cases exhaustively");
        }

        // Effects as doc comment
        if !func.effects.is_empty() {
            let effect_desc: Vec<String> = func
                .effects
                .iter()
                .filter_map(|name| {
                    module.effect_sets.iter().find(|es| es.name == *name).map(|es| {
                        let effs: Vec<String> = es
                            .effects
                            .iter()
                            .map(|e| format!("{:?}({})", e.kind, e.target))
                            .collect();
                        format!("{}: [{}]", name, effs.join(", "))
                    })
                })
                .collect();
            self.emit_line(&format!("/// Effects: {}", effect_desc.join("; ")));
        }

        self.emit_line(&format!(
            "pub fn {}{}({}) -> {} {{",
            fn_name,
            generics,
            params.join(", "),
            return_type
        ));
        self.indent += 1;
        self.emit_expr(&func.body, &return_type);
        self.indent -= 1;
        self.emit_line("}");
    }

    fn emit_expr(&mut self, expr: &Expr, return_type: &str) {
        match expr {
            Expr::Let {
                bindings, body, ..
            } => {
                for (name, value) in bindings {
                    self.emit_indent();
                    self.output.push_str(&format!("let {} = ", to_snake(name)));
                    self.emit_expr_inline(value, return_type);
                    self.output.push_str(";\n");
                }
                self.emit_expr(body, return_type);
            }
            Expr::Match { expr, arms, .. } => {
                self.emit_indent();
                self.output.push_str("match ");
                self.emit_expr_inline(expr, return_type);
                self.output.push_str(" {\n");
                self.indent += 1;
                for arm in arms {
                    self.emit_indent();
                    self.emit_pattern(&arm.pattern);
                    self.output.push_str(" => ");
                    self.emit_expr_inline(&arm.body, return_type);
                    self.output.push_str(",\n");
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
                self.emit_expr_inline(cond, return_type);
                self.output.push_str(" {\n");
                self.indent += 1;
                self.emit_expr(then_branch, return_type);
                self.indent -= 1;
                self.emit_indent();
                self.output.push_str("} else {\n");
                self.indent += 1;
                self.emit_expr(else_branch, return_type);
                self.indent -= 1;
                self.emit_indent();
                self.output.push_str("}\n");
            }
            _ => {
                self.emit_indent();
                self.emit_expr_inline(expr, return_type);
                self.output.push('\n');
            }
        }
    }

    fn emit_expr_inline(&mut self, expr: &Expr, return_type: &str) {
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
                self.output.push_str(&format!("{}::Ok(", return_type));
                self.emit_expr_inline(inner, return_type);
                self.output.push(')');
            }
            Expr::Err { tag, payload, .. } => {
                let variant = to_pascal(tag);
                self.output
                    .push_str(&format!("{}::{}(", return_type, variant));
                self.emit_expr_inline(payload, return_type);
                self.output.push(')');
            }
            Expr::Call { name, args, .. } => {
                self.output.push_str(&to_snake(name));
                self.output.push('(');
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        self.output.push_str(", ");
                    }
                    self.emit_expr_inline(arg, return_type);
                }
                self.output.push(')');
            }
            Expr::FieldAccess { expr, field, .. } => {
                self.emit_expr_inline(expr, return_type);
                self.output.push('.');
                self.output.push_str(&to_snake(field));
            }
            Expr::MapLit(entries, _) => {
                // Emit as a struct-like constructor or a HashMap
                self.output.push_str("{ ");
                for (i, (key, val)) in entries.iter().enumerate() {
                    if i > 0 {
                        self.output.push_str(", ");
                    }
                    self.output.push_str(&to_snake(key));
                    self.output.push_str(": ");
                    self.emit_expr_inline(val, return_type);
                }
                self.output.push_str(" }");
            }
            Expr::Wildcard(_) => {
                self.output.push('_');
            }
            Expr::Let {
                bindings, body, ..
            } => {
                self.output.push_str("{\n");
                self.indent += 1;
                for (name, value) in bindings {
                    self.emit_indent();
                    self.output.push_str(&format!("let {} = ", to_snake(name)));
                    self.emit_expr_inline(value, return_type);
                    self.output.push_str(";\n");
                }
                self.emit_indent();
                self.emit_expr_inline(body, return_type);
                self.output.push('\n');
                self.indent -= 1;
                self.emit_indent();
                self.output.push('}');
            }
            Expr::Match { expr, arms, .. } => {
                self.output.push_str("match ");
                self.emit_expr_inline(expr, return_type);
                self.output.push_str(" {\n");
                self.indent += 1;
                for arm in arms {
                    self.emit_indent();
                    self.emit_pattern(&arm.pattern);
                    self.output.push_str(" => ");
                    self.emit_expr_inline(&arm.body, return_type);
                    self.output.push_str(",\n");
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
                self.emit_expr_inline(cond, return_type);
                self.output.push_str(" { ");
                self.emit_expr_inline(then_branch, return_type);
                self.output.push_str(" } else { ");
                self.emit_expr_inline(else_branch, return_type);
                self.output.push_str(" }");
            }
        }
    }

    fn emit_pattern(&mut self, pattern: &Pattern) {
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
                                self.emit_pattern(arg);
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
                                self.emit_pattern(arg);
                            }
                        }
                        self.output.push(')');
                    }
                    "err" => {
                        self.output.push_str("Err(");
                        if args.is_empty() {
                            self.output.push('_');
                        } else {
                            for (i, arg) in args.iter().enumerate() {
                                if i > 0 {
                                    self.output.push_str(", ");
                                }
                                self.emit_pattern(arg);
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
                                self.emit_pattern(arg);
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

fn type_expr_to_rust(type_expr: &TypeExpr) -> String {
    match type_expr {
        TypeExpr::Named(name) => match name.as_str() {
            "UUID" => "Uuid".to_string(),
            "String" => "String".to_string(),
            "Int" => "i64".to_string(),
            "Bool" => "bool".to_string(),
            "Unit" => "()".to_string(),
            other => other.to_string(),
        },
        TypeExpr::Map(fields) => {
            // Generate an anonymous struct-like type
            let field_strs: Vec<String> = fields
                .iter()
                .map(|(name, typ)| format!("{}: {}", to_snake(name), type_expr_to_rust(typ)))
                .collect();
            format!("{{ {} }}", field_strs.join(", "))
        }
        TypeExpr::List(inner) => {
            format!("Vec<{}>", type_expr_to_rust(inner))
        }
        TypeExpr::Union(_variants) => {
            // For inline unions, generate an enum name placeholder
            "UnionType".to_string()
        }
        TypeExpr::Enum(variants) => {
            // Enums become their own type
            format!("/* enum: {} */", variants.join(" | "))
        }
    }
}

/// Convert a kebab-case name to snake_case
fn to_snake(name: &str) -> String {
    name.replace('-', "_").replace('/', "_")
}

/// Convert a kebab-case name to PascalCase
fn to_pascal(name: &str) -> String {
    name.split(|c| c == '-' || c == '_' || c == '/')
        .map(|part| {
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
        RustCodegen::new().generate(&module)
    }

    #[test]
    fn test_to_snake() {
        assert_eq!(to_snake("get-user-by-id"), "get_user_by_id");
        assert_eq!(to_snake("api-router/handle-request"), "api_router_handle_request");
    }

    #[test]
    fn test_to_pascal() {
        assert_eq!(to_pascal("get-user-by-id"), "GetUserById");
        assert_eq!(to_pascal("db-read"), "DbRead");
        assert_eq!(to_pascal("not-found"), "NotFound");
    }

    #[test]
    fn test_generates_struct() {
        let output = generate(
            "(module test :version 1 (type User (field id UUID :immutable) (field name String :min-len 1 :max-len 200)))",
        );
        assert!(output.contains("pub struct User {"));
        assert!(output.contains("pub id: Uuid,"));
        assert!(output.contains("pub name: String,"));
        assert!(output.contains("fn validate"));
    }

    #[test]
    fn test_generates_effect_trait() {
        let output = generate(
            "(module test :version 1 (effect-set db-read [:reads user-store]))",
        );
        assert!(output.contains("pub trait DbRead {"));
    }

    #[test]
    fn test_generates_return_enum() {
        let input = r#"(module test :version 1
            (fn get-thing
                :effects []
                :total true
                (param id UUID)
                (returns (union
                    (ok UUID :http 200)
                    (err :not-found {:id id} :http 404)))
                (ok id)))"#;
        let output = generate(input);
        assert!(output.contains("pub enum GetThingResult {"));
        assert!(output.contains("Ok(Uuid),"));
        assert!(output.contains("NotFound("));
        assert!(output.contains("fn http_status(&self) -> u16"));
    }

    #[test]
    fn test_full_example() {
        let source = std::fs::read_to_string(
            concat!(env!("CARGO_MANIFEST_DIR"), "/examples/user-service.pct"),
        )
        .unwrap();
        let output = generate(&source);
        // Should generate valid-looking Rust
        assert!(output.contains("pub struct User {"));
        assert!(output.contains("pub trait DbRead {"));
        assert!(output.contains("pub trait DbWrite {"));
        assert!(output.contains("pub trait HttpRespond {"));
        assert!(output.contains("pub enum GetUserByIdResult {"));
        assert!(output.contains("pub enum CreateUserResult {"));
        assert!(output.contains("pub fn get_user_by_id"));
        assert!(output.contains("pub fn create_user"));
        assert!(output.contains("fn http_status(&self) -> u16"));
    }
}

use super::route_analysis::*;

struct Emitter {
    output: String,
    indent: usize,
}

impl Emitter {
    fn new() -> Self {
        Emitter { output: String::new(), indent: 0 }
    }

    fn line(&mut self, s: &str) {
        if s.is_empty() {
            self.output.push('\n');
        } else {
            for _ in 0..self.indent {
                self.output.push_str("    ");
            }
            self.output.push_str(s);
            self.output.push('\n');
        }
    }

    fn blank(&mut self) {
        self.output.push('\n');
    }
}

/// Generate handlers.rs for the scaffolded web project
pub fn emit(table: &RouteTable) -> String {
    let mut e = Emitter::new();

    emit_imports(&mut e, table);
    e.blank();

    // Emit form structs for create routes
    for route in &table.routes {
        if route.kind == RouteKind::Create && !route.form_fields.is_empty() {
            emit_form_struct(&mut e, route);
            e.blank();
        }
    }

    // HTML handlers
    e.line("// ─── HTML Handlers ───");
    e.blank();

    for route in &table.routes {
        match route.kind {
            RouteKind::List => emit_list_handler(&mut e, route, table),
            RouteKind::NewForm => emit_new_form_handler(&mut e, route, table),
            RouteKind::Create => emit_create_handler(&mut e, route, table),
            RouteKind::Show => emit_show_handler(&mut e, route, table),
            RouteKind::Delete => emit_delete_handler(&mut e, route, table),
        }
        e.blank();
    }

    // JSON API handlers
    e.line("// ─── JSON API Handlers ───");
    e.blank();

    for route in &table.routes {
        if route.api_handler_name.is_some() {
            match route.kind {
                RouteKind::List => emit_api_list_handler(&mut e, route, table),
                RouteKind::Create => emit_api_create_handler(&mut e, route, table),
                RouteKind::Show => emit_api_show_handler(&mut e, route, table),
                _ => {}
            }
            e.blank();
        }
    }

    e.output
}

fn emit_imports(e: &mut Emitter, table: &RouteTable) {
    e.line("use axum::extract::{Path, State};");
    e.line("use axum::http::StatusCode;");
    e.line("use axum::response::{Html, IntoResponse, Redirect};");
    e.line("use axum::Form;");
    e.line("use serde::Deserialize;");
    e.blank();
    e.line("use crate::html::{html_alert, html_form, html_page, html_table};");
    e.line("use crate::AppState;");
    e.line(&format!("use crate::generated::{}::*;", table.module_name));
    e.line("use pact_runtime::prelude::*;");
}

fn emit_form_struct(e: &mut Emitter, route: &Route) {
    let struct_name = format!("Create{}Form", route.store_type);
    e.line("#[derive(Deserialize)]");
    e.line(&format!("pub struct {} {{", struct_name));
    e.indent += 1;
    for field in &route.form_fields {
        e.line(&format!("pub {}: String,", field.name));
    }
    e.indent -= 1;
    e.line("}");
}

fn store_field(store_type: &str) -> String {
    format!("{}_store", store_type.to_lowercase())
}

fn emit_list_handler(e: &mut Emitter, route: &Route, table: &RouteTable) {
    let sf = store_field(&route.store_type);
    let plural = route.path.trim_start_matches('/');
    let title = to_title_case(plural);
    let display_fields = get_display_fields(route, table);

    e.line(&format!("pub async fn {}(State(state): State<AppState>) -> Html<String> {{", route.handler_name));
    e.indent += 1;
    e.line(&format!("let store = state.{}.lock().unwrap();", sf));
    e.line("let items = store.list_all();");
    e.blank();

    // Build table rows
    e.line("let rows: Vec<Vec<String>> = items");
    e.indent += 1;
    e.line(".iter()");
    e.line(".map(|item| {");
    e.indent += 1;
    e.line("vec![");
    e.indent += 1;
    e.line(&format!(
        "format!(r#\"<a href=\"/{}/{{}}\" class=\"text-indigo-600 hover:text-indigo-800\">{{}}</a>\"#, item.id, &item.id.to_string()[..8]),",
        plural,
    ));
    for field in &display_fields {
        e.line(&format!("item.{}.clone(),", field));
    }
    e.line(&format!(
        "format!(r#\"<form method=\"POST\" action=\"/{}/{{}}/delete\" class=\"inline\"><button type=\"submit\" class=\"text-red-600 hover:text-red-800 text-sm\">Delete</button></form>\"#, item.id),",
        plural,
    ));
    e.indent -= 1;
    e.line("]");
    e.indent -= 1;
    e.line("})");
    e.line(".collect();");
    e.indent -= 1;
    e.blank();

    // Build headers
    let mut headers = vec!["\"ID\"".to_string()];
    for field in &display_fields {
        headers.push(format!("\"{}\"", to_title_case(field)));
    }
    headers.push("\"Actions\"".to_string());

    // Empty state
    e.line("let body = if items.is_empty() {");
    e.indent += 1;
    e.line(&format!(
        "format!(r#\"<h1 class=\"text-2xl font-bold mb-6\">{}</h1><p class=\"text-gray-500\">No {} yet. <a href=\"/{}/new\" class=\"text-indigo-600 hover:underline\">Create one</a>.</p>\"#)",
        title, plural, plural,
    ));
    e.indent -= 1;
    e.line("} else {");
    e.indent += 1;
    e.line(&format!(
        "format!(r#\"<h1 class=\"text-2xl font-bold mb-6\">{} ({{}})</h1>{{}}\"#, items.len(), html_table(&[{}], &rows))",
        title, headers.join(", "),
    ));
    e.indent -= 1;
    e.line("};");
    e.blank();
    e.line(&format!("Html(html_page(\"{}\", &body))", title));
    e.indent -= 1;
    e.line("}");
}

fn emit_new_form_handler(e: &mut Emitter, route: &Route, _table: &RouteTable) {
    let type_title = to_title_case(&route.store_type.to_lowercase());
    let plural = route.store_type.to_lowercase() + "s";
    let fields = form_field_entries(&route.form_fields);

    e.line(&format!("pub async fn {}() -> Html<String> {{", route.handler_name));
    e.indent += 1;
    e.line(&format!(
        "let body = format!(r#\"<h1 class=\"text-2xl font-bold mb-6\">Create {}</h1>{{}}\"#, html_form(\"/{}\", &[{}]));",
        type_title, plural, fields,
    ));
    e.line(&format!("Html(html_page(\"New {}\", &body))", type_title));
    e.indent -= 1;
    e.line("}");
}

fn emit_create_handler(e: &mut Emitter, route: &Route, _table: &RouteTable) {
    let sf = store_field(&route.store_type);
    let form_struct = format!("Create{}Form", route.store_type);
    let type_title = to_title_case(&route.store_type.to_lowercase());
    let plural = route.store_type.to_lowercase() + "s";

    let fn_route = match &route.function {
        Some(f) => f,
        None => return,
    };

    let input_struct = match &fn_route.input_struct {
        Some(s) => s.clone(),
        None => return,
    };

    let fields = form_field_entries(&route.form_fields);

    e.line(&format!("pub async fn {}(", route.handler_name));
    e.indent += 1;
    e.line("State(state): State<AppState>,");
    e.line(&format!("Form(form): Form<{}>,", form_struct));
    e.indent -= 1;
    e.line(") -> impl IntoResponse {");
    e.indent += 1;

    // Build input struct from form
    e.line(&format!("let input = {} {{", input_struct));
    e.indent += 1;
    for field in &route.form_fields {
        e.line(&format!("{name}: form.{name},", name = field.name));
    }
    e.indent -= 1;
    e.line("};");
    e.blank();

    // Call domain function
    e.line(&format!("let mut store = state.{}.lock().unwrap();", sf));
    e.line(&format!("let result = {}(&mut *store, input);", fn_route.fn_name));
    e.blank();

    // Match on result
    e.line("match result {");
    e.indent += 1;

    for variant in &fn_route.variants {
        if variant.is_ok {
            match &variant.payload_kind {
                PayloadKind::Type(type_name) => {
                    let singular = type_name.to_lowercase();
                    e.line(&format!("{}::Ok({}) => {{", fn_route.result_enum, singular));
                    e.indent += 1;
                    e.line(&format!(
                        "Redirect::to(&format!(\"/{}/{{}}?created=1\", {}.id)).into_response()",
                        plural, singular,
                    ));
                    e.indent -= 1;
                    e.line("}");
                }
                _ => {
                    e.line(&format!("{}::Ok(_) => {{", fn_route.result_enum));
                    e.indent += 1;
                    e.line(&format!("Redirect::to(\"/{}\").into_response()", plural));
                    e.indent -= 1;
                    e.line("}");
                }
            }
        } else {
            let pattern = variant_pattern(&fn_route.result_enum, variant);
            let error_msg = variant_error_msg(variant);

            e.line(&format!("{} => {{", pattern));
            e.indent += 1;
            e.line(&format!(
                "let body = format!(r#\"<h1 class=\"text-2xl font-bold mb-6\">Create {}</h1>{{}}\n{{}}\"#, html_alert(\"error\", &{}), html_form(\"/{}\", &[{}]));",
                type_title, error_msg, plural, fields,
            ));
            e.line(&format!("Html(html_page(\"New {}\", &body)).into_response()", type_title));
            e.indent -= 1;
            e.line("}");
        }
    }

    e.indent -= 1;
    e.line("}");
    e.indent -= 1;
    e.line("}");
}

fn emit_show_handler(e: &mut Emitter, route: &Route, _table: &RouteTable) {
    let sf = store_field(&route.store_type);
    let type_title = to_title_case(&route.store_type.to_lowercase());
    let plural = route.store_type.to_lowercase() + "s";

    let fn_route = match &route.function {
        Some(f) => f,
        None => return,
    };

    e.line(&format!("pub async fn {}(", route.handler_name));
    e.indent += 1;
    e.line("State(state): State<AppState>,");
    e.line("Path(id): Path<String>,");
    e.indent -= 1;
    e.line(") -> impl IntoResponse {");
    e.indent += 1;

    e.line(&format!("let store = state.{}.lock().unwrap();", sf));
    e.line(&format!("let result = {}(&*store, &id);", fn_route.fn_name));
    e.blank();

    e.line("match result {");
    e.indent += 1;

    for variant in &fn_route.variants {
        if variant.is_ok {
            let singular = route.store_type.to_lowercase();
            e.line(&format!("{}::Ok({}) => {{", fn_route.result_enum, singular));
            e.indent += 1;
            e.line(&format!(
                "let body = format!(r#\"<h1 class=\"text-2xl font-bold mb-6\">{} Details</h1><div class=\"bg-white shadow rounded-lg p-6\"><dl class=\"grid grid-cols-2 gap-4\"><dt class=\"text-sm font-medium text-gray-500\">ID</dt><dd class=\"text-sm text-gray-900\">{{}}</dd></dl><div class=\"mt-6 flex space-x-4\"><a href=\"/\" class=\"text-indigo-600 hover:underline\">Back to list</a><form method=\"POST\" action=\"/{}/{{}}/delete\" class=\"inline\"><button type=\"submit\" class=\"text-red-600 hover:underline\">Delete</button></form></div></div>\"#, {}.id, {}.id);",
                type_title, plural, singular, singular,
            ));
            e.line(&format!("Html(html_page(\"{} Details\", &body)).into_response()", type_title));
            e.indent -= 1;
            e.line("}");
        } else {
            let tag = variant.tag.as_deref().unwrap_or("error");
            let status_code = http_status_to_axum(variant.http_status);
            let pattern = variant_pattern(&fn_route.result_enum, variant);
            let error_detail = variant_error_detail(variant);
            let vtitle = variant_to_title(tag);

            e.line(&format!("{} => {{", pattern));
            e.indent += 1;
            e.line(&format!(
                "let body = format!(r#\"<h1 class=\"text-2xl font-bold mb-6\">{}</h1>{{}}<a href=\"/\" class=\"text-indigo-600 hover:underline\">Back to list</a>\"#, html_alert(\"error\", &{}));",
                vtitle, error_detail,
            ));
            e.line(&format!(
                "({}, Html(html_page(\"{}\", &body))).into_response()",
                status_code, vtitle,
            ));
            e.indent -= 1;
            e.line("}");
        }
    }

    e.indent -= 1;
    e.line("}");
    e.indent -= 1;
    e.line("}");
}

fn emit_delete_handler(e: &mut Emitter, route: &Route, _table: &RouteTable) {
    let sf = store_field(&route.store_type);

    e.line(&format!("pub async fn {}(", route.handler_name));
    e.indent += 1;
    e.line("State(state): State<AppState>,");
    e.line("Path(id): Path<String>,");
    e.indent -= 1;
    e.line(") -> impl IntoResponse {");
    e.indent += 1;
    e.line("if let Ok(uuid) = id.parse::<uuid::Uuid>() {");
    e.indent += 1;
    e.line(&format!("let mut store = state.{}.lock().unwrap();", sf));
    e.line("store.delete(&uuid);");
    e.indent -= 1;
    e.line("}");
    e.line("Redirect::to(\"/\")");
    e.indent -= 1;
    e.line("}");
}

// ─── JSON API Handlers ───

fn emit_api_list_handler(e: &mut Emitter, route: &Route, _table: &RouteTable) {
    let api_handler = match &route.api_handler_name {
        Some(h) => h.clone(),
        None => return,
    };
    let sf = store_field(&route.store_type);

    e.line(&format!("pub async fn {}(State(state): State<AppState>) -> impl IntoResponse {{", api_handler));
    e.indent += 1;
    e.line(&format!("let store = state.{}.lock().unwrap();", sf));
    e.line("let items = store.list_all();");
    e.line("(StatusCode::OK, axum::Json(items))");
    e.indent -= 1;
    e.line("}");
}

fn emit_api_show_handler(e: &mut Emitter, route: &Route, _table: &RouteTable) {
    let api_handler = match &route.api_handler_name {
        Some(h) => h.clone(),
        None => return,
    };
    let sf = store_field(&route.store_type);

    let fn_route = match &route.function {
        Some(f) => f,
        None => return,
    };

    e.line(&format!("pub async fn {}(", api_handler));
    e.indent += 1;
    e.line("State(state): State<AppState>,");
    e.line("Path(id): Path<String>,");
    e.indent -= 1;
    e.line(") -> impl IntoResponse {");
    e.indent += 1;

    e.line(&format!("let store = state.{}.lock().unwrap();", sf));
    e.line(&format!("let result = {}(&*store, &id);", fn_route.fn_name));
    e.blank();

    e.line("match result {");
    e.indent += 1;

    for variant in &fn_route.variants {
        if variant.is_ok {
            let singular = route.store_type.to_lowercase();
            e.line(&format!("{}::Ok({}) => {{", fn_route.result_enum, singular));
            e.indent += 1;
            e.line(&format!(
                "({}, axum::Json(serde_json::to_value({}).unwrap())).into_response()",
                http_status_to_axum(variant.http_status), singular,
            ));
            e.indent -= 1;
            e.line("}");
        } else {
            let pattern = variant_pattern(&fn_route.result_enum, variant);
            let status_code = http_status_to_axum(variant.http_status);
            let json_body = variant_json_body(variant);

            e.line(&format!("{} => {{", pattern));
            e.indent += 1;
            e.line(&format!("({}, axum::Json({})).into_response()", status_code, json_body));
            e.indent -= 1;
            e.line("}");
        }
    }

    e.indent -= 1;
    e.line("}");
    e.indent -= 1;
    e.line("}");
}

fn emit_api_create_handler(e: &mut Emitter, route: &Route, _table: &RouteTable) {
    let api_handler = match &route.api_handler_name {
        Some(h) => h.clone(),
        None => return,
    };
    let sf = store_field(&route.store_type);

    let fn_route = match &route.function {
        Some(f) => f,
        None => return,
    };

    let input_struct = match &fn_route.input_struct {
        Some(s) => s.clone(),
        None => return,
    };

    e.line(&format!("pub async fn {}(", api_handler));
    e.indent += 1;
    e.line("State(state): State<AppState>,");
    e.line(&format!("axum::Json(input): axum::Json<{}>,", input_struct));
    e.indent -= 1;
    e.line(") -> impl IntoResponse {");
    e.indent += 1;

    e.line(&format!("let mut store = state.{}.lock().unwrap();", sf));
    e.line(&format!("let result = {}(&mut *store, input);", fn_route.fn_name));
    e.blank();

    e.line("match result {");
    e.indent += 1;

    for variant in &fn_route.variants {
        if variant.is_ok {
            let singular = route.store_type.to_lowercase();
            e.line(&format!("{}::Ok({}) => {{", fn_route.result_enum, singular));
            e.indent += 1;
            e.line(&format!(
                "({}, axum::Json(serde_json::to_value({}).unwrap())).into_response()",
                http_status_to_axum(variant.http_status), singular,
            ));
            e.indent -= 1;
            e.line("}");
        } else {
            let pattern = variant_pattern(&fn_route.result_enum, variant);
            let status_code = http_status_to_axum(variant.http_status);
            let json_body = variant_json_body(variant);

            e.line(&format!("{} => {{", pattern));
            e.indent += 1;
            e.line(&format!("({}, axum::Json({})).into_response()", status_code, json_body));
            e.indent -= 1;
            e.line("}");
        }
    }

    e.indent -= 1;
    e.line("}");
    e.indent -= 1;
    e.line("}");
}

// ─── Helpers ───

fn variant_pattern(result_enum: &str, variant: &RouteVariant) -> String {
    match &variant.payload_kind {
        PayloadKind::Map(fields) => {
            let field_names: Vec<String> = fields.iter().map(|(n, _)| n.clone()).collect();
            format!("{}::{} {{ {} }}", result_enum, variant.variant_name, field_names.join(", "))
        }
        PayloadKind::List(_) => {
            format!("{}::{}(errors)", result_enum, variant.variant_name)
        }
        PayloadKind::Type(_) => {
            format!("{}::{}(v)", result_enum, variant.variant_name)
        }
        PayloadKind::Unit => {
            format!("{}::{}", result_enum, variant.variant_name)
        }
    }
}

fn variant_error_msg(variant: &RouteVariant) -> String {
    let tag = variant.tag.as_deref().unwrap_or("error");
    match &variant.payload_kind {
        PayloadKind::Map(fields) if fields.len() == 1 => {
            format!("format!(\"{}: {{}}\", {})", tag_to_message(tag), fields[0].0)
        }
        PayloadKind::List(_) => {
            "errors.iter().map(|e| format!(\"{}: {}\", e.field, e.message)).collect::<Vec<_>>().join(\", \")".to_string()
        }
        _ => {
            format!("\"{}\".to_string()", tag_to_message(tag))
        }
    }
}

fn variant_error_detail(variant: &RouteVariant) -> String {
    let tag = variant.tag.as_deref().unwrap_or("error");
    match &variant.payload_kind {
        PayloadKind::Map(fields) if fields.len() == 1 => {
            format!("format!(\"{{}}\", {})", fields[0].0)
        }
        _ => {
            format!("\"{}\"", tag_to_message(tag))
        }
    }
}

fn variant_json_body(variant: &RouteVariant) -> String {
    let tag = variant.tag.as_deref().unwrap_or("error");
    match &variant.payload_kind {
        PayloadKind::Map(fields) => {
            let mut entries = vec![format!("\"error\": \"{}\"", tag)];
            for (name, _) in fields {
                entries.push(format!("\"{}\": {}", name, name));
            }
            format!("serde_json::json!({{ {} }})", entries.join(", "))
        }
        PayloadKind::List(_) => {
            format!("serde_json::json!({{ \"error\": \"{}\", \"errors\": errors }})", tag)
        }
        _ => {
            format!("serde_json::json!({{ \"error\": \"{}\" }})", tag)
        }
    }
}

fn get_display_fields(route: &Route, table: &RouteTable) -> Vec<String> {
    // Find the NewForm route for the same store type to get field names
    for r in &table.routes {
        if r.kind == RouteKind::NewForm && r.store_type == route.store_type {
            return r.form_fields.iter().map(|f| f.name.clone()).collect();
        }
    }
    for r in &table.routes {
        if r.kind == RouteKind::Create && r.store_type == route.store_type {
            return r.form_fields.iter().map(|f| f.name.clone()).collect();
        }
    }
    vec![]
}

fn form_field_entries(fields: &[FormField]) -> String {
    fields.iter().map(|f| {
        format!("(\"{}\", \"{}\", \"{}\")", f.name, f.label, f.input_type)
    }).collect::<Vec<_>>().join(", ")
}

fn http_status_to_axum(status: u16) -> String {
    match status {
        200 => "StatusCode::OK".to_string(),
        201 => "StatusCode::CREATED".to_string(),
        400 => "StatusCode::BAD_REQUEST".to_string(),
        404 => "StatusCode::NOT_FOUND".to_string(),
        409 => "StatusCode::CONFLICT".to_string(),
        422 => "StatusCode::UNPROCESSABLE_ENTITY".to_string(),
        500 => "StatusCode::INTERNAL_SERVER_ERROR".to_string(),
        _ => format!("StatusCode::from_u16({}).unwrap()", status),
    }
}

fn tag_to_message(tag: &str) -> String {
    tag.split('-')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => {
                    let upper = c.to_uppercase().to_string();
                    upper + chars.as_str()
                }
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn variant_to_title(tag: &str) -> String {
    tag_to_message(tag)
}

fn to_title_case(name: &str) -> String {
    name.split(|c: char| c == '-' || c == '_')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => {
                    let upper = c.to_uppercase().to_string();
                    upper + chars.as_str()
                }
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scaffold::route_analysis;
    use crate::lexer::Lexer;
    use crate::lower::Lowerer;
    use crate::parser::Parser;

    fn analyze_example() -> RouteTable {
        let source = std::fs::read_to_string(
            concat!(env!("CARGO_MANIFEST_DIR"), "/examples/user-service.pct"),
        ).unwrap();
        let mut lexer = Lexer::new(&source);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = Parser::new(tokens);
        let sexprs = parser.parse_program().unwrap();
        let mut lowerer = Lowerer::new();
        let module = lowerer.lower_module(&sexprs[0]).unwrap();
        route_analysis::analyze(&module)
    }

    #[test]
    fn test_handlers_has_imports() {
        let table = analyze_example();
        let output = emit(&table);
        assert!(output.contains("use axum::extract::{Path, State};"));
        assert!(output.contains("use crate::generated::user_service::*;"));
        assert!(output.contains("use pact_runtime::prelude::*;"));
    }

    #[test]
    fn test_handlers_has_form_struct() {
        let table = analyze_example();
        let output = emit(&table);
        assert!(output.contains("pub struct CreateUserForm {"));
        assert!(output.contains("pub name: String,"));
        assert!(output.contains("pub email: String,"));
    }

    #[test]
    fn test_handlers_has_list_handler() {
        let table = analyze_example();
        let output = emit(&table);
        assert!(output.contains("pub async fn list_users("));
        assert!(output.contains("store.list_all()"));
    }

    #[test]
    fn test_handlers_has_show_handler() {
        let table = analyze_example();
        let output = emit(&table);
        assert!(output.contains("pub async fn show_user("));
        assert!(output.contains("get_user_by_id("));
    }

    #[test]
    fn test_handlers_has_create_handler() {
        let table = analyze_example();
        let output = emit(&table);
        assert!(output.contains("pub async fn create_user_handler("));
        assert!(output.contains("create_user("));
    }

    #[test]
    fn test_handlers_has_delete_handler() {
        let table = analyze_example();
        let output = emit(&table);
        assert!(output.contains("pub async fn delete_user("));
        assert!(output.contains("store.delete("));
    }

    #[test]
    fn test_handlers_has_api_handlers() {
        let table = analyze_example();
        let output = emit(&table);
        assert!(output.contains("pub async fn api_list_users("));
        assert!(output.contains("pub async fn api_get_user("));
        assert!(output.contains("pub async fn api_create_user("));
    }

    #[test]
    fn test_handlers_api_uses_json() {
        let table = analyze_example();
        let output = emit(&table);
        assert!(output.contains("axum::Json(serde_json::to_value("));
        assert!(output.contains("serde_json::json!"));
    }
}

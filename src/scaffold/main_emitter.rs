use super::route_analysis::*;

/// Generate main.rs for the scaffolded web project
pub fn emit(table: &RouteTable) -> String {
    let mut out = String::new();

    // Imports
    out.push_str("use std::sync::{Arc, Mutex};\n");
    out.push_str("\n");
    out.push_str("use axum::routing::{get, post};\n");
    out.push_str("use axum::Router;\n");
    out.push_str("use pact_runtime::prelude::InMemoryStore;\n");
    out.push_str("\n");
    out.push_str("mod generated;\n");
    out.push_str("mod handlers;\n");
    out.push_str("mod html;\n");
    out.push_str("\n");

    // Use statements for generated types
    for store in &table.store_types {
        out.push_str(&format!(
            "use generated::{}::{};\n",
            table.module_name, store.type_name
        ));
    }
    out.push_str("\n");

    // AppState struct
    out.push_str("#[derive(Clone)]\n");
    out.push_str("pub struct AppState {\n");
    for store in &table.store_types {
        let field_name = format!("{}_store", store.singular);
        out.push_str(&format!(
            "    pub {}: Arc<Mutex<InMemoryStore<{}>>>,\n",
            field_name, store.type_name
        ));
    }
    out.push_str("}\n");
    out.push_str("\n");

    // main function
    out.push_str("#[tokio::main]\n");
    out.push_str("async fn main() {\n");

    // State initialization
    out.push_str("    let state = AppState {\n");
    for store in &table.store_types {
        let field_name = format!("{}_store", store.singular);
        out.push_str(&format!(
            "        {}: Arc::new(Mutex::new(InMemoryStore::new())),\n",
            field_name
        ));
    }
    out.push_str("    };\n");
    out.push_str("\n");

    // Router
    out.push_str("    let app = Router::new()\n");

    // HTML routes
    out.push_str("        // HTML routes\n");

    // Root route → list handler of first store type
    if let Some(list_route) = table.routes.iter().find(|r| r.kind == RouteKind::List) {
        out.push_str(&format!(
            "        .route(\"/\", get(handlers::{}))\n",
            list_route.handler_name
        ));
    }

    for route in &table.routes {
        match route.kind {
            RouteKind::List => {
                // Skip if path is the same as root (already added)
                if route.path != "/" {
                    // Don't duplicate the root route
                }
            }
            RouteKind::NewForm => {
                out.push_str(&format!(
                    "        .route(\"{}\", get(handlers::{}))\n",
                    route.path, route.handler_name
                ));
            }
            RouteKind::Create => {
                out.push_str(&format!(
                    "        .route(\"{}\", post(handlers::{}))\n",
                    route.path, route.handler_name
                ));
            }
            RouteKind::Show => {
                out.push_str(&format!(
                    "        .route(\"{}\", get(handlers::{}))\n",
                    route.path, route.handler_name
                ));
            }
            RouteKind::Delete => {
                out.push_str(&format!(
                    "        .route(\"{}\", post(handlers::{}))\n",
                    route.path, route.handler_name
                ));
            }
        }
    }

    // API routes
    out.push_str("        // JSON API routes\n");
    for route in &table.routes {
        if let Some(ref api_path) = route.api_path {
            if let Some(ref api_handler) = route.api_handler_name {
                let method = match route.method {
                    HttpMethod::Get => "get",
                    HttpMethod::Post => "post",
                };
                out.push_str(&format!(
                    "        .route(\"{}\", {}(handlers::{}))\n",
                    api_path, method, api_handler
                ));
            }
        }
    }

    out.push_str("        .with_state(state);\n");
    out.push_str("\n");
    out.push_str("    let listener = tokio::net::TcpListener::bind(\"0.0.0.0:3000\").await.unwrap();\n");

    let app_title = to_title_case(&table.module_name);
    out.push_str(&format!(
        "    eprintln!(\"{} listening on http://localhost:3000\");\n",
        app_title
    ));
    out.push_str("    axum::serve(listener, app).await.unwrap();\n");
    out.push_str("}\n");

    out
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
    fn test_main_has_app_state() {
        let table = analyze_example();
        let output = emit(&table);
        assert!(output.contains("pub struct AppState {"));
        assert!(output.contains("Arc<Mutex<InMemoryStore<User>>>"));
    }

    #[test]
    fn test_main_has_router() {
        let table = analyze_example();
        let output = emit(&table);
        assert!(output.contains("Router::new()"));
        assert!(output.contains("handlers::list_users"));
        assert!(output.contains("handlers::show_user"));
        assert!(output.contains("handlers::create_user_handler"));
        assert!(output.contains("handlers::delete_user"));
    }

    #[test]
    fn test_main_has_api_routes() {
        let table = analyze_example();
        let output = emit(&table);
        assert!(output.contains("/api/users"));
        assert!(output.contains("handlers::api_list_users"));
        assert!(output.contains("handlers::api_create_user"));
        assert!(output.contains("handlers::api_get_user"));
    }

    #[test]
    fn test_main_has_server() {
        let table = analyze_example();
        let output = emit(&table);
        assert!(output.contains("TcpListener::bind"));
        assert!(output.contains("axum::serve"));
        assert!(output.contains("User Service"));
    }
}

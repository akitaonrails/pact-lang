use super::route_analysis::RouteTable;

/// Generate Cargo.toml for the scaffolded web project
pub fn emit(table: &RouteTable) -> String {
    let package_name = table.module_name.replace('_', "-");

    format!(
        r#"[package]
name = "{name}-web"
version = "0.1.0"
edition = "2021"

[dependencies]
pact-runtime = {{ path = "../pact-runtime" }}
axum = "0.8"
tokio = {{ version = "1", features = ["full"] }}
serde = {{ version = "1", features = ["derive"] }}
serde_json = "1"
uuid = {{ version = "1", features = ["v4"] }}
"#,
        name = package_name,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scaffold::route_analysis::{StoreInfo, RouteTable};

    fn test_table() -> RouteTable {
        RouteTable {
            module_name: "user_service".to_string(),
            store_types: vec![StoreInfo {
                type_name: "User".to_string(),
                plural: "users".to_string(),
                singular: "user".to_string(),
                needs_mut: true,
            }],
            routes: vec![],
        }
    }

    #[test]
    fn test_cargo_emitter_package_name() {
        let output = emit(&test_table());
        assert!(output.contains("name = \"user-service-web\""));
    }

    #[test]
    fn test_cargo_emitter_dependencies() {
        let output = emit(&test_table());
        assert!(output.contains("pact-runtime"));
        assert!(output.contains("axum"));
        assert!(output.contains("tokio"));
        assert!(output.contains("serde"));
        assert!(output.contains("serde_json"));
        assert!(output.contains("uuid"));
    }
}

pub mod route_analysis;
pub mod html_emitter;
pub mod cargo_emitter;
pub mod main_emitter;
pub mod handlers_emitter;

use std::fs;
use std::path::Path;

use crate::ast::Module;
use route_analysis::RouteTable;

/// Output of scaffolding — all generated files
pub struct ScaffoldOutput {
    pub main_rs: String,
    pub handlers_rs: String,
    pub html_rs: String,
    pub cargo_toml: String,
    pub generated_mod_rs: String,
}

/// Generate all scaffold files from an AST module
pub fn scaffold(module: &Module) -> ScaffoldOutput {
    let table = route_analysis::analyze(module);

    let module_name = module.name.replace('-', "_");

    ScaffoldOutput {
        main_rs: main_emitter::emit(&table),
        handlers_rs: handlers_emitter::emit(&table),
        html_rs: html_emitter::emit(&table),
        cargo_toml: cargo_emitter::emit(&table),
        generated_mod_rs: format!("pub mod {};\n", module_name),
    }
}

/// Write scaffold output to disk at the given output directory
pub fn write_scaffold(output: &ScaffoldOutput, output_dir: &Path) -> Result<(), String> {
    let src_dir = output_dir.join("src");
    let generated_dir = src_dir.join("generated");

    fs::create_dir_all(&generated_dir)
        .map_err(|e| format!("Failed to create directories: {}", e))?;

    // Write Cargo.toml only if it doesn't exist
    let cargo_path = output_dir.join("Cargo.toml");
    if !cargo_path.exists() {
        fs::write(&cargo_path, &output.cargo_toml)
            .map_err(|e| format!("Failed to write Cargo.toml: {}", e))?;
        eprintln!("  Created {}", cargo_path.display());
    } else {
        eprintln!("  Skipped {} (already exists)", cargo_path.display());
    }

    // Write src files
    let files = [
        (src_dir.join("main.rs"), &output.main_rs),
        (src_dir.join("handlers.rs"), &output.handlers_rs),
        (src_dir.join("html.rs"), &output.html_rs),
        (generated_dir.join("mod.rs"), &output.generated_mod_rs),
    ];

    for (path, content) in &files {
        fs::write(path, content)
            .map_err(|e| format!("Failed to write {}: {}", path.display(), e))?;
        eprintln!("  Created {}", path.display());
    }

    Ok(())
}

/// Get the RouteTable for inspection (useful for tests)
pub fn analyze(module: &Module) -> RouteTable {
    route_analysis::analyze(module)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::lower::Lowerer;
    use crate::parser::Parser;

    fn parse_module(source: &str) -> Module {
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = Parser::new(tokens);
        let sexprs = parser.parse_program().unwrap();
        let mut lowerer = Lowerer::new();
        lowerer.lower_module(&sexprs[0]).unwrap()
    }

    #[test]
    fn test_scaffold_produces_all_files() {
        let source = std::fs::read_to_string(
            concat!(env!("CARGO_MANIFEST_DIR"), "/examples/user-service.pct"),
        ).unwrap();
        let module = parse_module(&source);
        let output = scaffold(&module);

        assert!(!output.main_rs.is_empty());
        assert!(!output.handlers_rs.is_empty());
        assert!(!output.html_rs.is_empty());
        assert!(!output.cargo_toml.is_empty());
        assert_eq!(output.generated_mod_rs, "pub mod user_service;\n");
    }

    #[test]
    fn test_scaffold_main_references_handlers() {
        let source = std::fs::read_to_string(
            concat!(env!("CARGO_MANIFEST_DIR"), "/examples/user-service.pct"),
        ).unwrap();
        let module = parse_module(&source);
        let output = scaffold(&module);

        // main.rs should reference handler functions
        assert!(output.main_rs.contains("handlers::list_users"));
        assert!(output.main_rs.contains("handlers::show_user"));
        assert!(output.main_rs.contains("handlers::create_user_handler"));

        // handlers.rs should contain those functions
        assert!(output.handlers_rs.contains("pub async fn list_users("));
        assert!(output.handlers_rs.contains("pub async fn show_user("));
        assert!(output.handlers_rs.contains("pub async fn create_user_handler("));
    }

    #[test]
    fn test_scaffold_write_to_disk() {
        let source = std::fs::read_to_string(
            concat!(env!("CARGO_MANIFEST_DIR"), "/examples/user-service.pct"),
        ).unwrap();
        let module = parse_module(&source);
        let output = scaffold(&module);

        let tmp_dir = std::env::temp_dir().join("pact-scaffold-test");
        let _ = std::fs::remove_dir_all(&tmp_dir);

        write_scaffold(&output, &tmp_dir).unwrap();

        assert!(tmp_dir.join("Cargo.toml").exists());
        assert!(tmp_dir.join("src/main.rs").exists());
        assert!(tmp_dir.join("src/handlers.rs").exists());
        assert!(tmp_dir.join("src/html.rs").exists());
        assert!(tmp_dir.join("src/generated/mod.rs").exists());

        // Cleanup
        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_scaffold_skip_existing_cargo() {
        let source = std::fs::read_to_string(
            concat!(env!("CARGO_MANIFEST_DIR"), "/examples/user-service.pct"),
        ).unwrap();
        let module = parse_module(&source);
        let output = scaffold(&module);

        let tmp_dir = std::env::temp_dir().join("pact-scaffold-skip-test");
        let _ = std::fs::remove_dir_all(&tmp_dir);
        std::fs::create_dir_all(&tmp_dir).unwrap();

        // Write a custom Cargo.toml first
        std::fs::write(tmp_dir.join("Cargo.toml"), "# custom\n").unwrap();

        write_scaffold(&output, &tmp_dir).unwrap();

        // Should not overwrite
        let content = std::fs::read_to_string(tmp_dir.join("Cargo.toml")).unwrap();
        assert_eq!(content, "# custom\n");

        // Cleanup
        let _ = std::fs::remove_dir_all(&tmp_dir);
    }
}

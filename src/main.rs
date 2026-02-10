use std::fs;
use std::path::PathBuf;
use std::process;

use pact_lang::codegen::rust::RustCodegen;
use pact_lang::codegen::rust_v2::RustV2Codegen;
use pact_lang::diagnostics::{self, DiagnosticKind};
use pact_lang::generate::yaml_parser::YamlParser;
use pact_lang::generate::spec_parser;
use pact_lang::generate::pct_emitter::PctEmitter;
use pact_lang::lexer::Lexer;
use pact_lang::lower::Lowerer;
use pact_lang::parser::Parser;
use pact_lang::semantic;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 || args[1] == "--help" || args[1] == "-h" {
        eprintln!("Usage: pact compile <input.pct> [-o <output-dir>] [--runtime]");
        eprintln!("");
        eprintln!("Commands:");
        eprintln!("  compile    Parse, analyze, and generate Rust code from a Pact file");
        eprintln!("  generate   Generate a .pct file from a YAML spec");
        eprintln!("  check      Parse and analyze without generating code");
        eprintln!("  parse      Parse only (show CST)");
        eprintln!("");
        eprintln!("Flags:");
        eprintln!("  --runtime  Generate code targeting pact-runtime crate");
        process::exit(if args.len() < 2 { 1 } else { 0 });
    }

    let command = &args[1];
    match command.as_str() {
        "compile" => cmd_compile(&args[2..]),
        "generate" => cmd_generate(&args[2..]),
        "check" => cmd_check(&args[2..]),
        "parse" => cmd_parse(&args[2..]),
        _ => {
            eprintln!("Unknown command: {}", command);
            eprintln!("Run 'pact --help' for usage");
            process::exit(1);
        }
    }
}

fn cmd_compile(args: &[String]) {
    let (input_path, output_dir, use_runtime) = parse_compile_args(args);
    let source = read_source(&input_path);

    // Lex
    let mut lexer = Lexer::new(&source);
    let tokens = match lexer.tokenize() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Lexer error: {}", e);
            process::exit(1);
        }
    };

    // Parse
    let mut parser = Parser::new(tokens);
    let sexprs = match parser.parse_program() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Parse error: {}", e);
            process::exit(1);
        }
    };

    if sexprs.is_empty() {
        eprintln!("No top-level expressions found");
        process::exit(1);
    }

    // Lower
    let mut lowerer = Lowerer::new();
    let module = match lowerer.lower_module(&sexprs[0]) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Lowering error: {}", e);
            process::exit(1);
        }
    };

    // Print lowering warnings
    if !lowerer.diagnostics.is_empty() {
        let formatted = diagnostics::format_diagnostics(&source, &lowerer.diagnostics);
        eprint!("{}", formatted);
    }

    // Semantic analysis
    let diags = semantic::analyze(&module);
    if !diags.is_empty() {
        let formatted = diagnostics::format_diagnostics(&source, &diags);
        eprint!("{}", formatted);

        let error_count = diags.iter().filter(|d| d.kind == DiagnosticKind::Error).count();
        if error_count > 0 {
            eprintln!("{} error(s) found. Aborting code generation.", error_count);
            process::exit(1);
        }
    }

    // Code generation
    let rust_code = if use_runtime {
        RustV2Codegen::new().generate(&module)
    } else {
        RustCodegen::new().generate(&module)
    };

    // Write output
    let output_dir = output_dir.unwrap_or_else(|| PathBuf::from("output"));
    fs::create_dir_all(&output_dir).unwrap_or_else(|e| {
        eprintln!("Failed to create output directory: {}", e);
        process::exit(1);
    });

    let output_file = output_dir.join(format!("{}.rs", module.name.replace('-', "_")));
    fs::write(&output_file, &rust_code).unwrap_or_else(|e| {
        eprintln!("Failed to write output: {}", e);
        process::exit(1);
    });

    eprintln!("Generated {} ({} bytes)", output_file.display(), rust_code.len());
}

fn cmd_check(args: &[String]) {
    let (input_path, _) = parse_args(args);
    let source = read_source(&input_path);

    let mut lexer = Lexer::new(&source);
    let tokens = match lexer.tokenize() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Lexer error: {}", e);
            process::exit(1);
        }
    };

    let mut parser = Parser::new(tokens);
    let sexprs = match parser.parse_program() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Parse error: {}", e);
            process::exit(1);
        }
    };

    if sexprs.is_empty() {
        eprintln!("No top-level expressions found");
        process::exit(1);
    }

    let mut lowerer = Lowerer::new();
    let module = match lowerer.lower_module(&sexprs[0]) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Lowering error: {}", e);
            process::exit(1);
        }
    };

    if !lowerer.diagnostics.is_empty() {
        let formatted = diagnostics::format_diagnostics(&source, &lowerer.diagnostics);
        eprint!("{}", formatted);
    }

    let diags = semantic::analyze(&module);
    if !diags.is_empty() {
        let formatted = diagnostics::format_diagnostics(&source, &diags);
        eprint!("{}", formatted);
    }

    let error_count = diags.iter().filter(|d| d.kind == DiagnosticKind::Error).count();
    let warning_count = diags.iter().filter(|d| d.kind == DiagnosticKind::Warning).count();

    eprintln!(
        "Module '{}' v{}: {} error(s), {} warning(s)",
        module.name,
        module.version.unwrap_or(0),
        error_count,
        warning_count
    );

    if error_count > 0 {
        process::exit(1);
    }
}

fn cmd_parse(args: &[String]) {
    let (input_path, _) = parse_args(args);
    let source = read_source(&input_path);

    let mut lexer = Lexer::new(&source);
    let tokens = match lexer.tokenize() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Lexer error: {}", e);
            process::exit(1);
        }
    };

    eprintln!("Tokens: {}", tokens.len());

    let mut parser = Parser::new(tokens);
    let sexprs = match parser.parse_program() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Parse error: {}", e);
            process::exit(1);
        }
    };

    for (i, sexpr) in sexprs.iter().enumerate() {
        println!("Expression {}: {:#?}", i, sexpr);
    }
}

fn cmd_generate(args: &[String]) {
    let (input_path, output_path) = parse_args(args);
    let source = read_source(&input_path);

    // Parse YAML
    let mut yaml_parser = YamlParser::new(&source);
    let yaml = match yaml_parser.parse() {
        Ok(y) => y,
        Err(e) => {
            eprintln!("YAML parse error: {}", e);
            process::exit(1);
        }
    };

    // Parse spec
    let spec = match spec_parser::parse_spec(&yaml) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Spec parse error: {}", e);
            process::exit(1);
        }
    };

    // Emit .pct
    let pct_source = PctEmitter::new().emit(&spec);

    // Validate by round-tripping through lexer → parser → lowerer
    let mut lexer = Lexer::new(&pct_source);
    let tokens = match lexer.tokenize() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Generated .pct has lexer errors: {}", e);
            eprintln!("--- generated source ---");
            eprintln!("{}", pct_source);
            process::exit(1);
        }
    };

    let mut parser = Parser::new(tokens);
    let sexprs = match parser.parse_program() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Generated .pct has parse errors: {}", e);
            eprintln!("--- generated source ---");
            eprintln!("{}", pct_source);
            process::exit(1);
        }
    };

    if sexprs.is_empty() {
        eprintln!("Generated .pct has no top-level expressions");
        process::exit(1);
    }

    let mut lowerer = Lowerer::new();
    match lowerer.lower_module(&sexprs[0]) {
        Ok(_) => {}
        Err(e) => {
            eprintln!("Generated .pct has lowering errors: {}", e);
            eprintln!("--- generated source ---");
            eprintln!("{}", pct_source);
            process::exit(1);
        }
    }

    // Write output
    let output_file = output_path.unwrap_or_else(|| {
        let stem = input_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("output");
        // Strip .spec suffix if present
        let stem = stem.strip_suffix(".spec").unwrap_or(stem);
        PathBuf::from(format!("{}.pct", stem))
    });

    fs::write(&output_file, &pct_source).unwrap_or_else(|e| {
        eprintln!("Failed to write output: {}", e);
        process::exit(1);
    });

    eprintln!(
        "Generated {} ({} bytes) from spec '{}'",
        output_file.display(),
        pct_source.len(),
        spec.title
    );
}

fn parse_compile_args(args: &[String]) -> (PathBuf, Option<PathBuf>, bool) {
    let mut input: Option<PathBuf> = None;
    let mut output = None;
    let mut use_runtime = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-o" | "--output" => {
                i += 1;
                if i < args.len() {
                    output = Some(PathBuf::from(&args[i]));
                }
            }
            "--runtime" => {
                use_runtime = true;
            }
            _ => {
                if input.is_none() {
                    input = Some(PathBuf::from(&args[i]));
                }
            }
        }
        i += 1;
    }

    let input = input.unwrap_or_else(|| {
        eprintln!("Expected input file path");
        process::exit(1);
    });

    (input, output, use_runtime)
}

fn parse_args(args: &[String]) -> (PathBuf, Option<PathBuf>) {
    if args.is_empty() {
        eprintln!("Expected input file path");
        process::exit(1);
    }

    let input = PathBuf::from(&args[0]);
    let mut output = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-o" | "--output" => {
                i += 1;
                if i < args.len() {
                    output = Some(PathBuf::from(&args[i]));
                }
            }
            _ => {}
        }
        i += 1;
    }

    (input, output)
}

fn read_source(path: &PathBuf) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| {
        eprintln!("Failed to read '{}': {}", path.display(), e);
        process::exit(1);
    })
}

use std::fs;
use std::path::PathBuf;
use std::process;

use ais_lang::codegen::rust::RustCodegen;
use ais_lang::diagnostics::{self, DiagnosticKind};
use ais_lang::lexer::Lexer;
use ais_lang::lower::Lowerer;
use ais_lang::parser::Parser;
use ais_lang::semantic;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 || args[1] == "--help" || args[1] == "-h" {
        eprintln!("Usage: ais-lang compile <input.ais> [-o <output-dir>]");
        eprintln!("");
        eprintln!("Commands:");
        eprintln!("  compile    Parse, analyze, and generate Rust code from an AIS file");
        eprintln!("  check      Parse and analyze without generating code");
        eprintln!("  parse      Parse only (show CST)");
        process::exit(if args.len() < 2 { 1 } else { 0 });
    }

    let command = &args[1];
    match command.as_str() {
        "compile" => cmd_compile(&args[2..]),
        "check" => cmd_check(&args[2..]),
        "parse" => cmd_parse(&args[2..]),
        _ => {
            eprintln!("Unknown command: {}", command);
            eprintln!("Run 'ais-lang --help' for usage");
            process::exit(1);
        }
    }
}

fn cmd_compile(args: &[String]) {
    let (input_path, output_dir) = parse_args(args);
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
    let rust_code = RustCodegen::new().generate(&module);

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

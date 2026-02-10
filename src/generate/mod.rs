pub mod yaml_ast;
pub mod yaml_parser;
pub mod spec_ast;
pub mod spec_parser;
pub mod pct_emitter;

#[cfg(test)]
mod integration_tests {
    use super::yaml_parser::YamlParser;
    use super::spec_parser;
    use super::pct_emitter::PctEmitter;
    use crate::lexer::Lexer;
    use crate::parser::Parser;
    use crate::lower::Lowerer;

    const EXAMPLE_SPEC: &str = "\
spec: SPEC-2024-0042
title: \"User service\"
owner: platform-team
domain:
  User:
    fields:
      - name: required, string, 1-200 chars
      - email: required, email format, unique
      - id: auto-generated, immutable
endpoints:
  get-user:
    description: \"Returns a user by ID\"
    input: user id (from URL)
    outputs:
      - success: the user found (200)
      - not found: when the ID doesn't exist (404)
    constraints:
      - max response time: 50ms
      - read-only
quality:
  - all functions must be total
traceability:
  known dependencies: api-router, admin-panel
";

    #[test]
    fn test_yaml_to_spec_round_trip() {
        let yaml = YamlParser::new(EXAMPLE_SPEC).parse().unwrap();
        let spec = spec_parser::parse_spec(&yaml).unwrap();

        assert_eq!(spec.spec_id, "SPEC-2024-0042");
        assert_eq!(spec.title, "User service");
        assert_eq!(spec.domain_types.len(), 1);
        assert_eq!(spec.domain_types[0].fields.len(), 3);
        assert_eq!(spec.endpoints.len(), 1);
    }

    #[test]
    fn test_spec_to_pct_produces_valid_output() {
        let yaml = YamlParser::new(EXAMPLE_SPEC).parse().unwrap();
        let spec = spec_parser::parse_spec(&yaml).unwrap();
        let pct = PctEmitter::new().emit(&spec);

        // Must start with (module
        assert!(pct.starts_with("(module user-service"));

        // Must contain key structural elements
        assert!(pct.contains("(type User"));
        assert!(pct.contains("(fn get-user"));
        assert!(pct.contains("(effect-set db-read"));
    }

    #[test]
    fn test_generated_pct_lexes_and_parses() {
        let yaml = YamlParser::new(EXAMPLE_SPEC).parse().unwrap();
        let spec = spec_parser::parse_spec(&yaml).unwrap();
        let pct = PctEmitter::new().emit(&spec);

        let mut lexer = Lexer::new(&pct);
        let tokens = lexer.tokenize().expect("Generated .pct must lex successfully");
        assert!(tokens.len() > 10, "Should produce many tokens");

        let mut parser = Parser::new(tokens);
        let sexprs = parser.parse_program().expect("Generated .pct must parse successfully");
        assert_eq!(sexprs.len(), 1, "Should produce one top-level expression");
    }

    #[test]
    fn test_generated_pct_lowers_to_ast() {
        let yaml = YamlParser::new(EXAMPLE_SPEC).parse().unwrap();
        let spec = spec_parser::parse_spec(&yaml).unwrap();
        let pct = PctEmitter::new().emit(&spec);

        let mut lexer = Lexer::new(&pct);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = Parser::new(tokens);
        let sexprs = parser.parse_program().unwrap();

        let mut lowerer = Lowerer::new();
        let module = lowerer.lower_module(&sexprs[0])
            .expect("Generated .pct must lower to AST successfully");

        assert_eq!(module.name, "user-service");
        assert_eq!(module.types.len(), 1);
        assert_eq!(module.types[0].name, "User");
        assert!(!module.functions.is_empty());
        assert!(module.functions[0].total);
    }
}

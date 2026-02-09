use crate::lexer::{Token, TokenKind, Span};

/// Concrete Syntax Tree node — generic S-expression structure.
/// No semantic knowledge; just balanced structure with atoms.
#[derive(Debug, Clone, PartialEq)]
pub struct SExpr {
    pub kind: SExprKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SExprKind {
    /// (a b c ...)
    List(Vec<SExpr>),
    /// [a b c ...]
    Vector(Vec<SExpr>),
    /// {k1: v1, k2: v2, ...} or {k1 v1 k2 v2 ...}
    Map(Vec<(SExpr, SExpr)>),
    /// Leaf token
    Atom(AtomKind),
}

#[derive(Debug, Clone, PartialEq)]
pub enum AtomKind {
    Symbol(String),
    Keyword(String),
    StringLit(String),
    IntLit(i64),
    BoolLit(bool),
    DurationLit(u64, crate::lexer::DurationUnit),
    RegexLit(String),
}

impl SExpr {
    pub fn new(kind: SExprKind, span: Span) -> Self {
        SExpr { kind, span }
    }

    pub fn as_symbol(&self) -> Option<&str> {
        match &self.kind {
            SExprKind::Atom(AtomKind::Symbol(s)) => Some(s),
            _ => None,
        }
    }

    pub fn as_keyword(&self) -> Option<&str> {
        match &self.kind {
            SExprKind::Atom(AtomKind::Keyword(s)) => Some(s),
            _ => None,
        }
    }

    pub fn as_string(&self) -> Option<&str> {
        match &self.kind {
            SExprKind::Atom(AtomKind::StringLit(s)) => Some(s),
            _ => None,
        }
    }

    pub fn as_int(&self) -> Option<i64> {
        match &self.kind {
            SExprKind::Atom(AtomKind::IntLit(n)) => Some(*n),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match &self.kind {
            SExprKind::Atom(AtomKind::BoolLit(b)) => Some(*b),
            _ => None,
        }
    }

    pub fn as_list(&self) -> Option<&[SExpr]> {
        match &self.kind {
            SExprKind::List(items) => Some(items),
            _ => None,
        }
    }

    pub fn as_vector(&self) -> Option<&[SExpr]> {
        match &self.kind {
            SExprKind::Vector(items) => Some(items),
            _ => None,
        }
    }

    pub fn as_map(&self) -> Option<&[(SExpr, SExpr)]> {
        match &self.kind {
            SExprKind::Map(entries) => Some(entries),
            _ => None,
        }
    }
}

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Parser { tokens, pos: 0 }
    }

    /// Parse all top-level S-expressions
    pub fn parse_program(&mut self) -> Result<Vec<SExpr>, String> {
        let mut exprs = Vec::new();
        while !self.at_eof() {
            exprs.push(self.parse_sexpr()?);
        }
        Ok(exprs)
    }

    fn parse_sexpr(&mut self) -> Result<SExpr, String> {
        match self.peek_kind() {
            TokenKind::LParen => self.parse_list(),
            TokenKind::LBracket => self.parse_vector(),
            TokenKind::LBrace => self.parse_map(),
            TokenKind::RParen | TokenKind::RBracket | TokenKind::RBrace => {
                Err(format!("unexpected closing delimiter at byte {}", self.peek_span().start))
            }
            TokenKind::Eof => {
                Err("unexpected end of input".to_string())
            }
            _ => self.parse_atom(),
        }
    }

    fn parse_list(&mut self) -> Result<SExpr, String> {
        let start = self.peek_span().start;
        self.expect(TokenKind::LParen)?;
        let mut items = Vec::new();
        while !self.check(&TokenKind::RParen) && !self.at_eof() {
            items.push(self.parse_sexpr()?);
        }
        let end_span = self.peek_span();
        self.expect(TokenKind::RParen)?;
        Ok(SExpr::new(SExprKind::List(items), Span::new(start, end_span.end)))
    }

    fn parse_vector(&mut self) -> Result<SExpr, String> {
        let start = self.peek_span().start;
        self.expect(TokenKind::LBracket)?;
        let mut items = Vec::new();
        while !self.check(&TokenKind::RBracket) && !self.at_eof() {
            items.push(self.parse_sexpr()?);
        }
        let end_span = self.peek_span();
        self.expect(TokenKind::RBracket)?;
        Ok(SExpr::new(SExprKind::Vector(items), Span::new(start, end_span.end)))
    }

    fn parse_map(&mut self) -> Result<SExpr, String> {
        let start = self.peek_span().start;
        self.expect(TokenKind::LBrace)?;
        let mut entries = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.at_eof() {
            let key = self.parse_sexpr()?;
            // Skip optional colon separator (for {key: value} syntax)
            if self.check(&TokenKind::Colon) {
                self.advance();
            }
            let value = self.parse_sexpr()?;
            entries.push((key, value));
            // Skip optional comma separator
            if self.check(&TokenKind::Comma) {
                self.advance();
            }
        }
        let end_span = self.peek_span();
        self.expect(TokenKind::RBrace)?;
        Ok(SExpr::new(SExprKind::Map(entries), Span::new(start, end_span.end)))
    }

    fn parse_atom(&mut self) -> Result<SExpr, String> {
        let token = self.advance();
        let span = token.span.clone();
        let kind = match token.kind {
            TokenKind::Symbol(s) => AtomKind::Symbol(s),
            TokenKind::Keyword(s) => AtomKind::Keyword(s),
            TokenKind::StringLit(s) => AtomKind::StringLit(s),
            TokenKind::IntLit(n) => AtomKind::IntLit(n),
            TokenKind::BoolLit(b) => AtomKind::BoolLit(b),
            TokenKind::DurationLit(n, u) => AtomKind::DurationLit(n, u),
            TokenKind::RegexLit(s) => AtomKind::RegexLit(s),
            other => {
                return Err(format!("unexpected token {:?} at byte {}", other, span.start));
            }
        };
        Ok(SExpr::new(SExprKind::Atom(kind), span))
    }

    // --- helpers ---

    fn peek_kind(&self) -> TokenKind {
        self.tokens.get(self.pos).map(|t| t.kind.clone()).unwrap_or(TokenKind::Eof)
    }

    fn peek_span(&self) -> Span {
        self.tokens.get(self.pos).map(|t| t.span.clone()).unwrap_or(Span::new(0, 0))
    }

    fn check(&self, kind: &TokenKind) -> bool {
        std::mem::discriminant(&self.peek_kind()) == std::mem::discriminant(kind)
    }

    fn at_eof(&self) -> bool {
        self.pos >= self.tokens.len() || matches!(self.peek_kind(), TokenKind::Eof)
    }

    fn advance(&mut self) -> Token {
        let token = self.tokens[self.pos].clone();
        self.pos += 1;
        token
    }

    fn expect(&mut self, expected: TokenKind) -> Result<Token, String> {
        if self.check(&expected) {
            Ok(self.advance())
        } else {
            Err(format!(
                "expected {:?}, got {:?} at byte {}",
                expected,
                self.peek_kind(),
                self.peek_span().start
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;

    fn parse(input: &str) -> Vec<SExpr> {
        let mut lexer = Lexer::new(input);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = Parser::new(tokens);
        parser.parse_program().unwrap()
    }

    #[test]
    fn test_simple_list() {
        let result = parse("(foo bar)");
        assert_eq!(result.len(), 1);
        if let SExprKind::List(items) = &result[0].kind {
            assert_eq!(items.len(), 2);
            assert_eq!(items[0].as_symbol(), Some("foo"));
            assert_eq!(items[1].as_symbol(), Some("bar"));
        } else {
            panic!("expected list");
        }
    }

    #[test]
    fn test_nested_list() {
        let result = parse("(a (b c) d)");
        assert_eq!(result.len(), 1);
        if let SExprKind::List(items) = &result[0].kind {
            assert_eq!(items.len(), 3);
            assert_eq!(items[0].as_symbol(), Some("a"));
            assert!(matches!(&items[1].kind, SExprKind::List(_)));
            assert_eq!(items[2].as_symbol(), Some("d"));
        } else {
            panic!("expected list");
        }
    }

    #[test]
    fn test_vector() {
        let result = parse("[1 2 3]");
        assert_eq!(result.len(), 1);
        if let SExprKind::Vector(items) = &result[0].kind {
            assert_eq!(items.len(), 3);
            assert_eq!(items[0].as_int(), Some(1));
            assert_eq!(items[1].as_int(), Some(2));
            assert_eq!(items[2].as_int(), Some(3));
        } else {
            panic!("expected vector");
        }
    }

    #[test]
    fn test_map_colon_syntax() {
        let result = parse(r#"{req: "hello", author: "world"}"#);
        assert_eq!(result.len(), 1);
        if let SExprKind::Map(entries) = &result[0].kind {
            assert_eq!(entries.len(), 2);
            assert_eq!(entries[0].0.as_symbol(), Some("req"));
            assert_eq!(entries[0].1.as_string(), Some("hello"));
            assert_eq!(entries[1].0.as_symbol(), Some("author"));
            assert_eq!(entries[1].1.as_string(), Some("world"));
        } else {
            panic!("expected map");
        }
    }

    #[test]
    fn test_map_keyword_syntax() {
        // Maps can also use keyword keys without colon separator: {:id uuid}
        let result = parse("{:id uuid}");
        assert_eq!(result.len(), 1);
        if let SExprKind::Map(entries) = &result[0].kind {
            assert_eq!(entries.len(), 1);
            assert_eq!(entries[0].0.as_keyword(), Some("id"));
            assert_eq!(entries[0].1.as_symbol(), Some("uuid"));
        } else {
            panic!("expected map");
        }
    }

    #[test]
    fn test_keyword_args() {
        let result = parse("(fn foo :total true :effects [db-read])");
        assert_eq!(result.len(), 1);
        if let SExprKind::List(items) = &result[0].kind {
            assert_eq!(items.len(), 6);
            assert_eq!(items[0].as_symbol(), Some("fn"));
            assert_eq!(items[1].as_symbol(), Some("foo"));
            assert_eq!(items[2].as_keyword(), Some("total"));
            assert_eq!(items[3].as_bool(), Some(true));
            assert_eq!(items[4].as_keyword(), Some("effects"));
            assert!(matches!(&items[5].kind, SExprKind::Vector(_)));
        } else {
            panic!("expected list");
        }
    }

    #[test]
    fn test_complex_nested() {
        let input = r#"(module test
            :version 1
            (type User
                (field id UUID :immutable)))"#;
        let result = parse(input);
        assert_eq!(result.len(), 1);
        if let SExprKind::List(items) = &result[0].kind {
            assert_eq!(items[0].as_symbol(), Some("module"));
            assert_eq!(items[1].as_symbol(), Some("test"));
            assert_eq!(items[2].as_keyword(), Some("version"));
            assert_eq!(items[3].as_int(), Some(1));
            // The type decl is items[4]
            if let SExprKind::List(type_items) = &items[4].kind {
                assert_eq!(type_items[0].as_symbol(), Some("type"));
                assert_eq!(type_items[1].as_symbol(), Some("User"));
            } else {
                panic!("expected type list");
            }
        } else {
            panic!("expected list");
        }
    }

    #[test]
    fn test_full_example_parses() {
        let source = std::fs::read_to_string(
            concat!(env!("CARGO_MANIFEST_DIR"), "/examples/user-service.ais")
        ).unwrap();
        let mut lexer = Lexer::new(&source);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = Parser::new(tokens);
        let result = parser.parse_program().unwrap();
        assert_eq!(result.len(), 1); // one top-level module
        if let SExprKind::List(items) = &result[0].kind {
            assert_eq!(items[0].as_symbol(), Some("module"));
            assert_eq!(items[1].as_symbol(), Some("user-service"));
        } else {
            panic!("expected module list");
        }
    }
}

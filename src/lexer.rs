#[derive(Debug, Clone, PartialEq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Span { start, end }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    LParen,
    RParen,
    LBracket,
    RBracket,
    LBrace,
    RBrace,
    Symbol(String),
    Keyword(String),     // without the leading colon
    StringLit(String),
    IntLit(i64),
    BoolLit(bool),
    DurationLit(u64, DurationUnit),
    RegexLit(String),
    Colon,  // standalone `:` used as map separator in {key: value} syntax
    Comma,  // `,` used as separator in maps
    Eof,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DurationUnit {
    Ms,
    S,
    M,
    H,
}

impl std::fmt::Display for DurationUnit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DurationUnit::Ms => write!(f, "ms"),
            DurationUnit::S => write!(f, "s"),
            DurationUnit::M => write!(f, "m"),
            DurationUnit::H => write!(f, "h"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

impl Token {
    pub fn new(kind: TokenKind, span: Span) -> Self {
        Token { kind, span }
    }
}

pub struct Lexer<'a> {
    source: &'a str,
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(source: &'a str) -> Self {
        Lexer {
            source,
            bytes: source.as_bytes(),
            pos: 0,
        }
    }

    pub fn tokenize(&mut self) -> Result<Vec<Token>, String> {
        let mut tokens = Vec::new();
        loop {
            self.skip_whitespace_and_comments();
            if self.pos >= self.bytes.len() {
                tokens.push(Token::new(TokenKind::Eof, Span::new(self.pos, self.pos)));
                break;
            }
            let token = self.next_token()?;
            tokens.push(token);
        }
        Ok(tokens)
    }

    fn skip_whitespace_and_comments(&mut self) {
        while self.pos < self.bytes.len() {
            let ch = self.bytes[self.pos];
            if ch.is_ascii_whitespace() {
                self.pos += 1;
            } else if self.pos + 1 < self.bytes.len() && ch == b';' && self.bytes[self.pos + 1] == b';' {
                // Line comment: skip to end of line
                while self.pos < self.bytes.len() && self.bytes[self.pos] != b'\n' {
                    self.pos += 1;
                }
            } else {
                break;
            }
        }
    }

    fn next_token(&mut self) -> Result<Token, String> {
        let start = self.pos;
        let ch = self.bytes[self.pos];

        match ch {
            b'(' => { self.pos += 1; Ok(Token::new(TokenKind::LParen, Span::new(start, self.pos))) }
            b')' => { self.pos += 1; Ok(Token::new(TokenKind::RParen, Span::new(start, self.pos))) }
            b'[' => { self.pos += 1; Ok(Token::new(TokenKind::LBracket, Span::new(start, self.pos))) }
            b']' => { self.pos += 1; Ok(Token::new(TokenKind::RBracket, Span::new(start, self.pos))) }
            b'{' => { self.pos += 1; Ok(Token::new(TokenKind::LBrace, Span::new(start, self.pos))) }
            b'}' => { self.pos += 1; Ok(Token::new(TokenKind::RBrace, Span::new(start, self.pos))) }
            b'"' => self.lex_string(),
            b':' => self.lex_keyword_or_colon(),
            b',' => { self.pos += 1; Ok(Token::new(TokenKind::Comma, Span::new(start, self.pos))) }
            b'#' => self.lex_hash(),
            _ if ch.is_ascii_digit() => self.lex_number_or_duration(),
            _ if ch == b'-' && self.peek_next().map_or(false, |c| c.is_ascii_digit()) => self.lex_number_or_duration(),
            _ if is_symbol_start(ch) => self.lex_symbol(),
            _ => {
                // Try UTF-8 character for better error message
                let ch = self.source[self.pos..].chars().next().unwrap_or('?');
                Err(format!("unexpected character '{}' at byte {}", ch, self.pos))
            }
        }
    }

    fn peek_next(&self) -> Option<u8> {
        if self.pos + 1 < self.bytes.len() {
            Some(self.bytes[self.pos + 1])
        } else {
            None
        }
    }

    fn lex_string(&mut self) -> Result<Token, String> {
        let start = self.pos;
        self.pos += 1; // skip opening quote
        let mut value = String::new();
        while self.pos < self.bytes.len() {
            let ch = self.bytes[self.pos];
            match ch {
                b'"' => {
                    self.pos += 1;
                    return Ok(Token::new(TokenKind::StringLit(value), Span::new(start, self.pos)));
                }
                b'\\' => {
                    self.pos += 1;
                    if self.pos >= self.bytes.len() {
                        return Err(format!("unterminated string escape at byte {}", start));
                    }
                    match self.bytes[self.pos] {
                        b'"' => value.push('"'),
                        b'\\' => value.push('\\'),
                        b'n' => value.push('\n'),
                        b't' => value.push('\t'),
                        b'r' => value.push('\r'),
                        other => {
                            return Err(format!("unknown escape '\\{}' at byte {}", other as char, self.pos));
                        }
                    }
                    self.pos += 1;
                }
                _ => {
                    // Handle UTF-8 properly
                    let remaining = &self.source[self.pos..];
                    let c = remaining.chars().next().unwrap();
                    value.push(c);
                    self.pos += c.len_utf8();
                }
            }
        }
        Err(format!("unterminated string starting at byte {}", start))
    }

    fn lex_keyword_or_colon(&mut self) -> Result<Token, String> {
        let start = self.pos;
        self.pos += 1; // skip ':'
        // If next char is not a valid symbol start, it's a standalone colon (map separator)
        if self.pos >= self.bytes.len() || !is_symbol_start(self.bytes[self.pos]) {
            return Ok(Token::new(TokenKind::Colon, Span::new(start, self.pos)));
        }
        // Read the symbol part after the colon
        let sym_start = self.pos;
        while self.pos < self.bytes.len() && is_symbol_cont(self.bytes[self.pos]) {
            self.pos += 1;
        }
        let name = self.source[sym_start..self.pos].to_string();
        Ok(Token::new(TokenKind::Keyword(name), Span::new(start, self.pos)))
    }

    fn lex_hash(&mut self) -> Result<Token, String> {
        let start = self.pos;
        if self.pos + 1 < self.bytes.len() && self.bytes[self.pos + 1] == b'/' {
            // Regex literal: #/pattern/
            self.pos += 2; // skip #/
            let mut pattern = String::new();
            while self.pos < self.bytes.len() {
                let ch = self.bytes[self.pos];
                if ch == b'/' {
                    self.pos += 1;
                    return Ok(Token::new(TokenKind::RegexLit(pattern), Span::new(start, self.pos)));
                } else if ch == b'\\' && self.pos + 1 < self.bytes.len() && self.bytes[self.pos + 1] == b'/' {
                    pattern.push('/');
                    self.pos += 2;
                } else if ch == b'\\' {
                    pattern.push('\\');
                    self.pos += 1;
                    if self.pos < self.bytes.len() {
                        pattern.push(self.bytes[self.pos] as char);
                        self.pos += 1;
                    }
                } else {
                    pattern.push(ch as char);
                    self.pos += 1;
                }
            }
            Err(format!("unterminated regex literal starting at byte {}", start))
        } else {
            Err(format!("unexpected '#' at byte {}", start))
        }
    }

    fn lex_number_or_duration(&mut self) -> Result<Token, String> {
        let start = self.pos;
        let negative = if self.bytes[self.pos] == b'-' {
            self.pos += 1;
            true
        } else {
            false
        };

        let num_start = self.pos;
        while self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_digit() {
            self.pos += 1;
        }

        if self.pos == num_start {
            return Err(format!("expected digit after '-' at byte {}", start));
        }

        let num_str = &self.source[num_start..self.pos];

        // Check for duration suffix
        if self.pos < self.bytes.len() {
            let remaining = &self.source[self.pos..];
            if remaining.starts_with("ms") {
                let value: u64 = num_str.parse().map_err(|e| format!("invalid duration number: {}", e))?;
                self.pos += 2;
                return Ok(Token::new(TokenKind::DurationLit(value, DurationUnit::Ms), Span::new(start, self.pos)));
            }
            // Check single-char suffixes, but only if not followed by symbol chars
            if let Some(&ch) = self.bytes.get(self.pos) {
                let next_after = self.bytes.get(self.pos + 1).copied();
                match ch {
                    b's' if !next_after.map_or(false, is_symbol_cont) => {
                        let value: u64 = num_str.parse().map_err(|e| format!("invalid duration number: {}", e))?;
                        self.pos += 1;
                        return Ok(Token::new(TokenKind::DurationLit(value, DurationUnit::S), Span::new(start, self.pos)));
                    }
                    b'h' if !next_after.map_or(false, is_symbol_cont) => {
                        let value: u64 = num_str.parse().map_err(|e| format!("invalid duration number: {}", e))?;
                        self.pos += 1;
                        return Ok(Token::new(TokenKind::DurationLit(value, DurationUnit::H), Span::new(start, self.pos)));
                    }
                    b'm' if !next_after.map_or(false, is_symbol_cont) => {
                        let value: u64 = num_str.parse().map_err(|e| format!("invalid duration number: {}", e))?;
                        self.pos += 1;
                        return Ok(Token::new(TokenKind::DurationLit(value, DurationUnit::M), Span::new(start, self.pos)));
                    }
                    _ => {}
                }
            }
        }

        // Plain integer
        let full_str = &self.source[start..self.pos];
        let value: i64 = full_str.parse().map_err(|e| format!("invalid integer '{}': {}", full_str, e))?;
        let _ = negative; // already handled by parsing full_str which includes '-'
        Ok(Token::new(TokenKind::IntLit(value), Span::new(start, self.pos)))
    }

    fn lex_symbol(&mut self) -> Result<Token, String> {
        let start = self.pos;
        while self.pos < self.bytes.len() && is_symbol_cont(self.bytes[self.pos]) {
            self.pos += 1;
        }
        let text = &self.source[start..self.pos];

        // Check for bool literals
        match text {
            "true" => Ok(Token::new(TokenKind::BoolLit(true), Span::new(start, self.pos))),
            "false" => Ok(Token::new(TokenKind::BoolLit(false), Span::new(start, self.pos))),
            _ => Ok(Token::new(TokenKind::Symbol(text.to_string()), Span::new(start, self.pos))),
        }
    }
}

fn is_symbol_start(ch: u8) -> bool {
    ch.is_ascii_alphabetic()
        || matches!(ch, b'_' | b'-' | b'+' | b'*' | b'/' | b'!' | b'?' | b'>' | b'<' | b'=' | b'.')
}

fn is_symbol_cont(ch: u8) -> bool {
    is_symbol_start(ch) || ch.is_ascii_digit()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lex(input: &str) -> Vec<TokenKind> {
        let mut lexer = Lexer::new(input);
        lexer.tokenize()
            .unwrap()
            .into_iter()
            .map(|t| t.kind)
            .filter(|k| !matches!(k, TokenKind::Eof))
            .collect()
    }

    #[test]
    fn test_delimiters() {
        assert_eq!(lex("()[]{}"), vec![
            TokenKind::LParen, TokenKind::RParen,
            TokenKind::LBracket, TokenKind::RBracket,
            TokenKind::LBrace, TokenKind::RBrace,
        ]);
    }

    #[test]
    fn test_symbols() {
        assert_eq!(lex("foo bar-baz non-empty? insert!"), vec![
            TokenKind::Symbol("foo".into()),
            TokenKind::Symbol("bar-baz".into()),
            TokenKind::Symbol("non-empty?".into()),
            TokenKind::Symbol("insert!".into()),
        ]);
    }

    #[test]
    fn test_symbol_with_slash() {
        assert_eq!(lex("api-router/handle-request"), vec![
            TokenKind::Symbol("api-router/handle-request".into()),
        ]);
    }

    #[test]
    fn test_keywords() {
        assert_eq!(lex(":provenance :effects :total"), vec![
            TokenKind::Keyword("provenance".into()),
            TokenKind::Keyword("effects".into()),
            TokenKind::Keyword("total".into()),
        ]);
    }

    #[test]
    fn test_strings() {
        assert_eq!(lex(r#""hello" "world""#), vec![
            TokenKind::StringLit("hello".into()),
            TokenKind::StringLit("world".into()),
        ]);
    }

    #[test]
    fn test_string_escapes() {
        assert_eq!(lex(r#""hello\nworld""#), vec![
            TokenKind::StringLit("hello\nworld".into()),
        ]);
    }

    #[test]
    fn test_integers() {
        assert_eq!(lex("42 -7 0"), vec![
            TokenKind::IntLit(42),
            TokenKind::IntLit(-7),
            TokenKind::IntLit(0),
        ]);
    }

    #[test]
    fn test_booleans() {
        assert_eq!(lex("true false"), vec![
            TokenKind::BoolLit(true),
            TokenKind::BoolLit(false),
        ]);
    }

    #[test]
    fn test_durations() {
        assert_eq!(lex("50ms 200ms"), vec![
            TokenKind::DurationLit(50, DurationUnit::Ms),
            TokenKind::DurationLit(200, DurationUnit::Ms),
        ]);
    }

    #[test]
    fn test_regex() {
        assert_eq!(lex(r#"#/.+@.+\..+/"#), vec![
            TokenKind::RegexLit(r".+@.+\..+".into()),
        ]);
    }

    #[test]
    fn test_comments_skipped() {
        assert_eq!(lex(";; this is a comment\nfoo"), vec![
            TokenKind::Symbol("foo".into()),
        ]);
    }

    #[test]
    fn test_simple_list() {
        assert_eq!(lex("(module user-service)"), vec![
            TokenKind::LParen,
            TokenKind::Symbol("module".into()),
            TokenKind::Symbol("user-service".into()),
            TokenKind::RParen,
        ]);
    }

    #[test]
    fn test_keyword_value_pair() {
        assert_eq!(lex(":version 7"), vec![
            TokenKind::Keyword("version".into()),
            TokenKind::IntLit(7),
        ]);
    }

    #[test]
    fn test_field_declaration() {
        assert_eq!(lex("(field name String :min-len 1 :max-len 200)"), vec![
            TokenKind::LParen,
            TokenKind::Symbol("field".into()),
            TokenKind::Symbol("name".into()),
            TokenKind::Symbol("String".into()),
            TokenKind::Keyword("min-len".into()),
            TokenKind::IntLit(1),
            TokenKind::Keyword("max-len".into()),
            TokenKind::IntLit(200),
            TokenKind::RParen,
        ]);
    }

    #[test]
    fn test_map_syntax() {
        assert_eq!(lex(r#"{req: "SPEC-2024-0042", author: "agent"}"#), vec![
            TokenKind::LBrace,
            TokenKind::Symbol("req".into()),
            TokenKind::Colon,
            TokenKind::StringLit("SPEC-2024-0042".into()),
            TokenKind::Comma,
            TokenKind::Symbol("author".into()),
            TokenKind::Colon,
            TokenKind::StringLit("agent".into()),
            TokenKind::RBrace,
        ]);
    }

    #[test]
    fn test_dot_accessor() {
        // (. input email) — dot is a symbol
        assert_eq!(lex("(. input email)"), vec![
            TokenKind::LParen,
            TokenKind::Symbol(".".into()),
            TokenKind::Symbol("input".into()),
            TokenKind::Symbol("email".into()),
            TokenKind::RParen,
        ]);
    }
}

use super::yaml_ast::YamlValue;

/// Hand-written indentation-based YAML subset parser.
///
/// Supports:
/// - Key-value mappings (indentation-based nesting)
/// - Sequences (`- item`)
/// - Scalars (plain + double-quoted strings)
/// - Comments (`#`)
///
/// Does NOT support: anchors, tags, flow collections, multi-doc, block scalars.
pub struct YamlParser {
    lines: Vec<String>,
    pos: usize,
}

#[derive(Debug)]
pub struct YamlParseError {
    pub line: usize,
    pub message: String,
}

impl std::fmt::Display for YamlParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "YAML parse error at line {}: {}", self.line + 1, self.message)
    }
}

impl YamlParser {
    pub fn new(input: &str) -> Self {
        let lines: Vec<String> = input.lines().map(|l| l.to_string()).collect();
        YamlParser { lines, pos: 0 }
    }

    pub fn parse(&mut self) -> Result<YamlValue, YamlParseError> {
        self.skip_blank_and_comments();
        if self.pos >= self.lines.len() {
            return Ok(YamlValue::Mapping(Vec::new()));
        }
        let indent = self.current_indent();
        self.parse_value_at_indent(indent)
    }

    fn parse_value_at_indent(&mut self, indent: usize) -> Result<YamlValue, YamlParseError> {
        self.skip_blank_and_comments();
        if self.pos >= self.lines.len() {
            return Ok(YamlValue::Scalar(String::new()));
        }

        let line = self.current_line_trimmed();

        if line.starts_with("- ") || line == "-" {
            self.parse_sequence(indent)
        } else if line.contains(": ") || line.ends_with(':') {
            self.parse_mapping(indent)
        } else {
            // Plain scalar on its own line
            let val = self.parse_scalar_value(&line);
            self.pos += 1;
            Ok(val)
        }
    }

    fn parse_mapping(&mut self, indent: usize) -> Result<YamlValue, YamlParseError> {
        let mut pairs = Vec::new();

        while self.pos < self.lines.len() {
            self.skip_blank_and_comments();
            if self.pos >= self.lines.len() {
                break;
            }

            let cur_indent = self.current_indent();
            if cur_indent < indent {
                break;
            }
            if cur_indent > indent {
                break;
            }

            let line = self.current_line_trimmed();
            // Must be a key: value or key:
            if let Some((key, rest)) = self.split_key_value(&line) {
                self.pos += 1;

                let value = if rest.is_empty() {
                    // Value is on subsequent indented lines
                    self.skip_blank_and_comments();
                    if self.pos < self.lines.len() {
                        let child_indent = self.current_indent();
                        if child_indent > indent {
                            self.parse_value_at_indent(child_indent)?
                        } else {
                            YamlValue::Scalar(String::new())
                        }
                    } else {
                        YamlValue::Scalar(String::new())
                    }
                } else {
                    self.parse_scalar_value(&rest)
                };

                pairs.push((key, value));
            } else {
                break;
            }
        }

        Ok(YamlValue::Mapping(pairs))
    }

    fn parse_sequence(&mut self, indent: usize) -> Result<YamlValue, YamlParseError> {
        let mut items = Vec::new();

        while self.pos < self.lines.len() {
            self.skip_blank_and_comments();
            if self.pos >= self.lines.len() {
                break;
            }

            let cur_indent = self.current_indent();
            if cur_indent < indent {
                break;
            }
            if cur_indent > indent {
                break;
            }

            let line = self.current_line_trimmed();
            if !line.starts_with("- ") && line != "-" {
                break;
            }

            let after_dash = if line == "-" {
                String::new()
            } else {
                line[2..].to_string()
            };

            self.pos += 1;

            if after_dash.is_empty() {
                // Block sequence item — value on next indented lines
                self.skip_blank_and_comments();
                if self.pos < self.lines.len() {
                    let child_indent = self.current_indent();
                    if child_indent > indent {
                        items.push(self.parse_value_at_indent(child_indent)?);
                    } else {
                        items.push(YamlValue::Scalar(String::new()));
                    }
                } else {
                    items.push(YamlValue::Scalar(String::new()));
                }
            } else if after_dash.contains(": ") || after_dash.ends_with(':') {
                // Inline mapping item in sequence: `- key: value`
                // The content after "- " might be a single key: value or the start of a mapping
                let item_indent = indent + 2; // "- " is 2 chars
                let mut pairs = Vec::new();

                // Parse the first key-value from after_dash
                if let Some((key, rest)) = self.split_key_value(&after_dash) {
                    let value = if rest.is_empty() {
                        self.skip_blank_and_comments();
                        if self.pos < self.lines.len() {
                            let child_indent = self.current_indent();
                            if child_indent > indent {
                                self.parse_value_at_indent(child_indent)?
                            } else {
                                YamlValue::Scalar(String::new())
                            }
                        } else {
                            YamlValue::Scalar(String::new())
                        }
                    } else {
                        self.parse_scalar_value(&rest)
                    };
                    pairs.push((key, value));
                }

                // Parse continuation keys at item_indent
                while self.pos < self.lines.len() {
                    self.skip_blank_and_comments();
                    if self.pos >= self.lines.len() {
                        break;
                    }
                    let ci = self.current_indent();
                    if ci < item_indent {
                        break;
                    }
                    if ci > item_indent {
                        break;
                    }
                    let l = self.current_line_trimmed();
                    if let Some((key, rest)) = self.split_key_value(&l) {
                        self.pos += 1;
                        let value = if rest.is_empty() {
                            self.skip_blank_and_comments();
                            if self.pos < self.lines.len() {
                                let child_indent = self.current_indent();
                                if child_indent > item_indent {
                                    self.parse_value_at_indent(child_indent)?
                                } else {
                                    YamlValue::Scalar(String::new())
                                }
                            } else {
                                YamlValue::Scalar(String::new())
                            }
                        } else {
                            self.parse_scalar_value(&rest)
                        };
                        pairs.push((key, value));
                    } else {
                        break;
                    }
                }

                if pairs.len() == 1 && pairs[0].1 == YamlValue::Scalar(String::new()) {
                    // Just "- key:" with no value, treat as scalar
                    items.push(YamlValue::Scalar(after_dash.to_string()));
                } else {
                    items.push(YamlValue::Mapping(pairs));
                }
            } else {
                items.push(self.parse_scalar_value(&after_dash));
            }
        }

        Ok(YamlValue::Sequence(items))
    }

    fn split_key_value(&self, line: &str) -> Option<(String, String)> {
        // Find the first ": " or trailing ":"
        if let Some(idx) = line.find(": ") {
            let key = line[..idx].trim().to_string();
            let val = line[idx + 2..].trim().to_string();
            Some((key, val))
        } else if line.ends_with(':') {
            let key = line[..line.len() - 1].trim().to_string();
            Some((key, String::new()))
        } else {
            None
        }
    }

    fn parse_scalar_value(&self, s: &str) -> YamlValue {
        let trimmed = s.trim();
        if trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2 {
            // Quoted string — strip quotes
            YamlValue::Scalar(trimmed[1..trimmed.len() - 1].to_string())
        } else {
            YamlValue::Scalar(trimmed.to_string())
        }
    }

    fn current_line_trimmed(&self) -> String {
        self.lines[self.pos].trim().to_string()
    }

    fn current_indent(&self) -> usize {
        if self.pos >= self.lines.len() {
            return 0;
        }
        let line = &self.lines[self.pos];
        line.len() - line.trim_start().len()
    }

    fn skip_blank_and_comments(&mut self) {
        while self.pos < self.lines.len() {
            let trimmed = self.lines[self.pos].trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                self.pos += 1;
            } else {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_mapping() {
        let input = "name: Alice\nage: 30\n";
        let mut parser = YamlParser::new(input);
        let result = parser.parse().unwrap();
        assert_eq!(
            result,
            YamlValue::Mapping(vec![
                ("name".into(), YamlValue::Scalar("Alice".into())),
                ("age".into(), YamlValue::Scalar("30".into())),
            ])
        );
    }

    #[test]
    fn test_quoted_string() {
        let input = "title: \"Hello World\"\n";
        let mut parser = YamlParser::new(input);
        let result = parser.parse().unwrap();
        assert_eq!(
            result,
            YamlValue::Mapping(vec![
                ("title".into(), YamlValue::Scalar("Hello World".into())),
            ])
        );
    }

    #[test]
    fn test_nested_mapping() {
        let input = "domain:\n  User:\n    name: string\n";
        let mut parser = YamlParser::new(input);
        let result = parser.parse().unwrap();
        assert_eq!(
            result,
            YamlValue::Mapping(vec![
                ("domain".into(), YamlValue::Mapping(vec![
                    ("User".into(), YamlValue::Mapping(vec![
                        ("name".into(), YamlValue::Scalar("string".into())),
                    ])),
                ])),
            ])
        );
    }

    #[test]
    fn test_simple_sequence() {
        let input = "items:\n  - alpha\n  - beta\n  - gamma\n";
        let mut parser = YamlParser::new(input);
        let result = parser.parse().unwrap();
        assert_eq!(
            result,
            YamlValue::Mapping(vec![
                ("items".into(), YamlValue::Sequence(vec![
                    YamlValue::Scalar("alpha".into()),
                    YamlValue::Scalar("beta".into()),
                    YamlValue::Scalar("gamma".into()),
                ])),
            ])
        );
    }

    #[test]
    fn test_comments_and_blank_lines() {
        let input = "# This is a comment\n\nname: Alice\n# Another comment\nage: 30\n";
        let mut parser = YamlParser::new(input);
        let result = parser.parse().unwrap();
        assert_eq!(
            result,
            YamlValue::Mapping(vec![
                ("name".into(), YamlValue::Scalar("Alice".into())),
                ("age".into(), YamlValue::Scalar("30".into())),
            ])
        );
    }

    #[test]
    fn test_sequence_of_scalars_inline() {
        let input = "- one\n- two\n- three\n";
        let mut parser = YamlParser::new(input);
        let result = parser.parse().unwrap();
        assert_eq!(
            result,
            YamlValue::Sequence(vec![
                YamlValue::Scalar("one".into()),
                YamlValue::Scalar("two".into()),
                YamlValue::Scalar("three".into()),
            ])
        );
    }

    #[test]
    fn test_sequence_with_key_value_items() {
        let input = "fields:\n  - name: required, string\n  - email: required, email format\n";
        let mut parser = YamlParser::new(input);
        let result = parser.parse().unwrap();
        let fields = result.get("fields").unwrap().as_sequence().unwrap();
        assert_eq!(fields.len(), 2);
        // Each item is a mapping with one key
        match &fields[0] {
            YamlValue::Mapping(pairs) => {
                assert_eq!(pairs[0].0, "name");
                assert_eq!(pairs[0].1, YamlValue::Scalar("required, string".into()));
            }
            other => panic!("Expected Mapping, got {:?}", other),
        }
    }

    #[test]
    fn test_deeply_nested() {
        let input = "a:\n  b:\n    c: deep\n";
        let mut parser = YamlParser::new(input);
        let result = parser.parse().unwrap();
        let c_val = result
            .get("a").unwrap()
            .get("b").unwrap()
            .get("c").unwrap();
        assert_eq!(c_val, &YamlValue::Scalar("deep".into()));
    }

    #[test]
    fn test_empty_input() {
        let input = "";
        let mut parser = YamlParser::new(input);
        let result = parser.parse().unwrap();
        assert_eq!(result, YamlValue::Mapping(Vec::new()));
    }

    #[test]
    fn test_only_comments() {
        let input = "# just comments\n# nothing else\n";
        let mut parser = YamlParser::new(input);
        let result = parser.parse().unwrap();
        assert_eq!(result, YamlValue::Mapping(Vec::new()));
    }

    #[test]
    fn test_inline_comment_handling() {
        // Comments at end of value lines are treated as part of the scalar
        // (our minimal parser doesn't strip inline comments to keep things simple
        //  and avoid breaking quoted strings with # in them)
        let input = "key: value\n";
        let mut parser = YamlParser::new(input);
        let result = parser.parse().unwrap();
        assert_eq!(
            result,
            YamlValue::Mapping(vec![
                ("key".into(), YamlValue::Scalar("value".into())),
            ])
        );
    }

    #[test]
    fn test_multiple_top_level_sequences() {
        let input = "endpoints:\n  get-user:\n    description: \"Returns a user\"\n    input: user id\n";
        let mut parser = YamlParser::new(input);
        let result = parser.parse().unwrap();
        let get_user = result.get("endpoints").unwrap().get("get-user").unwrap();
        assert_eq!(
            get_user.get("description").unwrap(),
            &YamlValue::Scalar("Returns a user".into())
        );
        assert_eq!(
            get_user.get("input").unwrap(),
            &YamlValue::Scalar("user id".into())
        );
    }

    #[test]
    fn test_spec_like_structure() {
        let input = "\
spec: SPEC-2024-0042
title: \"User service\"
owner: platform-team
domain:
  User:
    fields:
      - name: required, string, 1-200 chars
      - email: required, email format, unique
quality:
  - all functions must be total
";
        let mut parser = YamlParser::new(input);
        let result = parser.parse().unwrap();
        assert_eq!(
            result.get("spec").unwrap(),
            &YamlValue::Scalar("SPEC-2024-0042".into())
        );
        assert_eq!(
            result.get("title").unwrap(),
            &YamlValue::Scalar("User service".into())
        );
        let quality = result.get("quality").unwrap().as_sequence().unwrap();
        assert_eq!(quality[0], YamlValue::Scalar("all functions must be total".into()));
    }
}

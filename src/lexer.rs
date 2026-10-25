use crate::{Diagnostic, Span};

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    Name(String),
    Integer(i64),
    Float(f64),
    String(String),
    Return,
    True,
    False,
    And,
    Or,
    Not,
    LParen,
    RParen,
    LBracket,
    RBracket,
    Comma,
    Colon,
    Dot,
    Question,
    Equal,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Caret,
    EqualEqual,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    Newline,
    Indent,
    Dedent,
    Eof,
}

/// Tokenize indentation-sensitive Pepper. Blank/comment lines do not affect indentation;
/// newlines inside parentheses and brackets are ignored. Tabs are rejected.
pub fn lex(source: &str) -> Result<Vec<Token>, Diagnostic> {
    let mut tokens = Vec::new();
    let mut indents = vec![0];
    let mut delimiters: Vec<(char, Span)> = Vec::new();
    let mut offset = 0;
    for line in source.split_inclusive('\n') {
        let text = line.trim_end_matches(['\r', '\n']);
        let bytes = text.as_bytes();
        let mut i = 0;
        while i < bytes.len() && bytes[i] == b' ' {
            i += 1;
        }
        if bytes.get(i) == Some(&b'\t') {
            return Err(Diagnostic::new(
                "use spaces for indentation, not tabs",
                Span {
                    start: offset + i,
                    end: offset + i + 1,
                },
            ));
        }
        if i == bytes.len() || bytes[i] == b'#' {
            offset += line.len();
            continue;
        }
        if delimiters.is_empty() {
            let current = *indents.last().unwrap();
            let span = Span {
                start: offset,
                end: offset + i,
            };
            if i > current {
                indents.push(i);
                tokens.push(Token {
                    kind: TokenKind::Indent,
                    span,
                });
            } else if i < current {
                while i < *indents.last().unwrap() {
                    indents.pop();
                    tokens.push(Token {
                        kind: TokenKind::Dedent,
                        span,
                    });
                }
                if i != *indents.last().unwrap() {
                    return Err(Diagnostic::new(
                        "indentation does not match an outer block",
                        span,
                    ));
                }
            }
        }
        while i < bytes.len() {
            let start = i;
            let ch = text[i..].chars().next().unwrap();
            if ch == ' ' {
                i += 1;
                continue;
            }
            if ch == '#' {
                break;
            }
            let kind = if ch.is_ascii_alphabetic() || ch == '_' {
                i += 1;
                while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                    i += 1;
                }
                match &text[start..i] {
                    "return" => TokenKind::Return,
                    "true" => TokenKind::True,
                    "false" => TokenKind::False,
                    "and" => TokenKind::And,
                    "or" => TokenKind::Or,
                    "not" => TokenKind::Not,
                    name => TokenKind::Name(name.into()),
                }
            } else if ch.is_ascii_digit() {
                i += 1;
                while i < bytes.len() && bytes[i].is_ascii_digit() {
                    i += 1;
                }
                let mut float = false;
                if bytes.get(i) == Some(&b'.') && bytes.get(i + 1).is_some_and(u8::is_ascii_digit) {
                    float = true;
                    i += 1;
                    while i < bytes.len() && bytes[i].is_ascii_digit() {
                        i += 1;
                    }
                }
                if matches!(bytes.get(i), Some(b'e' | b'E')) {
                    float = true;
                    i += 1;
                    if matches!(bytes.get(i), Some(b'+' | b'-')) {
                        i += 1;
                    }
                    let digits = i;
                    while i < bytes.len() && bytes[i].is_ascii_digit() {
                        i += 1;
                    }
                    if i == digits {
                        return Err(Diagnostic::new(
                            "expected exponent digits",
                            Span {
                                start: offset + start,
                                end: offset + i,
                            },
                        ));
                    }
                }
                let span = Span {
                    start: offset + start,
                    end: offset + i,
                };
                if float {
                    let value: f64 = text[start..i]
                        .parse()
                        .map_err(|_| Diagnostic::new("invalid float", span))?;
                    if !value.is_finite() {
                        return Err(Diagnostic::new("float is out of range", span));
                    }
                    TokenKind::Float(value)
                } else {
                    TokenKind::Integer(
                        text[start..i]
                            .parse()
                            .map_err(|_| Diagnostic::new("integer is out of range", span))?,
                    )
                }
            } else if ch == '"' {
                i += 1;
                let mut value = String::new();
                let mut closed = false;
                while i < bytes.len() {
                    let c = text[i..].chars().next().unwrap();
                    i += c.len_utf8();
                    if c == '"' {
                        closed = true;
                        break;
                    }
                    if c == '\\' {
                        let escaped = text[i..].chars().next().ok_or_else(|| {
                            Diagnostic::new(
                                "unterminated string escape",
                                Span {
                                    start: offset + start,
                                    end: offset + i,
                                },
                            )
                        })?;
                        i += escaped.len_utf8();
                        value.push(match escaped {
                            'n' => '\n',
                            'r' => '\r',
                            't' => '\t',
                            '\\' => '\\',
                            '"' => '"',
                            _ => {
                                return Err(Diagnostic::new(
                                    "unsupported string escape",
                                    Span {
                                        start: offset + i - escaped.len_utf8(),
                                        end: offset + i,
                                    },
                                ));
                            }
                        });
                    } else {
                        value.push(c);
                    }
                }
                if !closed {
                    return Err(Diagnostic::new(
                        "unterminated string",
                        Span {
                            start: offset + start,
                            end: offset + i,
                        },
                    ));
                }
                TokenKind::String(value)
            } else {
                i += ch.len_utf8();
                match ch {
                    '(' | '[' => {
                        delimiters.push((
                            ch,
                            Span {
                                start: offset + start,
                                end: offset + i,
                            },
                        ));
                        if ch == '(' {
                            TokenKind::LParen
                        } else {
                            TokenKind::LBracket
                        }
                    }
                    ')' | ']' => {
                        let expected = if ch == ')' { '(' } else { '[' };
                        if delimiters.pop().map(|d| d.0) != Some(expected) {
                            return Err(Diagnostic::new(
                                "mismatched closing delimiter",
                                Span {
                                    start: offset + start,
                                    end: offset + i,
                                },
                            ));
                        }
                        if ch == ')' {
                            TokenKind::RParen
                        } else {
                            TokenKind::RBracket
                        }
                    }
                    ',' => TokenKind::Comma,
                    ':' => TokenKind::Colon,
                    '.' => TokenKind::Dot,
                    '?' => TokenKind::Question,
                    '+' => TokenKind::Plus,
                    '-' => TokenKind::Minus,
                    '*' => TokenKind::Star,
                    '/' => TokenKind::Slash,
                    '%' => TokenKind::Percent,
                    '^' => TokenKind::Caret,
                    '=' | '!' | '<' | '>' => {
                        let equal = bytes.get(i) == Some(&b'=');
                        if equal {
                            i += 1;
                        }
                        match (ch, equal) {
                            ('=', true) => TokenKind::EqualEqual,
                            ('=', false) => TokenKind::Equal,
                            ('!', true) => TokenKind::NotEqual,
                            ('<', true) => TokenKind::LessEqual,
                            ('<', false) => TokenKind::Less,
                            ('>', true) => TokenKind::GreaterEqual,
                            ('>', false) => TokenKind::Greater,
                            _ => {
                                return Err(Diagnostic::new(
                                    "use 'not' for logical negation",
                                    Span {
                                        start: offset + start,
                                        end: offset + i,
                                    },
                                ));
                            }
                        }
                    }
                    _ => {
                        return Err(Diagnostic::new(
                            format!("unexpected character {ch:?}"),
                            Span {
                                start: offset + start,
                                end: offset + i,
                            },
                        ));
                    }
                }
            };
            tokens.push(Token {
                kind,
                span: Span {
                    start: offset + start,
                    end: offset + i,
                },
            });
        }
        if delimiters.is_empty() {
            tokens.push(Token {
                kind: TokenKind::Newline,
                span: Span {
                    start: offset + text.len(),
                    end: offset + line.len(),
                },
            });
        }
        offset += line.len();
    }
    if let Some((_, span)) = delimiters.last() {
        return Err(Diagnostic::new("unclosed delimiter", *span));
    }
    let end = Span {
        start: source.len(),
        end: source.len(),
    };
    while indents.len() > 1 {
        indents.pop();
        tokens.push(Token {
            kind: TokenKind::Dedent,
            span: end,
        });
    }
    tokens.push(Token {
        kind: TokenKind::Eof,
        span: end,
    });
    Ok(tokens)
}

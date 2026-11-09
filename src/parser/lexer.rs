use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    Contract,
    Enum,
    Model,
    Record,
    Service,
    TypeAlias,

    Ident(String),
    StringLit(String),
    DocComment(String),

    LBrace,
    RBrace,
    LParen,
    RParen,
    LBracket,
    RBracket,
    Colon,
    Comma,
    Semicolon,
    Question,
    At,
    Lt,
    Gt,

    Eof,
}

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum LexerError {
    #[error("Unexpected character '{0}' at line {1}, col {2}")]
    UnexpectedChar(char, usize, usize),
    #[error("Unterminated string literal at line {0}, col {1}")]
    UnterminatedString(usize, usize),
}

pub struct Lexer<'a> {
    chars: std::iter::Peekable<std::str::CharIndices<'a>>,
    source: &'a str,
    line: usize,
    col: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(source: &'a str) -> Self {
        Self {
            chars: source.char_indices().peekable(),
            source,
            line: 1,
            col: 1,
        }
    }

    pub fn tokenize(&mut self) -> Result<Vec<(Token, usize, usize)>, LexerError> {
        let mut tokens = Vec::new();

        while let Some(&(idx, ch)) = self.chars.peek() {
            let start_line = self.line;
            let start_col = self.col;

            match ch {
                ' ' | '\t' | '\r' => {
                    self.bump();
                }
                '\n' => {
                    self.bump();
                    self.line += 1;
                    self.col = 1;
                }
                '/' => {
                    self.bump();
                    if let Some(&(_, '/')) = self.chars.peek() {
                        self.bump();
                        let is_doc = if let Some(&(_, '/')) = self.chars.peek() {
                            self.bump();
                            true
                        } else {
                            false
                        };
                        let mut comment_text = String::new();
                        while let Some(&(_, c)) = self.chars.peek() {
                            if c == '\n' {
                                break;
                            }
                            comment_text.push(c);
                            self.bump();
                        }
                        if is_doc {
                            let text = comment_text.trim().to_string();
                            tokens.push((Token::DocComment(text), start_line, start_col));
                        }
                    } else if let Some(&(_, '*')) = self.chars.peek() {
                        self.bump();
                        let is_doc = if let Some(&(_, '*')) = self.chars.peek() {
                            self.bump();
                            true
                        } else {
                            false
                        };
                        let mut comment_text = String::new();
                        while let Some(&(_, c)) = self.chars.peek() {
                            if c == '*' {
                                self.bump();
                                if let Some(&(_, '/')) = self.chars.peek() {
                                    self.bump();
                                    break;
                                } else {
                                    comment_text.push('*');
                                }
                            } else {
                                if c == '\n' {
                                    self.line += 1;
                                    self.col = 0;
                                }
                                comment_text.push(c);
                                self.bump();
                            }
                        }
                        if is_doc {
                            let text = comment_text.trim().to_string();
                            tokens.push((Token::DocComment(text), start_line, start_col));
                        }
                    } else {
                        return Err(LexerError::UnexpectedChar('/', start_line, start_col));
                    }
                }
                '#' => {
                    self.bump();
                    while let Some(&(_, c)) = self.chars.peek() {
                        if c == '\n' {
                            break;
                        }
                        self.bump();
                    }
                }
                ';' => {
                    self.bump();
                    tokens.push((Token::Semicolon, start_line, start_col));
                }
                '{' => {
                    self.bump();
                    tokens.push((Token::LBrace, start_line, start_col));
                }
                '}' => {
                    self.bump();
                    tokens.push((Token::RBrace, start_line, start_col));
                }
                '(' => {
                    self.bump();
                    tokens.push((Token::LParen, start_line, start_col));
                }
                ')' => {
                    self.bump();
                    tokens.push((Token::RParen, start_line, start_col));
                }
                '[' => {
                    self.bump();
                    tokens.push((Token::LBracket, start_line, start_col));
                }
                ']' => {
                    self.bump();
                    tokens.push((Token::RBracket, start_line, start_col));
                }
                ':' => {
                    self.bump();
                    tokens.push((Token::Colon, start_line, start_col));
                }
                ',' => {
                    self.bump();
                    tokens.push((Token::Comma, start_line, start_col));
                }
                '?' => {
                    self.bump();
                    tokens.push((Token::Question, start_line, start_col));
                }
                '@' => {
                    self.bump();
                    tokens.push((Token::At, start_line, start_col));
                }
                '<' => {
                    self.bump();
                    tokens.push((Token::Lt, start_line, start_col));
                }
                '>' => {
                    self.bump();
                    tokens.push((Token::Gt, start_line, start_col));
                }
                '"' => {
                    let s = self.lex_string(start_line, start_col)?;
                    tokens.push((Token::StringLit(s), start_line, start_col));
                }
                c if c.is_alphabetic() || c == '_' => {
                    let ident = self.lex_ident_slice(idx);
                    let tok = match ident {
                        "contract" => Token::Contract,
                        "enum" => Token::Enum,
                        "model" => Token::Model,
                        "record" => Token::Record,
                        "service" => Token::Service,
                        "type" => Token::TypeAlias,
                        _ => Token::Ident(ident.to_string()),
                    };
                    tokens.push((tok, start_line, start_col));
                }
                other => {
                    return Err(LexerError::UnexpectedChar(other, start_line, start_col));
                }
            }
        }

        tokens.push((Token::Eof, self.line, self.col));
        Ok(tokens)
    }

    fn bump(&mut self) -> Option<(usize, char)> {
        let item = self.chars.next();
        if item.is_some() {
            self.col += 1;
        }
        item
    }

    fn lex_ident_slice(&mut self, start_idx: usize) -> &'a str {
        let mut end_idx = start_idx;
        while let Some(&(idx, ch)) = self.chars.peek() {
            if ch.is_alphanumeric() || ch == '_' {
                end_idx = idx + ch.len_utf8();
                self.bump();
            } else {
                break;
            }
        }
        &self.source[start_idx..end_idx]
    }

    fn lex_string(&mut self, start_line: usize, start_col: usize) -> Result<String, LexerError> {
        self.bump();
        let mut result = String::new();

        while let Some(&(_, ch)) = self.chars.peek() {
            if ch == '"' {
                self.bump();
                return Ok(result);
            }
            if ch == '\n' {
                return Err(LexerError::UnterminatedString(start_line, start_col));
            }
            if ch == '\\' {
                self.bump();
                if let Some(&(_, escaped)) = self.chars.peek() {
                    match escaped {
                        'n' => result.push('\n'),
                        't' => result.push('\t'),
                        'r' => result.push('\r'),
                        '\\' => result.push('\\'),
                        '"' => result.push('"'),
                        other => result.push(other),
                    }
                    self.bump();
                }
            } else {
                result.push(ch);
                self.bump();
            }
        }

        Err(LexerError::UnterminatedString(start_line, start_col))
    }
}

//! ash-lexer
//! Converts raw Ash source into a flat token stream with span information.
//! Handles indentation-based block structure by emitting synthetic
//! Indent / Dedent tokens, so the parser never has to think about whitespace.

// ─── Token ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // Literals
    Int(i64),
    Float(f64),
    Str(String),
    Bool(bool),

    // Identifiers & keywords
    Ident(String),
    Fn,
    Let,
    Mut,
    If,
    Else,
    Return,
    While,
    For,
    In,
    Match,
    Type,
    Move,
    Await,
    Panic,
    Use,

    // Arithmetic
    Plus,
    Minus,
    Star,
    Slash,
    Percent,

    // Comparison
    EqEq,  // ==
    NotEq, // !=
    Lt,    // <
    Gt,    // >
    LtEq,  // <=
    GtEq,  // >=

    // Logic
    And, // &&
    Or,  // ||
    Not, // !

    // Assignment / arrows
    Assign,    // =
    FatArrow,  // =>
    ThinArrow, // ->

    // Pipeline & null-safety
    Pipe,         // |>
    Question,     // ?
    NullCoalesce, // ??
    SafeDot,      // ?.
    Bang,         // ! (error propagation, post-fix)

    // Punctuation
    LParen,
    RParen,
    LBracket,
    RBracket,
    LBrace,
    RBrace,
    Comma,
    Colon,
    Dot,
    DotDot, // ..
    Amp,    // & (borrow)
    Pipe1,  // | (union / match arm)

    // Structure (synthetic)
    Newline,
    Indent,
    Dedent,
    Eof,
}

impl std::fmt::Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Token::Int(n) => return write!(f, "int({n})"),
            Token::Float(n) => return write!(f, "float({n})"),
            Token::Str(s) => return write!(f, "\"{}\"", s),
            Token::Bool(b) => return write!(f, "{b}"),
            Token::Ident(s) => return write!(f, "{s}"),
            Token::Fn => "fn",
            Token::Let => "let",
            Token::Mut => "mut",
            Token::If => "if",
            Token::Else => "else",
            Token::Return => "return",
            Token::While => "while",
            Token::For => "for",
            Token::In => "in",
            Token::Match => "match",
            Token::Type => "type",
            Token::Move => "move",
            Token::Await => "await",
            Token::Panic => "panic",
            Token::Use => "use",
            Token::Plus => "+",
            Token::Minus => "-",
            Token::Star => "*",
            Token::Slash => "/",
            Token::Percent => "%",
            Token::EqEq => "==",
            Token::NotEq => "!=",
            Token::Lt => "<",
            Token::Gt => ">",
            Token::LtEq => "<=",
            Token::GtEq => ">=",
            Token::And => "&&",
            Token::Or => "||",
            Token::Not => "!",
            Token::Assign => "=",
            Token::FatArrow => "=>",
            Token::ThinArrow => "->",
            Token::Pipe => "|>",
            Token::Question => "?",
            Token::NullCoalesce => "??",
            Token::SafeDot => "?.",
            Token::Bang => "!",
            Token::LParen => "(",
            Token::RParen => ")",
            Token::LBracket => "[",
            Token::RBracket => "]",
            Token::LBrace => "{",
            Token::RBrace => "}",
            Token::Comma => ",",
            Token::Colon => ":",
            Token::Dot => ".",
            Token::DotDot => "..",
            Token::Amp => "&",
            Token::Pipe1 => "|",
            Token::Newline => "\\n",
            Token::Indent => "<INDENT>",
            Token::Dedent => "<DEDENT>",
            Token::Eof => "<EOF>",
        };
        write!(f, "{s}")
    }
}

// ─── Span ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Span {
    pub line: usize,
    pub col: usize,
    pub len: usize,
}

impl Span {
    pub fn new(line: usize, col: usize, len: usize) -> Self {
        Span { line, col, len }
    }

    /// Merge two spans into one covering both.
    pub fn merge(&self, other: &Span) -> Span {
        Span {
            line: self.line,
            col: self.col,
            len: other.col + other.len - self.col,
        }
    }
}

impl std::fmt::Display for Span {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.line, self.col)
    }
}

// ─── Spanned wrapper ──────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Spanned<T> {
    pub node: T,
    pub span: Span,
}

impl<T> Spanned<T> {
    pub fn new(node: T, span: Span) -> Self {
        Spanned { node, span }
    }
}

pub type SpannedToken = Spanned<Token>;

// ─── Error ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct LexError {
    pub msg: String,
    pub span: Span,
}

impl std::fmt::Display for LexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "error[lex] at {}: {}", self.span, self.msg)
    }
}

// ─── Lexer ────────────────────────────────────────────────────────────────────

pub struct Lexer {
    src: Vec<char>,
    pos: usize,
    line: usize,
    col: usize,
    indent_stack: Vec<usize>,
    /// Tokens queued to be returned before scanning more source.
    pending: Vec<SpannedToken>,
    /// True when we are positioned at the very first character of a line.
    at_line_start: bool,
    /// Nesting depth of (), [], {}.  While > 0, newlines are ignored.
    nesting: usize,
}

impl Lexer {
    pub fn new(source: &str) -> Self {
        Lexer {
            src: source.chars().collect(),
            pos: 0,
            line: 1,
            col: 1,
            indent_stack: vec![0],
            pending: vec![],
            at_line_start: true,
            nesting: 0,
        }
    }

    // ── low-level helpers ────────────────────────────────────────────────────

    fn peek(&self) -> Option<char> {
        self.src.get(self.pos).copied()
    }

    fn peek_at(&self, offset: usize) -> Option<char> {
        self.src.get(self.pos + offset).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let c = self.src.get(self.pos).copied()?;
        self.pos += 1;
        if c == '\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        Some(c)
    }

    fn here(&self) -> Span {
        Span::new(self.line, self.col, 1)
    }

    fn spanned(&self, tok: Token, start_col: usize, start_line: usize) -> SpannedToken {
        let len = if start_line == self.line {
            self.col - start_col
        } else {
            1
        };
        Spanned::new(tok, Span::new(start_line, start_col, len))
    }

    fn err(&self, msg: impl Into<String>) -> LexError {
        LexError {
            msg: msg.into(),
            span: self.here(),
        }
    }

    // ── indentation handling ─────────────────────────────────────────────────

    /// Count spaces/tabs at current position without consuming them.
    fn measure_indent(&self) -> usize {
        let mut i = self.pos;
        let mut count = 0usize;
        while i < self.src.len() {
            match self.src[i] {
                ' ' => {
                    count += 1;
                    i += 1;
                }
                '\t' => {
                    count += 4;
                    i += 1;
                }
                _ => break,
            }
        }
        count
    }

    fn skip_whitespace_inline(&mut self) {
        while matches!(self.peek(), Some(' ') | Some('\t')) {
            self.advance();
        }
    }

    // ── public API ───────────────────────────────────────────────────────────

    /// Tokenise the entire source, returning all tokens including Eof.
    pub fn tokenize(mut self) -> Result<Vec<SpannedToken>, LexError> {
        let mut out = vec![];
        loop {
            let tok = self.next_token()?;
            let done = tok.node == Token::Eof;
            out.push(tok);
            if done {
                break;
            }
        }
        // Flush any remaining dedents
        let cur_indent = *self.indent_stack.last().unwrap();
        if cur_indent > 0 {
            let sp = self.here();
            while self.indent_stack.len() > 1 {
                self.indent_stack.pop();
                out.push(Spanned::new(Token::Dedent, sp.clone()));
            }
        }
        Ok(out)
    }

    fn next_token(&mut self) -> Result<SpannedToken, LexError> {
        // Drain pending queue first (indent / dedent tokens)
        if let Some(t) = self.pending.pop() {
            return Ok(t);
        }

        // Handle indentation at line start (only when not inside brackets)
        if self.at_line_start && self.nesting == 0 {
            self.at_line_start = false;
            let indent = self.measure_indent();
            let cur = *self.indent_stack.last().unwrap();
            let sp = self.here();

            if indent > cur {
                self.indent_stack.push(indent);
                self.skip_whitespace_inline();
                return Ok(Spanned::new(Token::Indent, sp));
            } else if indent < cur {
                self.skip_whitespace_inline();
                while *self.indent_stack.last().unwrap() > indent {
                    self.indent_stack.pop();
                    self.pending.push(Spanned::new(Token::Dedent, sp.clone()));
                }
                // Return the first dedent (rest are in pending)
                if let Some(t) = self.pending.pop() {
                    return Ok(t);
                }
            } else {
                self.skip_whitespace_inline();
            }
        }

        loop {
            match self.peek() {
                None => {
                    // Emit remaining dedents before EOF
                    let sp = self.here();
                    if self.indent_stack.len() > 1 {
                        self.indent_stack.pop();
                        // queue more if needed
                        while self.indent_stack.len() > 1 {
                            self.indent_stack.pop();
                            self.pending.push(Spanned::new(Token::Dedent, sp.clone()));
                        }
                        return Ok(Spanned::new(Token::Dedent, sp));
                    }
                    return Ok(Spanned::new(Token::Eof, sp));
                }
                Some(' ') | Some('\t') => {
                    self.advance();
                }
                Some('\r') => {
                    self.advance();
                }
                Some('#') => {
                    // Line comment — skip to end of line
                    while !matches!(self.peek(), None | Some('\n')) {
                        self.advance();
                    }
                }
                Some('\n') => {
                    self.advance();
                    self.at_line_start = true;
                    // Ignore newlines inside brackets
                    if self.nesting > 0 {
                        continue;
                    }
                    return Ok(Spanned::new(Token::Newline, self.here()));
                }
                _ => break,
            }
        }

        let sl = self.line;
        let sc = self.col;

        match self.peek() {
            None => Ok(Spanned::new(Token::Eof, self.here())),
            Some(c) if c.is_ascii_digit() => self.lex_number(sl, sc),
            Some('"') | Some('\'') => self.lex_string(sl, sc),
            Some(c) if c.is_alphabetic() || c == '_' => self.lex_ident(sl, sc),
            _ => self.lex_symbol(sl, sc),
        }
    }

    // ── number ───────────────────────────────────────────────────────────────

    fn lex_number(&mut self, sl: usize, sc: usize) -> Result<SpannedToken, LexError> {
        let mut s = String::new();
        let mut is_float = false;

        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                s.push(c);
                self.advance();
            } else if c == '.' && self.peek_at(1).map_or(false, |x| x.is_ascii_digit()) {
                is_float = true;
                s.push(c);
                self.advance();
            } else {
                break;
            }
        }

        if is_float {
            let f: f64 = s
                .parse()
                .map_err(|_| self.err(format!("invalid float '{s}'")))?;
            Ok(self.spanned(Token::Float(f), sc, sl))
        } else {
            let i: i64 = s
                .parse()
                .map_err(|_| self.err(format!("invalid int '{s}'")))?;
            Ok(self.spanned(Token::Int(i), sc, sl))
        }
    }

    // ── string ───────────────────────────────────────────────────────────────

    fn lex_string(&mut self, sl: usize, sc: usize) -> Result<SpannedToken, LexError> {
        let quote = self.advance().unwrap();
        let mut s = String::new();
        loop {
            match self.advance() {
                None => return Err(self.err("unterminated string literal")),
                Some(c) if c == quote => break,
                Some('\\') => match self.advance() {
                    Some('n') => s.push('\n'),
                    Some('t') => s.push('\t'),
                    Some('r') => s.push('\r'),
                    Some('\\') => s.push('\\'),
                    Some('"') => s.push('"'),
                    Some('\'') => s.push('\''),
                    Some('{') => s.push('{'),
                    Some(c) => {
                        s.push('\\');
                        s.push(c);
                    }
                    None => return Err(self.err("unexpected EOF in string escape")),
                },
                Some(c) => s.push(c),
            }
        }
        Ok(self.spanned(Token::Str(s), sc, sl))
    }

    // ── identifier / keyword ─────────────────────────────────────────────────

    fn lex_ident(&mut self, sl: usize, sc: usize) -> Result<SpannedToken, LexError> {
        let mut s = String::new();
        while let Some(c) = self.peek() {
            if c.is_alphanumeric() || c == '_' {
                s.push(c);
                self.advance();
            } else {
                break;
            }
        }
        let tok = match s.as_str() {
            "fn" => Token::Fn,
            "let" => Token::Let,
            "mut" => Token::Mut,
            "if" => Token::If,
            "else" => Token::Else,
            "return" => Token::Return,
            "while" => Token::While,
            "for" => Token::For,
            "in" => Token::In,
            "match" => Token::Match,
            "type" => Token::Type,
            "move" => Token::Move,
            "await" => Token::Await,
            "panic" => Token::Panic,
            "use" => Token::Use,
            "true" => Token::Bool(true),
            "false" => Token::Bool(false),
            _ => Token::Ident(s),
        };
        Ok(self.spanned(tok, sc, sl))
    }

    // ── symbols ──────────────────────────────────────────────────────────────

    fn lex_symbol(&mut self, sl: usize, sc: usize) -> Result<SpannedToken, LexError> {
        let c = self.advance().unwrap();
        let tok = match c {
            '+' => Token::Plus,
            '-' => match self.peek() {
                Some('>') => {
                    self.advance();
                    Token::ThinArrow
                }
                _ => Token::Minus,
            },
            '*' => Token::Star,
            '%' => Token::Percent,
            ',' => Token::Comma,
            ':' => Token::Colon,

            '(' => {
                self.nesting += 1;
                Token::LParen
            }
            ')' => {
                if self.nesting > 0 {
                    self.nesting -= 1;
                }
                Token::RParen
            }
            '[' => {
                self.nesting += 1;
                Token::LBracket
            }
            ']' => {
                if self.nesting > 0 {
                    self.nesting -= 1;
                }
                Token::RBracket
            }
            '{' => {
                self.nesting += 1;
                Token::LBrace
            }
            '}' => {
                if self.nesting > 0 {
                    self.nesting -= 1;
                }
                Token::RBrace
            }

            '/' => Token::Slash,

            '=' => match self.peek() {
                Some('=') => {
                    self.advance();
                    Token::EqEq
                }
                Some('>') => {
                    self.advance();
                    Token::FatArrow
                }
                _ => Token::Assign,
            },

            '!' => match self.peek() {
                Some('=') => {
                    self.advance();
                    Token::NotEq
                }
                _ => Token::Bang,
            },

            '<' => match self.peek() {
                Some('=') => {
                    self.advance();
                    Token::LtEq
                }
                _ => Token::Lt,
            },

            '>' => match self.peek() {
                Some('=') => {
                    self.advance();
                    Token::GtEq
                }
                _ => Token::Gt,
            },

            '&' => match self.peek() {
                Some('&') => {
                    self.advance();
                    Token::And
                }
                _ => Token::Amp,
            },
            '|' => match self.peek() {
                Some('|') => {
                    self.advance();
                    Token::Or
                }
                Some('>') => {
                    self.advance();
                    Token::Pipe
                }
                _ => Token::Pipe1,
            },

            '?' => match self.peek() {
                Some('?') => {
                    self.advance();
                    Token::NullCoalesce
                }
                Some('.') => {
                    self.advance();
                    Token::SafeDot
                }
                _ => Token::Question,
            },

            '.' => match self.peek() {
                Some('.') => {
                    self.advance();
                    Token::DotDot
                }
                _ => Token::Dot,
            },

            other => {
                return Err(LexError {
                    msg: format!("unexpected character '{other}'"),
                    span: Span::new(sl, sc, 1),
                })
            }
        };
        Ok(self.spanned(tok, sc, sl))
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn lex(src: &str) -> Vec<Token> {
        Lexer::new(src)
            .tokenize()
            .expect("lex failed")
            .into_iter()
            .map(|t| t.node)
            .filter(|t| !matches!(t, Token::Newline | Token::Eof))
            .collect()
    }

    fn lex_full(src: &str) -> Vec<Token> {
        Lexer::new(src)
            .tokenize()
            .expect("lex failed")
            .into_iter()
            .map(|t| t.node)
            .collect()
    }

    #[test]
    fn test_integers() {
        assert_eq!(lex("42"), vec![Token::Int(42)]);
        assert_eq!(lex("0"), vec![Token::Int(0)]);
        assert_eq!(lex("1000"), vec![Token::Int(1000)]);
    }

    #[test]
    #[allow(clippy::approx_constant)]
    fn test_floats() {
        assert_eq!(lex("3.14"), vec![Token::Float(3.14)]);
        assert_eq!(lex("0.5"), vec![Token::Float(0.5)]);
    }

    #[test]
    fn test_strings() {
        assert_eq!(lex(r#""hello""#), vec![Token::Str("hello".into())]);
        assert_eq!(lex(r#""hi\nworld""#), vec![Token::Str("hi\nworld".into())]);
        assert_eq!(lex("'ash'"), vec![Token::Str("ash".into())]);
    }

    #[test]
    fn test_booleans() {
        assert_eq!(
            lex("true false"),
            vec![Token::Bool(true), Token::Bool(false)]
        );
    }

    #[test]
    fn test_keywords() {
        let src = "fn let mut if else return while for in match type move await panic";
        let toks = lex(src);
        assert_eq!(
            toks,
            vec![
                Token::Fn,
                Token::Let,
                Token::Mut,
                Token::If,
                Token::Else,
                Token::Return,
                Token::While,
                Token::For,
                Token::In,
                Token::Match,
                Token::Type,
                Token::Move,
                Token::Await,
                Token::Panic,
            ]
        );
    }

    #[test]
    fn test_identifiers() {
        assert_eq!(
            lex("foo bar_baz _x y2"),
            vec![
                Token::Ident("foo".into()),
                Token::Ident("bar_baz".into()),
                Token::Ident("_x".into()),
                Token::Ident("y2".into()),
            ]
        );
    }

    #[test]
    fn test_operators() {
        assert_eq!(
            lex("+ - * / %"),
            vec![
                Token::Plus,
                Token::Minus,
                Token::Star,
                Token::Slash,
                Token::Percent,
            ]
        );
        assert_eq!(
            lex("== != < > <= >="),
            vec![
                Token::EqEq,
                Token::NotEq,
                Token::Lt,
                Token::Gt,
                Token::LtEq,
                Token::GtEq,
            ]
        );
        assert_eq!(lex("&& ||"), vec![Token::And, Token::Or]);
        assert_eq!(
            lex("=> -> |>"),
            vec![Token::FatArrow, Token::ThinArrow, Token::Pipe]
        );
        assert_eq!(
            lex("?? ?. ?"),
            vec![Token::NullCoalesce, Token::SafeDot, Token::Question]
        );
        assert_eq!(lex(".."), vec![Token::DotDot]);
    }

    #[test]
    fn test_comments_ignored() {
        assert_eq!(
            lex("x # this is a comment\ny"),
            vec![Token::Ident("x".into()), Token::Ident("y".into()),]
        );
    }

    #[test]
    fn test_indent_dedent() {
        let src = "if x\n    y\nz";
        let toks = lex_full(src);
        assert!(toks.contains(&Token::Indent), "expected Indent token");
        assert!(toks.contains(&Token::Dedent), "expected Dedent token");
    }

    #[test]
    fn test_nested_indent() {
        let src = "a\n    b\n        c\n    d\ne";
        let toks = lex_full(src);
        let indents = toks.iter().filter(|t| **t == Token::Indent).count();
        let dedents = toks.iter().filter(|t| **t == Token::Dedent).count();
        assert_eq!(indents, 2, "expected 2 indents");
        assert_eq!(dedents, 2, "expected 2 dedents");
    }

    #[test]
    fn test_newlines_suppressed_inside_parens() {
        let src = "(\n    x\n    y\n)";
        let toks = lex_full(src);
        assert!(
            !toks.contains(&Token::Newline),
            "newlines should be suppressed inside ()"
        );
    }

    #[test]
    fn test_fn_def_tokens() {
        let src = "fn add(a b)\n    a + b";
        let toks = lex(src);
        assert_eq!(toks[0], Token::Fn);
        assert_eq!(toks[1], Token::Ident("add".into()));
        assert_eq!(toks[2], Token::LParen);
    }

    #[test]
    fn test_lambda_tokens() {
        let src = "x => x + 1";
        let toks = lex(src);
        assert_eq!(
            toks,
            vec![
                Token::Ident("x".into()),
                Token::FatArrow,
                Token::Ident("x".into()),
                Token::Plus,
                Token::Int(1),
            ]
        );
    }

    #[test]
    fn test_pipe() {
        let src = "items |> filter(x => x > 0)";
        let toks = lex(src);
        assert!(toks.contains(&Token::Pipe));
        assert!(toks.contains(&Token::FatArrow));
    }

    #[test]
    fn test_type_annotation() {
        let src = "name:str";
        let toks = lex(src);
        assert_eq!(
            toks,
            vec![
                Token::Ident("name".into()),
                Token::Colon,
                Token::Ident("str".into()),
            ]
        );
    }

    #[test]
    fn test_error_propagation_operator() {
        let src = "file.read(path)!";
        let toks = lex(src);
        assert!(toks.contains(&Token::Bang));
    }

    #[test]
    fn test_string_interpolation_preserved() {
        // Interpolation is resolved at parse time; lexer just returns the raw string
        let src = r#""hello {name}""#;
        let toks = lex(src);
        assert_eq!(toks, vec![Token::Str("hello {name}".into())]);
    }

    #[test]
    fn test_multiline_list_no_indent() {
        // Lists inside [] should not produce indent/dedent tokens
        let src = "[\n    1\n    2\n    3\n]";
        let toks = lex_full(src);
        assert!(!toks.contains(&Token::Indent));
        assert!(!toks.contains(&Token::Dedent));
    }

    #[test]
    fn test_unknown_char_errors() {
        let result = Lexer::new("x @ y").tokenize();
        assert!(result.is_err(), "@ should be an error");
    }
}

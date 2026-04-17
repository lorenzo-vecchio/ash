//! Ash Parser
//! Recursive-descent parser. Consumes a flat token stream from the lexer
//! and produces a typed AST.

pub mod ast;

use ash_lexer::{Span, Spanned, Token};
use ast::*;

// ─── Error ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ParseError {
    pub msg: String,
    pub span: Span,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "error[parse] at {}: {}", self.span, self.msg)
    }
}

type PResult<T> = Result<T, ParseError>;

// ─── Parser ───────────────────────────────────────────────────────────────────

pub struct Parser {
    tokens: Vec<Spanned<Token>>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Spanned<Token>>) -> Self {
        Parser { tokens, pos: 0 }
    }

    fn peek(&self) -> &Token {
        self.tokens.get(self.pos).map(|s| &s.node).unwrap_or(&Token::Eof)
    }

    fn peek_span(&self) -> Span {
        self.tokens.get(self.pos).map(|s| s.span.clone()).unwrap_or_default()
    }

    fn peek2(&self) -> &Token {
        self.tokens.get(self.pos + 1).map(|s| &s.node).unwrap_or(&Token::Eof)
    }

    fn advance(&mut self) -> &Spanned<Token> {
        let t = &self.tokens[self.pos];
        if self.pos + 1 < self.tokens.len() { self.pos += 1; }
        t
    }

    fn expect(&mut self, tok: &Token) -> PResult<Span> {
        if self.peek() == tok {
            Ok(self.advance().span.clone())
        } else {
            Err(self.err(format!("expected '{tok}', got '{}'", self.peek())))
        }
    }

    fn eat(&mut self, tok: &Token) -> bool {
        if self.peek() == tok { self.advance(); true } else { false }
    }

    fn skip_newlines(&mut self) {
        while self.peek() == &Token::Newline { self.advance(); }
    }

    fn err(&self, msg: impl Into<String>) -> ParseError {
        ParseError { msg: msg.into(), span: self.peek_span() }
    }

    fn at_eof(&self) -> bool { self.peek() == &Token::Eof }

    pub fn parse(mut self) -> PResult<Program> {
        let start = self.peek_span();
        self.skip_newlines();
        let mut stmts = vec![];
        while !self.at_eof() {
            stmts.push(self.parse_stmt()?);
            self.skip_newlines();
        }
        Ok(Program { stmts, span: start })
    }

    fn parse_stmt(&mut self) -> PResult<Stmt> {
        let span = self.peek_span();
        let kind = match self.peek() {
            Token::Fn     => StmtKind::FnDef(self.parse_fn_def()?),
            Token::Type   => StmtKind::TypeDef(self.parse_type_def()?),
            Token::Return => {
                self.advance();
                let val = if !matches!(self.peek(), Token::Newline | Token::Eof | Token::Dedent) {
                    Some(self.parse_expr()?)
                } else { None };
                StmtKind::Return(val)
            }
            Token::While  => self.parse_while()?,
            Token::For    => self.parse_for()?,
            Token::Panic  => { self.advance(); StmtKind::Panic(self.parse_expr()?) }
            Token::Let    => self.parse_let()?,
            Token::Mut    => self.parse_let()?,
            _ => {
                // Check for annotated bare assignment: ident: Type = expr
                if let Token::Ident(_) = self.peek().clone() {
                    if self.peek2() == &Token::Colon {
                        let name = self.expect_ident()?;
                        self.expect(&Token::Colon)?;
                        let ty = self.parse_type()?;
                        self.expect(&Token::Assign)?;
                        let value = self.parse_expr()?;
                        return Ok(Stmt { kind: StmtKind::Let { name, ty, mutable: false, value }, span });
                    }
                }
                let expr = self.parse_expr()?;
                if self.peek() == &Token::Assign {
                    self.advance();
                    let value = self.parse_expr()?;
                    StmtKind::Assign { target: expr, value }
                } else {
                    StmtKind::Expr(expr)
                }
            }
        };
        Ok(Stmt { kind, span })
    }

    fn parse_let(&mut self) -> PResult<StmtKind> {
        let mutable = if self.peek() == &Token::Mut { self.advance(); true }
                      else { self.eat(&Token::Let); false };
        let name = self.expect_ident()?;
        let ty = if self.eat(&Token::Colon) { self.parse_type()? } else { AshType::Infer };
        self.expect(&Token::Assign)?;
        let value = self.parse_expr()?;
        Ok(StmtKind::Let { name, ty, mutable, value })
    }

    fn parse_while(&mut self) -> PResult<StmtKind> {
        self.expect(&Token::While)?;
        let cond = self.parse_expr()?;
        let body = self.parse_block()?;
        Ok(StmtKind::While { cond, body })
    }

    fn parse_for(&mut self) -> PResult<StmtKind> {
        self.expect(&Token::For)?;
        let var = self.expect_ident()?;
        self.expect(&Token::In)?;
        let iter = self.parse_expr()?;
        let body = self.parse_block()?;
        Ok(StmtKind::For { var, iter, body })
    }

    fn parse_fn_def(&mut self) -> PResult<FnDef> {
        let span = self.peek_span();
        self.expect(&Token::Fn)?;
        let name = self.expect_ident()?;
        let generics = if self.eat(&Token::LBracket) {
            let mut gs = vec![];
            while self.peek() != &Token::RBracket && !self.at_eof() { gs.push(self.expect_ident()?); }
            self.expect(&Token::RBracket)?;
            gs
        } else { vec![] };
        self.expect(&Token::LParen)?;
        let mut params = vec![];
        while self.peek() != &Token::RParen && !self.at_eof() {
            params.push(self.parse_param()?);
            self.eat(&Token::Comma);
        }
        self.expect(&Token::RParen)?;
        let ret = if self.eat(&Token::Colon) { self.parse_type()? } else { AshType::Infer };
        let body = self.parse_block()?;
        Ok(FnDef { name, generics, params, ret, body, span })
    }

    fn parse_param(&mut self) -> PResult<Param> {
        let span = self.peek_span();
        let borrow  = self.eat(&Token::Amp);
        let mutable = self.eat(&Token::Mut);
        let name    = self.expect_ident()?;
        let ty = if self.eat(&Token::Colon) { self.parse_type()? } else { AshType::Infer };
        Ok(Param { name, ty, mutable, borrow, span })
    }

    fn parse_type_def(&mut self) -> PResult<TypeDef> {
        let span = self.peek_span();
        self.expect(&Token::Type)?;
        let name = self.expect_ident()?;
        let generics = if self.eat(&Token::LBracket) {
            let mut gs = vec![];
            while self.peek() != &Token::RBracket && !self.at_eof() { gs.push(self.expect_ident()?); }
            self.expect(&Token::RBracket)?;
            gs
        } else { vec![] };
        if self.eat(&Token::Assign) {
            let variants = self.parse_union_variants()?;
            return Ok(TypeDef { name, generics, kind: TypeDefKind::Union(variants), span });
        }
        let fields = self.parse_struct_fields()?;
        Ok(TypeDef { name, generics, kind: TypeDefKind::Struct(fields), span })
    }

    fn parse_union_variants(&mut self) -> PResult<Vec<UnionVariant>> {
        let mut variants = vec![];
        loop {
            let span = self.peek_span();
            let name = self.expect_ident()?;
            let fields = if self.eat(&Token::LParen) {
                let mut ts = vec![];
                while self.peek() != &Token::RParen && !self.at_eof() {
                    ts.push(self.parse_type()?);
                    self.eat(&Token::Comma);
                }
                self.expect(&Token::RParen)?;
                ts
            } else { vec![] };
            variants.push(UnionVariant { name, fields, span });
            if !self.eat(&Token::Pipe1) { break; }
            self.skip_newlines();
        }
        Ok(variants)
    }

    fn parse_struct_fields(&mut self) -> PResult<Vec<FieldDef>> {
        self.skip_newlines();
        if !self.eat(&Token::Indent) { return Ok(vec![]); }
        let mut fields = vec![];
        loop {
            self.skip_newlines();
            if matches!(self.peek(), Token::Dedent | Token::Eof) { break; }
            let span = self.peek_span();
            let name = self.expect_ident()?;
            self.expect(&Token::Colon)?;
            let ty = self.parse_type()?;
            fields.push(FieldDef { name, ty, span });
        }
        self.eat(&Token::Dedent);
        Ok(fields)
    }

    fn parse_block(&mut self) -> PResult<Block> {
        let span = self.peek_span();
        self.skip_newlines();
        self.expect(&Token::Indent)?;
        let mut stmts = vec![];
        self.skip_newlines();
        while !matches!(self.peek(), Token::Dedent | Token::Eof) {
            stmts.push(self.parse_stmt()?);
            self.skip_newlines();
        }
        self.eat(&Token::Dedent);
        Ok(Block { stmts, span })
    }

    fn parse_type(&mut self) -> PResult<AshType> {
        match self.peek().clone() {
            Token::Question => {
                self.advance();
                Ok(AshType::Option(Box::new(self.parse_type()?)))
            }
            Token::LBracket => {
                self.advance();
                let inner = self.parse_type()?;
                self.expect(&Token::RBracket)?;
                Ok(AshType::List(Box::new(inner)))
            }
            Token::LBrace => {
                self.advance();
                let k = self.parse_type()?;
                self.expect(&Token::Colon)?;
                let v = self.parse_type()?;
                self.expect(&Token::RBrace)?;
                Ok(AshType::Map(Box::new(k), Box::new(v)))
            }
            Token::LParen => {
                self.advance();
                let mut ts = vec![];
                while self.peek() != &Token::RParen && !self.at_eof() {
                    ts.push(self.parse_type()?);
                    self.eat(&Token::Comma);
                }
                self.expect(&Token::RParen)?;
                Ok(AshType::Tuple(ts))
            }
            Token::Ident(name) => {
                self.advance();
                if self.eat(&Token::LBracket) {
                    while self.peek() != &Token::RBracket && !self.at_eof() { self.parse_type()?; }
                    self.expect(&Token::RBracket)?;
                    return Ok(AshType::Named(name));
                }
                if name.len() == 1 && name.chars().next().map_or(false, |c| c.is_uppercase()) {
                    return Ok(AshType::Generic(name));
                }
                Ok(match name.as_str() {
                    "int"   => AshType::Int,
                    "float" => AshType::Float,
                    "bool"  => AshType::Bool,
                    "str"   => AshType::Str,
                    "void"  => AshType::Void,
                    _       => AshType::Named(name),
                })
            }
            _ => Err(self.err(format!("expected type, got '{}'", self.peek()))),
        }
    }

    fn parse_expr(&mut self) -> PResult<Expr> {
        self.parse_lambda_or_binop()
    }

    fn parse_lambda_or_binop(&mut self) -> PResult<Expr> {
        // Single-param lambda: ident =>
        if matches!(self.peek(), Token::Ident(_)) && self.peek2() == &Token::FatArrow {
            let span = self.peek_span();
            let name = self.expect_ident()?;
            self.expect(&Token::FatArrow)?;
            let body = self.parse_expr()?;
            let param = Param { name, ty: AshType::Infer, mutable: false, borrow: false, span: span.clone() };
            return Ok(Expr { kind: ExprKind::Lambda { params: vec![param], body: Box::new(body) }, span });
        }
        // Multi-param lambda: (params) =>
        if self.peek() == &Token::LParen {
            let saved = self.pos;
            if let Ok((params, span)) = self.try_parse_lambda_params() {
                if self.peek() == &Token::FatArrow {
                    self.advance();
                    let body = self.parse_expr()?;
                    return Ok(Expr { kind: ExprKind::Lambda { params, body: Box::new(body) }, span });
                }
            }
            self.pos = saved;
        }
        self.parse_or()
    }

    fn try_parse_lambda_params(&mut self) -> PResult<(Vec<Param>, Span)> {
        let span = self.peek_span();
        self.expect(&Token::LParen)?;
        let mut params = vec![];
        while self.peek() != &Token::RParen && !self.at_eof() {
            let ps = self.peek_span();
            let name = self.expect_ident()?;
            let ty = if self.eat(&Token::Colon) { self.parse_type()? } else { AshType::Infer };
            params.push(Param { name, ty, mutable: false, borrow: false, span: ps });
            self.eat(&Token::Comma);
        }
        self.expect(&Token::RParen)?;
        Ok((params, span))
    }

    fn parse_or(&mut self) -> PResult<Expr> {
        let mut lhs = self.parse_and()?;
        while self.peek() == &Token::Or {
            let span = self.peek_span();
            self.advance();
            let rhs = self.parse_and()?;
            lhs = Expr { kind: ExprKind::BinOp { op: BinOp::Or, lhs: Box::new(lhs), rhs: Box::new(rhs) }, span };
        }
        // Range: start..end — lower precedence than arithmetic
        if self.peek() == &Token::DotDot {
            let span = self.peek_span();
            self.advance();
            let rhs = self.parse_and()?;
            lhs = Expr { kind: ExprKind::Range { start: Box::new(lhs), end: Box::new(rhs) }, span };
        }
        self.parse_pipeline_or_null(lhs)
    }

    fn parse_pipeline_or_null(&mut self, mut lhs: Expr) -> PResult<Expr> {
        loop {
            match self.peek() {
                Token::Pipe => {
                    let span = self.peek_span();
                    self.advance();
                    let rhs = self.parse_and()?;
                    lhs = Expr { kind: ExprKind::Pipe { lhs: Box::new(lhs), rhs: Box::new(rhs) }, span };
                }
                Token::NullCoalesce => {
                    let span = self.peek_span();
                    self.advance();
                    let rhs = self.parse_and()?;
                    lhs = Expr { kind: ExprKind::NullCoalesce { lhs: Box::new(lhs), rhs: Box::new(rhs) }, span };
                }
                _ => break,
            }
        }
        Ok(lhs)
    }

    fn parse_and(&mut self) -> PResult<Expr> {
        let mut lhs = self.parse_equality()?;
        while self.peek() == &Token::And {
            let span = self.peek_span();
            self.advance();
            let rhs = self.parse_equality()?;
            lhs = Expr { kind: ExprKind::BinOp { op: BinOp::And, lhs: Box::new(lhs), rhs: Box::new(rhs) }, span };
        }
        Ok(lhs)
    }

    fn parse_equality(&mut self) -> PResult<Expr> {
        let mut lhs = self.parse_comparison()?;
        loop {
            let op = match self.peek() { Token::EqEq => BinOp::Eq, Token::NotEq => BinOp::NotEq, _ => break };
            let span = self.peek_span(); self.advance();
            let rhs = self.parse_comparison()?;
            lhs = Expr { kind: ExprKind::BinOp { op, lhs: Box::new(lhs), rhs: Box::new(rhs) }, span };
        }
        Ok(lhs)
    }

    fn parse_comparison(&mut self) -> PResult<Expr> {
        let mut lhs = self.parse_add()?;
        loop {
            let op = match self.peek() {
                Token::Lt => BinOp::Lt, Token::Gt => BinOp::Gt,
                Token::LtEq => BinOp::LtEq, Token::GtEq => BinOp::GtEq, _ => break,
            };
            let span = self.peek_span(); self.advance();
            let rhs = self.parse_add()?;
            lhs = Expr { kind: ExprKind::BinOp { op, lhs: Box::new(lhs), rhs: Box::new(rhs) }, span };
        }
        Ok(lhs)
    }

    fn parse_add(&mut self) -> PResult<Expr> {
        let mut lhs = self.parse_mul()?;
        loop {
            let op = match self.peek() { Token::Plus => BinOp::Add, Token::Minus => BinOp::Sub, _ => break };
            let span = self.peek_span(); self.advance();
            let rhs = self.parse_mul()?;
            lhs = Expr { kind: ExprKind::BinOp { op, lhs: Box::new(lhs), rhs: Box::new(rhs) }, span };
        }
        Ok(lhs)
    }

    fn parse_mul(&mut self) -> PResult<Expr> {
        let mut lhs = self.parse_unary()?;
        loop {
            let op = match self.peek() {
                Token::Star => BinOp::Mul, Token::Slash => BinOp::Div, Token::Percent => BinOp::Mod, _ => break,
            };
            let span = self.peek_span(); self.advance();
            let rhs = self.parse_unary()?;
            lhs = Expr { kind: ExprKind::BinOp { op, lhs: Box::new(lhs), rhs: Box::new(rhs) }, span };
        }
        Ok(lhs)
    }

    fn parse_unary(&mut self) -> PResult<Expr> {
        let span = self.peek_span();
        if self.eat(&Token::Minus) {
            return Ok(Expr { kind: ExprKind::UnOp { op: UnOp::Neg, expr: Box::new(self.parse_unary()?) }, span });
        }
        if self.eat(&Token::Bang) {
            return Ok(Expr { kind: ExprKind::UnOp { op: UnOp::Not, expr: Box::new(self.parse_unary()?) }, span });
        }
        if self.eat(&Token::Amp) {
            return Ok(Expr { kind: ExprKind::Borrow(Box::new(self.parse_unary()?)), span });
        }
        if self.eat(&Token::Await) {
            return Ok(Expr { kind: ExprKind::Await(Box::new(self.parse_unary()?)), span });
        }
        if self.eat(&Token::Move) {
            return Ok(Expr { kind: ExprKind::Move(Box::new(self.parse_unary()?)), span });
        }
        self.parse_postfix()
    }

    fn parse_postfix(&mut self) -> PResult<Expr> {
        let mut expr = self.parse_primary()?;
        loop {
            let span = self.peek_span();
            match self.peek().clone() {
                Token::Dot => {
                    self.advance();
                    let field = self.expect_ident_or_keyword()?;
                    if self.peek() == &Token::LParen {
                        let args = self.parse_arg_list()?;
                        let callee = Box::new(Expr { kind: ExprKind::Field { obj: Box::new(expr), field }, span: span.clone() });
                        expr = Expr { kind: ExprKind::Call { callee, args }, span };
                    } else {
                        expr = Expr { kind: ExprKind::Field { obj: Box::new(expr), field }, span };
                    }
                }
                Token::SafeDot => {
                    self.advance();
                    let field = self.expect_ident()?;
                    expr = Expr { kind: ExprKind::SafeField { obj: Box::new(expr), field }, span };
                }
                Token::LBracket => {
                    self.advance();
                    let index = self.parse_expr()?;
                    self.expect(&Token::RBracket)?;
                    expr = Expr { kind: ExprKind::Index { obj: Box::new(expr), index: Box::new(index) }, span };
                }
                Token::LParen => {
                    let args = self.parse_arg_list()?;
                    expr = Expr { kind: ExprKind::Call { callee: Box::new(expr), args }, span };
                }
                Token::Bang => {
                    self.advance();
                    expr = Expr { kind: ExprKind::Propagate(Box::new(expr)), span };
                }
                _ => break,
            }
        }
        Ok(expr)
    }

    fn parse_arg_list(&mut self) -> PResult<Vec<Expr>> {
        self.expect(&Token::LParen)?;
        let mut args = vec![];
        while self.peek() != &Token::RParen && !self.at_eof() {
            args.push(self.parse_expr()?);
            self.eat(&Token::Comma);
        }
        self.expect(&Token::RParen)?;
        Ok(args)
    }

    fn parse_primary(&mut self) -> PResult<Expr> {
        let span = self.peek_span();
        match self.peek().clone() {
            Token::Int(n)   => { self.advance(); Ok(Expr { kind: ExprKind::Int(n),   span }) }
            Token::Float(n) => { self.advance(); Ok(Expr { kind: ExprKind::Float(n), span }) }
            Token::Bool(b)  => { self.advance(); Ok(Expr { kind: ExprKind::Bool(b),  span }) }
            Token::Str(s)   => { self.advance(); Ok(Expr { kind: ExprKind::Str(s),   span }) }
            Token::Ident(_) => {
                let name = self.expect_ident()?;
                // Struct literal: TypeName { field: val, ... }
                // Only if { is on the same line (no Newline token between)
                if self.peek() == &Token::LBrace {
                    return self.parse_struct_lit(name, span);
                }
                Ok(Expr { kind: ExprKind::Ident(name), span })
            }
            Token::LParen   => {
                self.advance();
                if self.peek() == &Token::RParen { self.advance(); return Ok(Expr { kind: ExprKind::Tuple(vec![]), span }); }
                let first = self.parse_expr()?;
                if self.eat(&Token::Comma) {
                    let mut elems = vec![first];
                    while self.peek() != &Token::RParen && !self.at_eof() { elems.push(self.parse_expr()?); self.eat(&Token::Comma); }
                    self.expect(&Token::RParen)?;
                    Ok(Expr { kind: ExprKind::Tuple(elems), span })
                } else { self.expect(&Token::RParen)?; Ok(first) }
            }
            Token::LBracket => {
                self.advance();
                let mut elems = vec![];
                while self.peek() != &Token::RBracket && !self.at_eof() { elems.push(self.parse_expr()?); self.eat(&Token::Comma); }
                self.expect(&Token::RBracket)?;
                Ok(Expr { kind: ExprKind::List(elems), span })
            }
            Token::LBrace => {
                self.advance();
                let mut pairs = vec![];
                while self.peek() != &Token::RBrace && !self.at_eof() {
                    self.skip_newlines();
                    if self.peek() == &Token::RBrace { break; }
                    let key = self.parse_expr()?;
                    self.expect(&Token::Colon)?;
                    let val = self.parse_expr()?;
                    pairs.push((key, val));
                    self.eat(&Token::Comma);
                    self.skip_newlines();
                }
                self.expect(&Token::RBrace)?;
                Ok(Expr { kind: ExprKind::Map(pairs), span })
            }
            Token::If    => self.parse_if_expr(),
            Token::Match => self.parse_match_expr(),
            _ => Err(self.err(format!("unexpected token '{}' in expression", self.peek()))),
        }
    }

    fn parse_if_expr(&mut self) -> PResult<Expr> {
        let span = self.peek_span();
        self.expect(&Token::If)?;
        let cond = self.parse_expr()?;
        let then = self.parse_block()?;
        let else_ = if self.eat(&Token::Else) {
            if self.peek() == &Token::If {
                Some(Box::new(self.parse_if_expr()?))
            } else {
                let block = self.parse_block()?;
                Some(Box::new(Expr { kind: ExprKind::Block(block), span: span.clone() }))
            }
        } else { None };
        Ok(Expr { kind: ExprKind::If { cond: Box::new(cond), then: Box::new(then), else_ }, span })
    }

    fn parse_match_expr(&mut self) -> PResult<Expr> {
        let span = self.peek_span();
        self.expect(&Token::Match)?;
        let scrutinee = self.parse_expr()?;
        self.skip_newlines();
        self.expect(&Token::Indent)?;
        let mut arms = vec![];
        self.skip_newlines();
        while !matches!(self.peek(), Token::Dedent | Token::Eof) {
            arms.push(self.parse_match_arm()?);
            self.skip_newlines();
        }
        self.eat(&Token::Dedent);
        Ok(Expr { kind: ExprKind::Match { scrutinee: Box::new(scrutinee), arms }, span })
    }

    fn parse_match_arm(&mut self) -> PResult<MatchArm> {
        let span = self.peek_span();
        let pattern = self.parse_pattern()?;
        self.expect(&Token::FatArrow)?;
        let body = self.parse_expr()?;
        Ok(MatchArm { pattern, body, span })
    }

    fn parse_pattern(&mut self) -> PResult<Pattern> {
        match self.peek().clone() {
            Token::Ident(name) if name == "_" => { self.advance(); Ok(Pattern::Wildcard) }
            Token::Ident(name) => {
                self.advance();
                if self.eat(&Token::LParen) {
                    let mut inner = vec![];
                    while self.peek() != &Token::RParen && !self.at_eof() { inner.push(self.parse_pattern()?); self.eat(&Token::Comma); }
                    self.expect(&Token::RParen)?;
                    Ok(Pattern::Variant(name, inner))
                } else if self.peek() == &Token::LBrace {
                    // Struct pattern: Point { x, y } or Point { x: 0, y: 0 }
                    self.advance();
                    let mut fields = vec![];
                    while self.peek() != &Token::RBrace && !self.at_eof() {
                        let field_name = self.expect_ident()?;
                        let pat = if self.eat(&Token::Colon) {
                            self.parse_pattern()?
                        } else {
                            Pattern::Ident(field_name.clone())
                        };
                        fields.push((field_name, pat));
                        self.eat(&Token::Comma);
                    }
                    self.expect(&Token::RBrace)?;
                    Ok(Pattern::Struct(name, fields))
                } else { Ok(Pattern::Ident(name)) }
            }
            Token::LParen => {
                // Tuple pattern: (a, b)
                self.advance();
                let mut pats = vec![];
                while self.peek() != &Token::RParen && !self.at_eof() {
                    pats.push(self.parse_pattern()?);
                    self.eat(&Token::Comma);
                }
                self.expect(&Token::RParen)?;
                Ok(Pattern::Tuple(pats))
            }
            Token::Int(n)   => { self.advance(); Ok(Pattern::Literal(LitPattern::Int(n))) }
            Token::Float(n) => { self.advance(); Ok(Pattern::Literal(LitPattern::Float(n))) }
            Token::Str(s)   => { self.advance(); Ok(Pattern::Literal(LitPattern::Str(s))) }
            Token::Bool(b)  => { self.advance(); Ok(Pattern::Literal(LitPattern::Bool(b))) }
            _ => Err(self.err(format!("expected pattern, got '{}'", self.peek()))),
        }
    }

    fn parse_struct_lit(&mut self, name: String, span: Span) -> PResult<Expr> {
        self.expect(&Token::LBrace)?;
        let mut fields = vec![];
        while self.peek() != &Token::RBrace && !self.at_eof() {
            self.skip_newlines();
            if self.peek() == &Token::RBrace { break; }
            let field_name = self.expect_ident()?;
            self.expect(&Token::Colon)?;
            let val = self.parse_expr()?;
            fields.push((field_name, val));
            self.eat(&Token::Comma);
            self.skip_newlines();
        }
        self.expect(&Token::RBrace)?;
        Ok(Expr { kind: ExprKind::StructLit { name, fields }, span })
    }

    fn expect_ident(&mut self) -> PResult<String> {
        match self.peek().clone() {
            Token::Ident(s) => { self.advance(); Ok(s) }
            _ => Err(self.err(format!("expected identifier, got '{}'", self.peek()))),
        }
    }

    /// Like expect_ident but also accepts reserved keywords as method names after `.`
    /// e.g. re.match, map.type, list.in
    fn expect_ident_or_keyword(&mut self) -> PResult<String> {
        match self.peek().clone() {
            Token::Ident(s) => { self.advance(); Ok(s) }
            Token::Match    => { self.advance(); Ok("match".into()) }
            Token::For      => { self.advance(); Ok("for".into()) }
            Token::In       => { self.advance(); Ok("in".into()) }
            Token::Type     => { self.advance(); Ok("type".into()) }
            Token::Let      => { self.advance(); Ok("let".into()) }
            Token::Mut      => { self.advance(); Ok("mut".into()) }
            Token::If       => { self.advance(); Ok("if".into()) }
            Token::Else     => { self.advance(); Ok("else".into()) }
            _ => Err(self.err(format!("expected identifier, got '{}'", self.peek()))),
        }
    }
}

pub fn parse(tokens: Vec<Spanned<Token>>) -> PResult<Program> {
    Parser::new(tokens).parse()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ash_lexer::Lexer;

    fn parse_src(src: &str) -> Program {
        let tokens = Lexer::new(src).tokenize().expect("lex failed");
        parse(tokens).expect("parse failed")
    }

    #[test]
    fn test_explicit_let() {
        let p = parse_src("let x = 42");
        assert!(matches!(p.stmts[0].kind, StmtKind::Let { mutable: false, .. }));
    }

    #[test]
    fn test_mut_binding() {
        let p = parse_src("mut x = 42");
        assert!(matches!(p.stmts[0].kind, StmtKind::Let { mutable: true, .. }));
    }

    #[test]
    fn test_typed_binding() {
        let p = parse_src("let x:int = 42");
        if let StmtKind::Let { ty, .. } = &p.stmts[0].kind {
            assert_eq!(*ty, AshType::Int);
        } else { panic!("expected let"); }
    }

    #[test]
    fn test_fn_def() {
        let p = parse_src("fn add(a b)\n    a + b");
        if let StmtKind::FnDef(f) = &p.stmts[0].kind {
            assert_eq!(f.name, "add");
            assert_eq!(f.params.len(), 2);
        } else { panic!("expected fn"); }
    }

    #[test]
    fn test_fn_with_return_type() {
        let p = parse_src("fn double(x:int):int\n    x * 2");
        if let StmtKind::FnDef(f) = &p.stmts[0].kind {
            assert_eq!(f.ret, AshType::Int);
        }
    }

    #[test]
    fn test_fn_with_generics() {
        let p = parse_src("fn first[T](items:[T]):T\n    items[0]");
        if let StmtKind::FnDef(f) = &p.stmts[0].kind {
            assert_eq!(f.generics, vec!["T"]);
        }
    }

    #[test]
    fn test_single_param_lambda() {
        let p = parse_src("x => x + 1");
        assert!(matches!(p.stmts[0].kind, StmtKind::Expr(Expr { kind: ExprKind::Lambda { .. }, .. })));
    }

    #[test]
    fn test_multi_param_lambda() {
        let p = parse_src("(x y) => x + y");
        if let StmtKind::Expr(Expr { kind: ExprKind::Lambda { params, .. }, .. }) = &p.stmts[0].kind {
            assert_eq!(params.len(), 2);
        } else { panic!("expected lambda"); }
    }

    #[test]
    fn test_pipeline() {
        let p = parse_src("items |> filter(x => x > 0)");
        assert!(matches!(p.stmts[0].kind, StmtKind::Expr(Expr { kind: ExprKind::Pipe { .. }, .. })));
    }

    #[test]
    fn test_null_coalesce() {
        let p = parse_src("name ?? \"anon\"");
        assert!(matches!(p.stmts[0].kind, StmtKind::Expr(Expr { kind: ExprKind::NullCoalesce { .. }, .. })));
    }

    #[test]
    fn test_safe_navigation() {
        let p = parse_src("user?.name");
        assert!(matches!(p.stmts[0].kind, StmtKind::Expr(Expr { kind: ExprKind::SafeField { .. }, .. })));
    }

    #[test]
    fn test_error_propagation() {
        let p = parse_src("file.read(path)!");
        assert!(matches!(p.stmts[0].kind, StmtKind::Expr(Expr { kind: ExprKind::Propagate(_), .. })));
    }

    #[test]
    fn test_if_expr() {
        let p = parse_src("if x > 0\n    x\nelse\n    0");
        assert!(matches!(p.stmts[0].kind, StmtKind::Expr(Expr { kind: ExprKind::If { .. }, .. })));
    }

    #[test]
    fn test_match_expr() {
        let p = parse_src("match result\n    Ok(v) => v\n    Err(e) => 0");
        if let StmtKind::Expr(Expr { kind: ExprKind::Match { arms, .. }, .. }) = &p.stmts[0].kind {
            assert_eq!(arms.len(), 2);
        } else { panic!("expected match"); }
    }

    #[test]
    fn test_while_loop() {
        let p = parse_src("while x > 0\n    x = x - 1");
        assert!(matches!(p.stmts[0].kind, StmtKind::While { .. }));
    }

    #[test]
    fn test_for_loop() {
        let p = parse_src("for x in items\n    println(x)");
        assert!(matches!(p.stmts[0].kind, StmtKind::For { .. }));
    }

    #[test]
    fn test_type_def_struct() {
        let p = parse_src("type Point\n    x:float\n    y:float");
        if let StmtKind::TypeDef(td) = &p.stmts[0].kind {
            assert_eq!(td.name, "Point");
            assert!(matches!(td.kind, TypeDefKind::Struct(_)));
            if let TypeDefKind::Struct(fields) = &td.kind {
                assert_eq!(fields.len(), 2);
            }
        } else { panic!("expected type def"); }
    }

    #[test]
    fn test_type_def_union() {
        let p = parse_src("type Shape = Circle(float) | Rect(float float)");
        if let StmtKind::TypeDef(td) = &p.stmts[0].kind {
            if let TypeDefKind::Union(variants) = &td.kind {
                assert_eq!(variants.len(), 2);
                assert_eq!(variants[0].name, "Circle");
                assert_eq!(variants[1].name, "Rect");
            } else { panic!("expected union"); }
        }
    }

    #[test]
    fn test_list_literal() {
        let p = parse_src("x = [1, 2, 3]");
        if let StmtKind::Assign { value, .. } = &p.stmts[0].kind {
            assert!(matches!(value.kind, ExprKind::List(_)));
        }
    }

    #[test]
    fn test_method_chain() {
        let p = parse_src("items.filter(x => x > 0).len()");
        assert!(matches!(p.stmts[0].kind, StmtKind::Expr(_)));
    }

    #[test]
    fn test_borrow_param() {
        let p = parse_src("fn show(&x:int)\n    println(x)");
        if let StmtKind::FnDef(f) = &p.stmts[0].kind {
            assert!(f.params[0].borrow);
        }
    }

    #[test]
    fn test_return_stmt() {
        let p = parse_src("fn f(x)\n    return x + 1");
        if let StmtKind::FnDef(f) = &p.stmts[0].kind {
            assert!(matches!(f.body.stmts[0].kind, StmtKind::Return(_)));
        }
    }

    #[test]
    fn test_multiple_statements() {
        let p = parse_src("x = 1\ny = 2\nz = x + y");
        assert_eq!(p.stmts.len(), 3);
    }

    #[test]
    fn test_option_type() {
        let p = parse_src("let x:?int = 42");
        if let StmtKind::Let { ty, .. } = &p.stmts[0].kind {
            assert!(matches!(ty, AshType::Option(_)));
        }
    }

    #[test]
    fn test_await_expr() {
        let p = parse_src("result = await fetch(url)");
        if let StmtKind::Assign { value, .. } = &p.stmts[0].kind {
            assert!(matches!(value.kind, ExprKind::Await(_)));
        }
    }

    #[test]
    fn test_nested_fn_calls() {
        let p = parse_src("println(fmt(\"{x}\"))");
        assert!(matches!(p.stmts[0].kind, StmtKind::Expr(Expr { kind: ExprKind::Call { .. }, .. })));
    }

    #[test]
    fn test_arithmetic_precedence() {
        // 2 + 3 * 4 should parse as 2 + (3 * 4)
        let p = parse_src("2 + 3 * 4");
        if let StmtKind::Expr(Expr { kind: ExprKind::BinOp { op, rhs, .. }, .. }) = &p.stmts[0].kind {
            assert_eq!(*op, BinOp::Add);
            assert!(matches!(rhs.kind, ExprKind::BinOp { op: BinOp::Mul, .. }));
        }
    }

    #[test]
    fn test_index_expr() {
        let p = parse_src("items[0]");
        assert!(matches!(p.stmts[0].kind, StmtKind::Expr(Expr { kind: ExprKind::Index { .. }, .. })));
    }
}

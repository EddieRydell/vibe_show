use super::ast::*;
use super::error::CompileError;
use super::lexer::{SpannedToken, Token};

pub fn parse(tokens: Vec<SpannedToken>) -> Result<Script, Vec<CompileError>> {
    let mut parser = Parser::new(tokens);
    parser.parse_script()
}

struct Parser {
    tokens: Vec<SpannedToken>,
    pos: usize,
    errors: Vec<CompileError>,
}

impl Parser {
    fn new(tokens: Vec<SpannedToken>) -> Self {
        Self {
            tokens,
            pos: 0,
            errors: Vec::new(),
        }
    }

    fn parse_script(&mut self) -> Result<Script, Vec<CompileError>> {
        let mut metadata = Vec::new();
        let mut type_defs = Vec::new();
        let mut params = Vec::new();
        let mut functions = Vec::new();
        let mut body = Vec::new();

        self.skip_newlines();

        while !self.at_eof() {
            match self.peek() {
                Token::At => {
                    match self.parse_metadata() {
                        Ok(m) => metadata.push(m),
                        Err(e) => self.errors.push(e),
                    }
                }
                Token::Enum | Token::Flags => {
                    match self.parse_type_def() {
                        Ok(td) => type_defs.push(td),
                        Err(e) => self.errors.push(e),
                    }
                }
                Token::Param => {
                    match self.parse_param_def() {
                        Ok(p) => params.push(p),
                        Err(e) => self.errors.push(e),
                    }
                }
                Token::Fn => {
                    match self.parse_fn_def() {
                        Ok(f) => functions.push(f),
                        Err(e) => self.errors.push(e),
                    }
                }
                _ => {
                    match self.parse_stmt() {
                        Ok(s) => body.push(s),
                        Err(e) => {
                            self.errors.push(e);
                            self.recover_to_newline();
                        }
                    }
                }
            }
            self.skip_newlines();
        }

        if self.errors.is_empty() {
            Ok(Script {
                metadata,
                type_defs,
                params,
                functions,
                body,
            })
        } else {
            Err(std::mem::take(&mut self.errors))
        }
    }

    // ── Helpers ────────────────────────────────────────────────────

    fn peek(&self) -> &Token {
        self.tokens.get(self.pos).map_or(&Token::Eof, |t| &t.token)
    }

    fn span(&self) -> Span {
        self.tokens.get(self.pos).map_or(
            Span::new(0, 0),
            |t| t.span,
        )
    }

    fn at_eof(&self) -> bool {
        matches!(self.peek(), Token::Eof)
    }

    fn advance(&mut self) -> &SpannedToken {
        let tok = &self.tokens[self.pos];
        if self.pos < self.tokens.len() - 1 {
            self.pos += 1;
        }
        tok
    }

    fn expect(&mut self, expected: &Token) -> Result<Span, CompileError> {
        if self.peek() == expected {
            let sp = self.span();
            self.advance();
            Ok(sp)
        } else {
            Err(CompileError::parser(
                format!("Expected {expected:?}, got {:?}", self.peek()),
                self.span(),
            ))
        }
    }

    fn expect_ident(&mut self) -> Result<(String, Span), CompileError> {
        if let Token::Ident(name) = self.peek().clone() {
            let sp = self.span();
            self.advance();
            Ok((name, sp))
        } else {
            Err(CompileError::parser(
                format!("Expected identifier, got {:?}", self.peek()),
                self.span(),
            ))
        }
    }

    fn skip_newlines(&mut self) {
        while matches!(self.peek(), Token::Newline) {
            self.advance();
        }
    }

    fn expect_terminator(&mut self) -> Result<(), CompileError> {
        if matches!(self.peek(), Token::Newline | Token::Eof | Token::RBrace) {
            if matches!(self.peek(), Token::Newline) {
                self.advance();
            }
            Ok(())
        } else {
            Err(CompileError::parser(
                format!("Expected newline or end of block, got {:?}", self.peek()),
                self.span(),
            ))
        }
    }

    fn recover_to_newline(&mut self) {
        while !matches!(self.peek(), Token::Newline | Token::Eof) {
            self.advance();
        }
        self.skip_newlines();
    }

    // ── Metadata ──────────────────────────────────────────────────

    fn parse_metadata(&mut self) -> Result<Metadata, CompileError> {
        let start = self.span();
        self.expect(&Token::At)?;
        let (key, _) = self.expect_ident()?;
        let value = match self.peek() {
            Token::String(s) => {
                let s = s.clone();
                self.advance();
                MetaValue::Str(s)
            }
            Token::True => {
                self.advance();
                MetaValue::Bool(true)
            }
            Token::False => {
                self.advance();
                MetaValue::Bool(false)
            }
            _ => {
                return Err(CompileError::parser(
                    "Expected string or boolean after @key",
                    self.span(),
                ));
            }
        };
        let end_span = self.span();
        self.expect_terminator()?;
        Ok(Metadata {
            key,
            value,
            span: start.merge(end_span),
        })
    }

    // ── Type definitions ──────────────────────────────────────────

    fn parse_type_def(&mut self) -> Result<TypeDef, CompileError> {
        let start = self.span();
        let kind = match self.peek() {
            Token::Enum => { self.advance(); TypeDefKind::Enum }
            Token::Flags => { self.advance(); TypeDefKind::Flags }
            _ => return Err(CompileError::parser("Expected 'enum' or 'flags'", self.span())),
        };
        let (name, _) = self.expect_ident()?;
        self.expect(&Token::LBrace)?;
        self.skip_newlines();

        let mut variants = Vec::new();
        while !matches!(self.peek(), Token::RBrace | Token::Eof) {
            let (variant, _) = self.expect_ident()?;
            variants.push(variant);
            if matches!(self.peek(), Token::Comma) {
                self.advance();
            }
            self.skip_newlines();
        }
        let end_span = self.span();
        self.expect(&Token::RBrace)?;
        self.expect_terminator()?;

        Ok(TypeDef {
            kind,
            name,
            variants,
            span: start.merge(end_span),
        })
    }

    // ── Param definitions ─────────────────────────────────────────

    fn parse_param_def(&mut self) -> Result<ParamDef, CompileError> {
        let start = self.span();
        self.expect(&Token::Param)?;
        let (name, _) = self.expect_ident()?;
        self.expect(&Token::Colon)?;
        let ty = self.parse_param_type()?;
        self.expect(&Token::Eq)?;
        let default = self.parse_param_default(&ty)?;
        let end_span = self.span();
        self.expect_terminator()?;

        Ok(ParamDef {
            name,
            ty,
            default,
            span: start.merge(end_span),
        })
    }

    fn parse_param_type(&mut self) -> Result<ParamType, CompileError> {
        match self.peek().clone() {
            Token::FloatTy => {
                self.advance();
                if matches!(self.peek(), Token::LParen) {
                    self.advance();
                    let min = self.parse_number_lit()?;
                    self.expect(&Token::Comma)?;
                    let max = self.parse_number_lit()?;
                    self.expect(&Token::RParen)?;
                    Ok(ParamType::Float(Some((min, max))))
                } else {
                    Ok(ParamType::Float(None))
                }
            }
            Token::IntTy => {
                self.advance();
                if matches!(self.peek(), Token::LParen) {
                    self.advance();
                    let min = self.parse_number_lit()? as i32;
                    self.expect(&Token::Comma)?;
                    let max = self.parse_number_lit()? as i32;
                    self.expect(&Token::RParen)?;
                    Ok(ParamType::Int(Some((min, max))))
                } else {
                    Ok(ParamType::Int(None))
                }
            }
            Token::BoolTy => { self.advance(); Ok(ParamType::Bool) }
            Token::ColorTy => { self.advance(); Ok(ParamType::Color) }
            Token::GradientTy => { self.advance(); Ok(ParamType::Gradient) }
            Token::CurveTy => { self.advance(); Ok(ParamType::Curve) }
            Token::Ident(name) => { self.advance(); Ok(ParamType::Named(name)) }
            _ => Err(CompileError::parser(
                format!("Expected type name, got {:?}", self.peek()),
                self.span(),
            )),
        }
    }

    fn parse_number_lit(&mut self) -> Result<f64, CompileError> {
        let neg = if matches!(self.peek(), Token::Minus) {
            self.advance();
            true
        } else {
            false
        };
        match self.peek().clone() {
            Token::Float(v) => { self.advance(); Ok(if neg { -v } else { v }) }
            Token::Int(v) => { self.advance(); Ok(if neg { -f64::from(v) } else { f64::from(v) }) }
            _ => Err(CompileError::parser(
                format!("Expected number, got {:?}", self.peek()),
                self.span(),
            )),
        }
    }

    fn parse_param_default(&mut self, ty: &ParamType) -> Result<Expr, CompileError> {
        let span = self.span();
        match ty {
            ParamType::Gradient => self.parse_gradient_lit(),
            ParamType::Curve => self.parse_curve_lit(),
            ParamType::Named(_) => {
                // Could be EnumVariant or FlagCombine
                // Check if it's Flag1 | Flag2
                let (first, _) = self.expect_ident()?;
                if matches!(self.peek(), Token::Pipe) {
                    let mut flags = vec![first];
                    while matches!(self.peek(), Token::Pipe) {
                        self.advance();
                        let (f, _) = self.expect_ident()?;
                        flags.push(f);
                    }
                    Ok(Expr {
                        kind: ExprKind::FlagCombine(flags),
                        span,
                    })
                } else {
                    Ok(Expr {
                        kind: ExprKind::Ident(first),
                        span,
                    })
                }
            }
            _ => self.parse_expr(),
        }
    }

    fn parse_gradient_lit(&mut self) -> Result<Expr, CompileError> {
        let span = self.span();
        let mut stops = Vec::new();
        loop {
            let (r, g, b) = self.parse_color_hex()?;
            let position = if matches!(self.peek(), Token::At) {
                self.advance();
                Some(self.parse_number_lit()?)
            } else {
                None
            };
            stops.push(GradientStop {
                color: (r, g, b),
                position,
            });
            if !matches!(self.peek(), Token::Comma) {
                break;
            }
            self.advance();
        }
        Ok(Expr {
            kind: ExprKind::GradientLit(stops),
            span,
        })
    }

    fn parse_curve_lit(&mut self) -> Result<Expr, CompileError> {
        let span = self.span();
        let mut points = Vec::new();
        loop {
            let x = self.parse_number_lit()?;
            self.expect(&Token::Colon)?;
            let y = self.parse_number_lit()?;
            points.push((x, y));
            if !matches!(self.peek(), Token::Comma) {
                break;
            }
            self.advance();
        }
        Ok(Expr {
            kind: ExprKind::CurveLit(points),
            span,
        })
    }

    fn parse_color_hex(&mut self) -> Result<(u8, u8, u8), CompileError> {
        if let Token::ColorHex(r, g, b) = self.peek().clone() {
            self.advance();
            Ok((r, g, b))
        } else {
            Err(CompileError::parser(
                format!("Expected color hex literal (#rrggbb), got {:?}", self.peek()),
                self.span(),
            ))
        }
    }

    // ── Function definitions ──────────────────────────────────────

    fn parse_fn_def(&mut self) -> Result<FnDef, CompileError> {
        let start = self.span();
        self.expect(&Token::Fn)?;
        let (name, _) = self.expect_ident()?;
        self.expect(&Token::LParen)?;

        let mut params = Vec::new();
        while !matches!(self.peek(), Token::RParen | Token::Eof) {
            let (pname, _) = self.expect_ident()?;
            self.expect(&Token::Colon)?;
            let ty = self.parse_type_name()?;
            params.push(FnParam { name: pname, ty });
            if matches!(self.peek(), Token::Comma) {
                self.advance();
            }
        }
        self.expect(&Token::RParen)?;
        self.expect(&Token::Arrow)?;
        let return_type = self.parse_type_name()?;

        self.expect(&Token::LBrace)?;
        self.skip_newlines();
        let body = self.parse_block()?;
        let end_span = self.span();
        self.expect(&Token::RBrace)?;
        self.expect_terminator()?;

        Ok(FnDef {
            name,
            params,
            return_type,
            body,
            span: start.merge(end_span),
        })
    }

    fn parse_type_name(&mut self) -> Result<TypeName, CompileError> {
        match self.peek() {
            Token::FloatTy => { self.advance(); Ok(TypeName::Float) }
            Token::IntTy => { self.advance(); Ok(TypeName::Int) }
            Token::BoolTy => { self.advance(); Ok(TypeName::Bool) }
            Token::ColorTy => { self.advance(); Ok(TypeName::Color) }
            Token::Vec2Ty => { self.advance(); Ok(TypeName::Vec2) }
            Token::GradientTy => { self.advance(); Ok(TypeName::Gradient) }
            Token::CurveTy => { self.advance(); Ok(TypeName::Curve) }
            _ => Err(CompileError::parser(
                format!("Expected type name, got {:?}", self.peek()),
                self.span(),
            )),
        }
    }

    // ── Statements ────────────────────────────────────────────────

    fn parse_block(&mut self) -> Result<Vec<Stmt>, CompileError> {
        let mut stmts = Vec::new();
        self.skip_newlines();
        while !matches!(self.peek(), Token::RBrace | Token::Eof) {
            stmts.push(self.parse_stmt()?);
            self.skip_newlines();
        }
        Ok(stmts)
    }

    fn parse_stmt(&mut self) -> Result<Stmt, CompileError> {
        match self.peek() {
            Token::Let => {
                let start = self.span();
                self.advance();
                let (name, _) = self.expect_ident()?;
                self.expect(&Token::Eq)?;
                let value = self.parse_expr()?;
                let end_span = value.span;
                self.expect_terminator()?;
                Ok(Stmt::Let {
                    name,
                    value,
                    span: start.merge(end_span),
                })
            }
            _ => {
                let expr = self.parse_expr()?;
                // Don't require terminator if we're at RBrace (last expr in block)
                if !matches!(self.peek(), Token::RBrace | Token::Eof) {
                    self.expect_terminator()?;
                }
                Ok(Stmt::Expr(expr))
            }
        }
    }

    // ── Expression parsing (precedence climbing) ──────────────────

    fn parse_expr(&mut self) -> Result<Expr, CompileError> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> Result<Expr, CompileError> {
        let mut left = self.parse_and()?;
        while matches!(self.peek(), Token::Or) {
            self.advance();
            let right = self.parse_and()?;
            let span = left.span.merge(right.span);
            left = Expr {
                kind: ExprKind::BinOp {
                    op: BinOp::Or,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                span,
            };
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<Expr, CompileError> {
        let mut left = self.parse_equality()?;
        while matches!(self.peek(), Token::And) {
            self.advance();
            let right = self.parse_equality()?;
            let span = left.span.merge(right.span);
            left = Expr {
                kind: ExprKind::BinOp {
                    op: BinOp::And,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                span,
            };
        }
        Ok(left)
    }

    fn parse_equality(&mut self) -> Result<Expr, CompileError> {
        let mut left = self.parse_comparison()?;
        loop {
            let op = match self.peek() {
                Token::EqEq => BinOp::Eq,
                Token::Ne => BinOp::Ne,
                _ => break,
            };
            self.advance();
            let right = self.parse_comparison()?;
            let span = left.span.merge(right.span);
            left = Expr {
                kind: ExprKind::BinOp {
                    op,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                span,
            };
        }
        Ok(left)
    }

    fn parse_comparison(&mut self) -> Result<Expr, CompileError> {
        let mut left = self.parse_add()?;
        loop {
            let op = match self.peek() {
                Token::Lt => BinOp::Lt,
                Token::Gt => BinOp::Gt,
                Token::Le => BinOp::Le,
                Token::Ge => BinOp::Ge,
                _ => break,
            };
            self.advance();
            let right = self.parse_add()?;
            let span = left.span.merge(right.span);
            left = Expr {
                kind: ExprKind::BinOp {
                    op,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                span,
            };
        }
        Ok(left)
    }

    fn parse_add(&mut self) -> Result<Expr, CompileError> {
        let mut left = self.parse_mul()?;
        loop {
            let op = match self.peek() {
                Token::Plus => BinOp::Add,
                Token::Minus => BinOp::Sub,
                _ => break,
            };
            self.advance();
            let right = self.parse_mul()?;
            let span = left.span.merge(right.span);
            left = Expr {
                kind: ExprKind::BinOp {
                    op,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                span,
            };
        }
        Ok(left)
    }

    fn parse_mul(&mut self) -> Result<Expr, CompileError> {
        let mut left = self.parse_unary()?;
        loop {
            let op = match self.peek() {
                Token::Star => BinOp::Mul,
                Token::Slash => BinOp::Div,
                Token::Percent => BinOp::Mod,
                _ => break,
            };
            self.advance();
            let right = self.parse_unary()?;
            let span = left.span.merge(right.span);
            left = Expr {
                kind: ExprKind::BinOp {
                    op,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                span,
            };
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<Expr, CompileError> {
        match self.peek() {
            Token::Minus => {
                let start = self.span();
                self.advance();
                let operand = self.parse_unary()?;
                let span = start.merge(operand.span);
                Ok(Expr {
                    kind: ExprKind::UnaryOp {
                        op: UnaryOp::Neg,
                        operand: Box::new(operand),
                    },
                    span,
                })
            }
            Token::Bang => {
                let start = self.span();
                self.advance();
                let operand = self.parse_unary()?;
                let span = start.merge(operand.span);
                Ok(Expr {
                    kind: ExprKind::UnaryOp {
                        op: UnaryOp::Not,
                        operand: Box::new(operand),
                    },
                    span,
                })
            }
            _ => self.parse_postfix(),
        }
    }

    fn parse_postfix(&mut self) -> Result<Expr, CompileError> {
        let mut expr = self.parse_primary()?;

        loop {
            match self.peek() {
                Token::Dot => {
                    self.advance();
                    let (field, field_span) = self.expect_ident()?;
                    // Check if it's a method call: obj.method(args)
                    if matches!(self.peek(), Token::LParen) {
                        self.advance();
                        let args = self.parse_args()?;
                        let span = expr.span.merge(field_span);
                        expr = Expr {
                            kind: ExprKind::MethodCall {
                                object: Box::new(expr),
                                method: field,
                                args,
                            },
                            span,
                        };
                    } else {
                        let span = expr.span.merge(field_span);
                        expr = Expr {
                            kind: ExprKind::Field {
                                object: Box::new(expr),
                                field,
                            },
                            span,
                        };
                    }
                }
                Token::LParen if matches!(expr.kind, ExprKind::Ident(_)) => {
                    // This handles: `ident(args)` for both function calls and
                    // gradient/curve evaluation: `palette(t)`, `curve(t)`
                    if let ExprKind::Ident(name) = &expr.kind {
                        let name = name.clone();
                        self.advance();
                        let args = self.parse_args()?;
                        let span = expr.span.merge(self.span());
                        expr = Expr {
                            kind: ExprKind::Call { name, args },
                            span,
                        };
                    }
                }
                _ => break,
            }
        }

        Ok(expr)
    }

    fn parse_args(&mut self) -> Result<Vec<Expr>, CompileError> {
        let mut args = Vec::new();
        if !matches!(self.peek(), Token::RParen) {
            args.push(self.parse_expr()?);
            while matches!(self.peek(), Token::Comma) {
                self.advance();
                args.push(self.parse_expr()?);
            }
        }
        self.expect(&Token::RParen)?;
        Ok(args)
    }

    fn parse_primary(&mut self) -> Result<Expr, CompileError> {
        let span = self.span();
        match self.peek().clone() {
            Token::Float(v) => {
                self.advance();
                Ok(Expr { kind: ExprKind::FloatLit(v), span })
            }
            Token::Int(v) => {
                self.advance();
                Ok(Expr { kind: ExprKind::IntLit(v), span })
            }
            Token::True => {
                self.advance();
                Ok(Expr { kind: ExprKind::BoolLit(true), span })
            }
            Token::False => {
                self.advance();
                Ok(Expr { kind: ExprKind::BoolLit(false), span })
            }
            Token::ColorHex(r, g, b) => {
                self.advance();
                Ok(Expr { kind: ExprKind::ColorLit { r, g, b }, span })
            }
            Token::Ident(name) => {
                self.advance();
                // Check for Enum.Variant pattern
                if matches!(self.peek(), Token::Dot) {
                    // Peek further to see if the next token is an ident (not a function call)
                    // This is Enum.Variant only if the first ident starts with uppercase
                    if name.chars().next().is_some_and(|c| c.is_ascii_uppercase()) {
                        let dot_pos = self.pos;
                        self.advance(); // skip dot
                        if let Token::Ident(variant) = self.peek().clone() {
                            // Check: is the next thing NOT a '(' ? (then it's field access, not method)
                            self.advance();
                            if !matches!(self.peek(), Token::LParen) {
                                return Ok(Expr {
                                    kind: ExprKind::EnumAccess {
                                        enum_name: name,
                                        variant,
                                    },
                                    span: span.merge(self.span()),
                                });
                            }
                            // It was `EnumName.method(` — backtrack
                            self.pos = dot_pos;
                        } else {
                            self.pos = dot_pos;
                        }
                    }
                }
                Ok(Expr { kind: ExprKind::Ident(name), span })
            }
            Token::LParen => {
                self.advance();
                let expr = self.parse_expr()?;
                self.expect(&Token::RParen)?;
                Ok(expr)
            }
            Token::If => {
                self.parse_if_expr()
            }
            _ => {
                Err(CompileError::parser(
                    format!("Unexpected token: {:?}", self.peek()),
                    self.span(),
                ))
            }
        }
    }

    fn parse_if_expr(&mut self) -> Result<Expr, CompileError> {
        let start = self.span();
        self.expect(&Token::If)?;
        let condition = self.parse_expr()?;
        self.expect(&Token::LBrace)?;
        self.skip_newlines();
        let then_body = self.parse_block()?;
        let mut end_span = self.span();
        self.expect(&Token::RBrace)?;

        let else_body = if matches!(self.peek(), Token::Else) {
            self.advance();
            if matches!(self.peek(), Token::If) {
                // else if — parse as single-element block
                let nested = self.parse_if_expr()?;
                end_span = nested.span;
                Some(vec![Stmt::Expr(nested)])
            } else {
                self.expect(&Token::LBrace)?;
                self.skip_newlines();
                let body = self.parse_block()?;
                end_span = self.span();
                self.expect(&Token::RBrace)?;
                Some(body)
            }
        } else {
            None
        };

        Ok(Expr {
            kind: ExprKind::If {
                condition: Box::new(condition),
                then_body,
                else_body,
            },
            span: start.merge(end_span),
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::dsl::lexer::lex;

    fn parse_str(s: &str) -> Script {
        let tokens = lex(s).unwrap();
        parse(tokens).unwrap()
    }

    #[test]
    fn parse_metadata() {
        let script = parse_str("@name \"Fire\"\n@spatial false");
        assert_eq!(script.metadata.len(), 2);
        assert_eq!(script.metadata[0].key, "name");
        assert!(matches!(script.metadata[0].value, MetaValue::Str(ref s) if s == "Fire"));
        assert_eq!(script.metadata[1].key, "spatial");
        assert!(matches!(script.metadata[1].value, MetaValue::Bool(false)));
    }

    #[test]
    fn parse_enum_def() {
        let script = parse_str("enum ColorMode { Static, Gradient, Rainbow }");
        assert_eq!(script.type_defs.len(), 1);
        assert_eq!(script.type_defs[0].name, "ColorMode");
        assert_eq!(script.type_defs[0].kind, TypeDefKind::Enum);
        assert_eq!(script.type_defs[0].variants, vec!["Static", "Gradient", "Rainbow"]);
    }

    #[test]
    fn parse_flags_def() {
        let script = parse_str("flags Options { Mirror, Wrap }");
        assert_eq!(script.type_defs.len(), 1);
        assert_eq!(script.type_defs[0].kind, TypeDefKind::Flags);
        assert_eq!(script.type_defs[0].variants, vec!["Mirror", "Wrap"]);
    }

    #[test]
    fn parse_float_param() {
        let script = parse_str("param speed: float(0.1, 10.0) = 2.0");
        assert_eq!(script.params.len(), 1);
        assert_eq!(script.params[0].name, "speed");
        assert!(matches!(script.params[0].ty, ParamType::Float(Some((min, max))) if (min - 0.1).abs() < 0.001 && (max - 10.0).abs() < 0.001));
    }

    #[test]
    fn parse_color_param() {
        let script = parse_str("param col: color = #ff0000");
        assert_eq!(script.params.len(), 1);
        assert!(matches!(script.params[0].default.kind, ExprKind::ColorLit { r: 255, g: 0, b: 0 }));
    }

    #[test]
    fn parse_enum_param_default() {
        let script = parse_str("enum Mode { A, B }\nparam mode: Mode = A");
        assert_eq!(script.params.len(), 1);
        assert!(matches!(script.params[0].default.kind, ExprKind::Ident(ref s) if s == "A"));
    }

    #[test]
    fn parse_flags_param_default() {
        let script = parse_str("flags Opts { Mirror, Wrap }\nparam opts: Opts = Mirror | Wrap");
        assert_eq!(script.params.len(), 1);
        assert!(matches!(script.params[0].default.kind, ExprKind::FlagCombine(ref flags) if flags == &["Mirror", "Wrap"]));
    }

    #[test]
    fn parse_fn_def() {
        let script = parse_str("fn pulse(x: float) -> float {\nx * x\n}");
        assert_eq!(script.functions.len(), 1);
        assert_eq!(script.functions[0].name, "pulse");
        assert_eq!(script.functions[0].params.len(), 1);
        assert_eq!(script.functions[0].return_type, TypeName::Float);
    }

    #[test]
    fn parse_let_and_expr() {
        let script = parse_str("let x = 1.0 + 2.0\nx * 3.0");
        assert_eq!(script.body.len(), 2);
        assert!(matches!(script.body[0], Stmt::Let { ref name, .. } if name == "x"));
    }

    #[test]
    fn parse_if_else() {
        let script = parse_str("if x > 0.0 {\nrgb(1.0, 0.0, 0.0)\n} else {\nrgb(0.0, 0.0, 1.0)\n}");
        assert_eq!(script.body.len(), 1);
        if let Stmt::Expr(ref e) = script.body[0] {
            assert!(matches!(e.kind, ExprKind::If { .. }));
        } else {
            panic!("expected expression statement");
        }
    }

    #[test]
    fn parse_function_call() {
        let script = parse_str("sin(t * 3.14)");
        assert_eq!(script.body.len(), 1);
        if let Stmt::Expr(ref e) = script.body[0] {
            assert!(matches!(e.kind, ExprKind::Call { ref name, .. } if name == "sin"));
        } else {
            panic!("expected call");
        }
    }

    #[test]
    fn parse_field_access() {
        let script = parse_str("pos2d.x");
        if let Stmt::Expr(ref e) = script.body[0] {
            assert!(matches!(e.kind, ExprKind::Field { ref field, .. } if field == "x"));
        } else {
            panic!("expected field access");
        }
    }

    #[test]
    fn parse_method_call() {
        let script = parse_str("c.scale(0.5)");
        if let Stmt::Expr(ref e) = script.body[0] {
            assert!(matches!(e.kind, ExprKind::MethodCall { ref method, .. } if method == "scale"));
        } else {
            panic!("expected method call");
        }
    }

    #[test]
    fn parse_enum_access() {
        let script = parse_str("Mode.Static");
        if let Stmt::Expr(ref e) = script.body[0] {
            assert!(matches!(e.kind, ExprKind::EnumAccess { ref enum_name, ref variant } if enum_name == "Mode" && variant == "Static"));
        } else {
            panic!("expected enum access");
        }
    }

    #[test]
    fn parse_complex_script() {
        let source = r#"
@name "Test Effect"
@spatial false

enum Mode { A, B }

param speed: float(0.1, 10.0) = 1.0
param mode: Mode = A

fn pulse(x: float) -> float {
    x * x
}

let phase = t * speed
let intensity = pulse(phase)

if mode == Mode.A {
    rgb(intensity, 0.0, 0.0)
} else {
    rgb(0.0, 0.0, intensity)
}
"#;
        let script = parse_str(source);
        assert_eq!(script.metadata.len(), 2);
        assert_eq!(script.type_defs.len(), 1);
        assert_eq!(script.params.len(), 2);
        assert_eq!(script.functions.len(), 1);
        assert!(script.body.len() >= 2);
    }

    #[test]
    fn parse_gradient_param() {
        let script = parse_str("param g: gradient = #000000, #ff4400, #ffffff");
        assert_eq!(script.params.len(), 1);
        if let ExprKind::GradientLit(ref stops) = script.params[0].default.kind {
            assert_eq!(stops.len(), 3);
            assert_eq!(stops[0].color, (0, 0, 0));
            assert_eq!(stops[1].color, (255, 68, 0));
        } else {
            panic!("expected gradient literal");
        }
    }
}

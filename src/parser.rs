use crate::ast::*;
use crate::lexer::{Token, TokenKind as T};
use crate::{Diagnostic, Span};

pub struct Parser {
    tokens: Vec<Token>,
    position: usize,
}

impl Parser {
    pub(crate) fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            position: 0,
        }
    }

    pub fn parse(mut self) -> Result<Module, Diagnostic> {
        let statements = self.statements(false)?;
        self.expect(&T::Eof, "end of file")?;
        Ok(Module { statements })
    }

    fn current(&self) -> &Token {
        &self.tokens[self.position]
    }
    fn at(&self, kind: &T) -> bool {
        std::mem::discriminant(&self.current().kind) == std::mem::discriminant(kind)
    }
    fn bump(&mut self) -> Token {
        let token = self.current().clone();
        if !self.at(&T::Eof) {
            self.position += 1;
        }
        token
    }
    fn eat(&mut self, kind: &T) -> bool {
        if self.at(kind) {
            self.bump();
            true
        } else {
            false
        }
    }
    fn error(&self, message: impl Into<String>) -> Diagnostic {
        Diagnostic::new(message, self.current().span)
    }
    fn expect(&mut self, kind: &T, description: &str) -> Result<Token, Diagnostic> {
        if self.at(kind) {
            Ok(self.bump())
        } else {
            Err(self.error(format!(
                "expected {description}, found {:?}",
                self.current().kind
            )))
        }
    }
    fn name(&mut self) -> Result<(String, Span), Diagnostic> {
        match self.current().kind.clone() {
            T::Name(name) => Ok((name, self.bump().span)),
            _ => Err(self.error("expected a name")),
        }
    }
    fn statements(&mut self, nested: bool) -> Result<Vec<Stmt>, Diagnostic> {
        let mut statements = Vec::new();
        while !(self.at(&T::Eof) || nested && self.at(&T::Dedent)) {
            if self.eat(&T::Newline) {
                continue;
            }
            statements.push(self.statement()?);
        }
        Ok(statements)
    }
    fn function_ahead(&self) -> bool {
        if !matches!(self.current().kind, T::Name(_))
            || !matches!(
                self.tokens.get(self.position + 1).map(|t| &t.kind),
                Some(T::LParen)
            )
        {
            return false;
        }
        let mut depth = 0;
        for (index, token) in self.tokens.iter().enumerate().skip(self.position + 1) {
            match token.kind {
                T::LParen => depth += 1,
                T::RParen => {
                    depth -= 1;
                    if depth == 0 {
                        return matches!(
                            self.tokens.get(index + 1).map(|t| &t.kind),
                            Some(T::Equal)
                        );
                    }
                }
                T::Eof | T::Newline => return false,
                _ => {}
            }
        }
        false
    }
    fn statement(&mut self) -> Result<Stmt, Diagnostic> {
        let start = self.current().span.start;
        if self.function_ahead() {
            return self.function();
        }
        let kind = if self.eat(&T::Return) {
            StmtKind::Return(if self.at(&T::Newline) {
                None
            } else {
                Some(self.expression(0)?)
            })
        } else if matches!(self.current().kind, T::Name(_))
            && matches!(
                self.tokens.get(self.position + 1).map(|t| &t.kind),
                Some(T::Colon | T::Equal)
            )
        {
            let (name, _) = self.name()?;
            let annotation = if self.eat(&T::Colon) {
                Some(self.type_ref()?)
            } else {
                None
            };
            self.expect(&T::Equal, "'=' in assignment")?;
            StmtKind::Assign {
                name,
                annotation,
                value: self.expression(0)?,
            }
        } else {
            StmtKind::Expression(self.expression(0)?)
        };
        let end = self.expect(&T::Newline, "end of statement")?.span.end;
        Ok(Stmt {
            kind,
            span: Span { start, end },
        })
    }
    fn function(&mut self) -> Result<Stmt, Diagnostic> {
        let (name, span) = self.name()?;
        // Capitalized declarations will eventually describe classes, with different semantics.
        if name.starts_with(char::is_uppercase) {
            return Err(Diagnostic::new(
                "class declarations are not implemented yet",
                span,
            ));
        }
        self.expect(&T::LParen, "'('")?;
        let mut parameters = Vec::new();
        if !self.at(&T::RParen) {
            loop {
                let (name, start) = self.name()?;
                self.expect(&T::Colon, "':' and a parameter type")?;
                let ty = self.type_ref()?;
                let span = Span {
                    start: start.start,
                    end: ty.span.end,
                };
                parameters.push(Parameter { name, ty, span });
                if !self.eat(&T::Comma) || self.at(&T::RParen) {
                    break;
                }
            }
        }
        self.expect(&T::RParen, "')'")?;
        self.expect(&T::Equal, "'='")?;
        let body = if self.eat(&T::Newline) {
            self.expect(&T::Indent, "an indented function body")?;
            let body = self.statements(true)?;
            self.expect(&T::Dedent, "end of function body")?;
            body
        } else {
            vec![self.statement()?]
        };
        let end = body.last().map(|s| s.span.end).unwrap_or(span.end);
        Ok(Stmt {
            kind: StmtKind::Function {
                name,
                parameters,
                body,
            },
            span: Span {
                start: span.start,
                end,
            },
        })
    }
    fn type_ref(&mut self) -> Result<TypeRef, Diagnostic> {
        let (name, mut span) = self.name()?;
        let mut array_dimensions = 0;
        while self.eat(&T::LBracket) {
            span.end = self.expect(&T::RBracket, "']' in array type")?.span.end;
            array_dimensions += 1;
        }
        let nullable = self.eat(&T::Question);
        if nullable {
            span.end = self.tokens[self.position - 1].span.end;
        }
        Ok(TypeRef {
            name,
            nullable,
            array_dimensions,
            span,
        })
    }
    fn expression(&mut self, minimum: u8) -> Result<Expr, Diagnostic> {
        let token = self.bump();
        let mut left = match token.kind {
            T::Integer(value) => Expr {
                kind: ExprKind::Integer(value),
                span: token.span,
            },
            T::Float(value) => Expr {
                kind: ExprKind::Float(value),
                span: token.span,
            },
            T::String(value) => Expr {
                kind: ExprKind::String(value),
                span: token.span,
            },
            T::True | T::False => Expr {
                kind: ExprKind::Bool(token.kind == T::True),
                span: token.span,
            },
            T::Name(name) => {
                if matches!(
                    name.as_str(),
                    "if" | "else"
                        | "elseif"
                        | "otherwise"
                        | "for"
                        | "while"
                        | "uses"
                        | "using"
                        | "include"
                        | "co"
                        | "try"
                        | "catch"
                        | "finally"
                        | "with"
                        | "break"
                        | "continue"
                        | "pass"
                ) {
                    return Err(Diagnostic::new(
                        format!("'{name}' syntax is not implemented yet"),
                        token.span,
                    ));
                }
                Expr {
                    kind: ExprKind::Name(name),
                    span: token.span,
                }
            }
            T::Minus | T::Plus | T::Not => {
                let op = match token.kind {
                    T::Minus => UnaryOp::Negate,
                    T::Plus => UnaryOp::Positive,
                    _ => UnaryOp::Not,
                };
                let operand = self.expression(if op == UnaryOp::Not { 5 } else { 13 })?;
                let span = Span {
                    start: token.span.start,
                    end: operand.span.end,
                };
                Expr {
                    kind: ExprKind::Unary {
                        op,
                        operand: Box::new(operand),
                    },
                    span,
                }
            }
            T::LParen => {
                let mut expr = self.expression(0)?;
                let end = self.expect(&T::RParen, "')'")?.span.end;
                expr.span = Span {
                    start: token.span.start,
                    end,
                };
                expr
            }
            T::LBracket => {
                let values = self.expression_list(&T::RBracket)?;
                let end = self.expect(&T::RBracket, "']'")?.span.end;
                Expr {
                    kind: ExprKind::Array(values),
                    span: Span {
                        start: token.span.start,
                        end,
                    },
                }
            }
            _ => return Err(Diagnostic::new("expected an expression", token.span)),
        };
        loop {
            let start = left.span.start;
            if self.at(&T::LParen) {
                self.bump();
                let arguments = self.expression_list(&T::RParen)?;
                let end = self.expect(&T::RParen, "')'")?.span.end;
                left = Expr {
                    kind: ExprKind::Call {
                        callee: Box::new(left),
                        arguments,
                    },
                    span: Span { start, end },
                };
                continue;
            }
            if self.eat(&T::Dot) {
                let (name, span) = self.name()?;
                left = Expr {
                    kind: ExprKind::Member {
                        object: Box::new(left),
                        name,
                    },
                    span: Span {
                        start,
                        end: span.end,
                    },
                };
                continue;
            }
            if self.eat(&T::LBracket) {
                let index = self.expression(0)?;
                let end = self.expect(&T::RBracket, "']'")?.span.end;
                left = Expr {
                    kind: ExprKind::Index {
                        object: Box::new(left),
                        index: Box::new(index),
                    },
                    span: Span { start, end },
                };
                continue;
            }
            let Some((op, left_power, right_power)) = binary(&self.current().kind) else {
                break;
            };
            if left_power < minimum {
                break;
            }
            self.bump();
            let right = self.expression(right_power)?;
            let end = right.span.end;
            left = Expr {
                kind: ExprKind::Binary {
                    op,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                span: Span { start, end },
            };
        }
        Ok(left)
    }
    fn expression_list(&mut self, closing: &T) -> Result<Vec<Expr>, Diagnostic> {
        let mut values = Vec::new();
        if !self.at(closing) {
            loop {
                values.push(self.expression(0)?);
                if !self.eat(&T::Comma) || self.at(closing) {
                    break;
                }
            }
        }
        Ok(values)
    }
}

fn binary(kind: &T) -> Option<(BinaryOp, u8, u8)> {
    let (op, power) = match kind {
        T::Or => (BinaryOp::Or, 1),
        T::And => (BinaryOp::And, 3),
        T::EqualEqual => (BinaryOp::Equal, 5),
        T::NotEqual => (BinaryOp::NotEqual, 5),
        T::Less => (BinaryOp::Less, 5),
        T::LessEqual => (BinaryOp::LessEqual, 5),
        T::Greater => (BinaryOp::Greater, 5),
        T::GreaterEqual => (BinaryOp::GreaterEqual, 5),
        T::Plus => (BinaryOp::Add, 7),
        T::Minus => (BinaryOp::Subtract, 7),
        T::Star => (BinaryOp::Multiply, 9),
        T::Slash => (BinaryOp::Divide, 9),
        T::Percent => (BinaryOp::Remainder, 9),
        T::Caret => return Some((BinaryOp::Power, 15, 14)),
        _ => return None,
    };
    Some((op, power, power + 1))
}

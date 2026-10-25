//! Semantic checks for the first executable subset of Pepper.
//!
//! This deliberately rejects everything except a parameterless main, print calls
//! with string literals, and a bare return. The parser accepts a larger language.
use crate::{
    Diagnostic, Span,
    ast::{ExprKind, Module, StmtKind},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Program {
    pub instructions: Vec<Instruction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Instruction {
    Print(String),
    Return,
}

/// Validate the executable subset before generating any LLVM instructions.
pub fn analyze(module: &Module) -> Result<Program, Diagnostic> {
    let mut main_body = None;
    for statement in &module.statements {
        match &statement.kind {
            StmtKind::Function {
                name,
                parameters,
                body,
            } if name == "main" => {
                if main_body.is_some() {
                    return Err(Diagnostic::new("duplicate main function", statement.span));
                }
                if !parameters.is_empty() {
                    return Err(Diagnostic::new(
                        "main must not take parameters",
                        statement.span,
                    ));
                }
                main_body = Some(body);
            }
            _ => {
                return Err(Diagnostic::new(
                    "the LLVM backend currently supports only a top-level main() function",
                    statement.span,
                ));
            }
        }
    }
    let body = main_body
        .ok_or_else(|| Diagnostic::new("missing main() function", Span { start: 0, end: 0 }))?;
    let mut instructions = Vec::new();
    let mut returned = false;
    for statement in body {
        if returned {
            return Err(Diagnostic::new(
                "unreachable statement after return",
                statement.span,
            ));
        }
        match &statement.kind {
            StmtKind::Return(None) => {
                returned = true;
                instructions.push(Instruction::Return);
            }
            StmtKind::Expression(expr) => {
                let ExprKind::Call { callee, arguments } = &expr.kind else {
                    return Err(Diagnostic::new(
                        "expected print(\"text\") or a bare return",
                        expr.span,
                    ));
                };
                let ExprKind::Name(name) = &callee.kind else {
                    return Err(Diagnostic::new(
                        "only calls to the built-in print are supported",
                        callee.span,
                    ));
                };
                if name != "print" {
                    return Err(Diagnostic::new(
                        format!("unknown function '{name}'; only the built-in print is supported"),
                        callee.span,
                    ));
                }
                if arguments.len() != 1 {
                    return Err(Diagnostic::new(
                        "print expects exactly one String argument",
                        expr.span,
                    ));
                }
                let ExprKind::String(value) = &arguments[0].kind else {
                    return Err(Diagnostic::new(
                        "print currently requires a String literal",
                        arguments[0].span,
                    ));
                };
                instructions.push(Instruction::Print(value.clone()));
            }
            _ => {
                return Err(Diagnostic::new(
                    "this statement is not supported by the LLVM backend yet",
                    statement.span,
                ));
            }
        }
    }
    Ok(Program { instructions })
}

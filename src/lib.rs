//! Pepper's parser and minimal native LLVM compiler.
pub mod ast;
pub mod backend;
pub mod diagnostic;
pub mod lexer;
pub mod parser;
pub mod semantic;

pub use diagnostic::{Diagnostic, Span};

/// Parse a source file into a syntax tree, reporting the first lexical or syntax error.
pub fn parse(source: &str) -> Result<ast::Module, Diagnostic> {
    parser::Parser::new(lexer::lex(source)?).parse()
}

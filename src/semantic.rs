//! Name resolution, local type inference and a typed IR for the executable subset.
use crate::{
    Diagnostic, Span,
    ast::{self, BinaryOp, UnaryOp},
};
use std::{collections::HashMap, fmt};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    Unit,
    Int,
    Double,
    Bool,
    String,
    Array(Box<Type>, usize),
}
impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Array(element, length) => write!(f, "{element}[{length}]"),
            other => write!(
                f,
                "{}",
                match other {
                    Self::Unit => "Unit",
                    Self::Int => "Int",
                    Self::Double => "Double",
                    Self::Bool => "Bool",
                    Self::String => "String",
                    _ => unreachable!(),
                }
            ),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Program {
    pub functions: Vec<Function>,
    pub main: usize,
}
#[derive(Debug, Clone)]
pub struct Function {
    pub name: String,
    pub parameters: Vec<(String, Type)>,
    pub return_type: Type,
    pub body: Vec<Statement>,
}
#[derive(Debug, Clone)]
pub enum Statement {
    Assign(String, Expr),
    Evaluate(Expr),
    Return(Option<Expr>),
}
#[derive(Debug, Clone)]
pub struct Expr {
    pub kind: ExprKind,
    pub ty: Type,
    pub span: Span,
}
#[derive(Debug, Clone)]
pub enum ExprKind {
    Integer(i64),
    Float(f64),
    Bool(bool),
    String(String),
    Local(String),
    Array(Vec<Expr>),
    Promote(Box<Expr>),
    Unary(UnaryOp, Box<Expr>),
    Binary(BinaryOp, Box<Expr>, Box<Expr>),
    Call(usize, Vec<Expr>),
    Print(Box<Expr>),
    Index(Box<Expr>, Box<Expr>),
}

#[derive(Clone, Copy)]
struct Declaration<'a> {
    name: &'a str,
    parameters: &'a [ast::Parameter],
    body: &'a [ast::Stmt],
    span: Span,
}
struct Analyzer<'a> {
    declarations: Vec<Declaration<'a>>,
    names: HashMap<String, usize>,
    functions: Vec<Option<Function>>,
    visiting: Vec<bool>,
}

pub fn analyze(module: &ast::Module) -> Result<Program, Diagnostic> {
    let mut declarations = Vec::new();
    let mut names = HashMap::new();
    for statement in &module.statements {
        let ast::StmtKind::Function {
            name,
            parameters,
            body,
        } = &statement.kind
        else {
            return Err(Diagnostic::new(
                "only function declarations are allowed at top level",
                statement.span,
            ));
        };
        if name == "print" {
            return Err(Diagnostic::new(
                "print is a reserved built-in function",
                statement.span,
            ));
        }
        if names.insert(name.clone(), declarations.len()).is_some() {
            return Err(Diagnostic::new(
                format!("duplicate {name} function"),
                statement.span,
            ));
        }
        declarations.push(Declaration {
            name,
            parameters,
            body,
            span: statement.span,
        });
    }
    let main = *names
        .get("main")
        .ok_or_else(|| Diagnostic::new("missing main() function", Span { start: 0, end: 0 }))?;
    if !declarations[main].parameters.is_empty() {
        return Err(Diagnostic::new(
            "main must not take parameters",
            declarations[main].span,
        ));
    }
    let count = declarations.len();
    let mut analyzer = Analyzer {
        declarations,
        names,
        functions: vec![None; count],
        visiting: vec![false; count],
    };
    for index in 0..count {
        analyzer.function(index)?;
    }
    Ok(Program {
        functions: analyzer.functions.into_iter().map(Option::unwrap).collect(),
        main,
    })
}

impl Analyzer<'_> {
    fn function(&mut self, index: usize) -> Result<Type, Diagnostic> {
        if let Some(function) = &self.functions[index] {
            return Ok(function.return_type.clone());
        }
        let declaration = self.declarations[index];
        if self.visiting[index] {
            return Err(Diagnostic::new(
                "recursive function return types cannot be inferred yet",
                declaration.span,
            ));
        }
        self.visiting[index] = true;
        let mut parameters = Vec::new();
        let mut locals = HashMap::new();
        for parameter in declaration.parameters {
            if parameter.ty.array_dimensions != 0 {
                return Err(Diagnostic::new(
                    "array parameters are not implemented yet",
                    parameter.ty.span,
                ));
            }
            let ty = annotation(&parameter.ty, None)?;
            if locals.insert(parameter.name.clone(), ty.clone()).is_some() {
                return Err(Diagnostic::new(
                    format!("duplicate parameter '{}'", parameter.name),
                    parameter.span,
                ));
            }
            parameters.push((parameter.name.clone(), ty));
        }
        let mut body = Vec::new();
        let mut return_type = Type::Unit;
        let mut returned = false;
        for (position, statement) in declaration.body.iter().enumerate() {
            if returned {
                return Err(Diagnostic::new(
                    "unreachable statement after return",
                    statement.span,
                ));
            }
            let final_statement = position + 1 == declaration.body.len();
            match &statement.kind {
                ast::StmtKind::Assign {
                    name,
                    annotation: declared,
                    value,
                } => {
                    let mut value = self.expression(value, &locals)?;
                    if value.ty == Type::Unit {
                        return Err(Diagnostic::new("cannot assign a Unit value", value.span));
                    }
                    if let Some(declared) = declared {
                        let expected = annotation(declared, Some(&value.ty))?;
                        if let Some(previous) = locals.get(name) {
                            if previous != &expected {
                                return Err(Diagnostic::new(
                                    format!(
                                        "cannot change variable '{name}' from {previous} to {expected}"
                                    ),
                                    declared.span,
                                ));
                            }
                        }
                        value = coerce(value, &expected)?;
                    }
                    if let Some(expected) = locals.get(name) {
                        value = coerce(value, expected)?;
                    }
                    locals.insert(name.clone(), value.ty.clone());
                    body.push(Statement::Assign(name.clone(), value));
                }
                ast::StmtKind::Expression(expr) => {
                    let expr = self.expression(expr, &locals)?;
                    if final_statement && declaration.name != "main" {
                        return_type = expr.ty.clone();
                        body.push(Statement::Return(Some(expr)));
                        returned = true;
                    } else {
                        body.push(Statement::Evaluate(expr));
                    }
                }
                ast::StmtKind::Return(value) => {
                    if declaration.name == "main" && value.is_some() {
                        return Err(Diagnostic::new(
                            "main may only use a bare return",
                            statement.span,
                        ));
                    }
                    let expr = value
                        .as_ref()
                        .map(|e| self.expression(e, &locals))
                        .transpose()?;
                    return_type = expr.as_ref().map_or(Type::Unit, |e| e.ty.clone());
                    body.push(Statement::Return(expr));
                    returned = true;
                }
                ast::StmtKind::Function { .. } => {
                    return Err(Diagnostic::new(
                        "nested functions are not implemented by the LLVM backend yet",
                        statement.span,
                    ));
                }
            }
        }
        if matches!(return_type, Type::Array(..)) {
            return Err(Diagnostic::new(
                "returning arrays is not implemented yet",
                declaration.span,
            ));
        }
        if !returned {
            body.push(Statement::Return(None));
        }
        self.visiting[index] = false;
        self.functions[index] = Some(Function {
            name: declaration.name.into(),
            parameters,
            return_type: return_type.clone(),
            body,
        });
        Ok(return_type)
    }

    fn expression(
        &mut self,
        expr: &ast::Expr,
        locals: &HashMap<String, Type>,
    ) -> Result<Expr, Diagnostic> {
        let span = expr.span;
        let (kind, ty) = match &expr.kind {
            ast::ExprKind::Integer(value) => (ExprKind::Integer(*value), Type::Int),
            ast::ExprKind::Float(value) => (ExprKind::Float(*value), Type::Double),
            ast::ExprKind::Bool(value) => (ExprKind::Bool(*value), Type::Bool),
            ast::ExprKind::String(value) => (ExprKind::String(value.clone()), Type::String),
            ast::ExprKind::Name(name) => (
                ExprKind::Local(name.clone()),
                locals
                    .get(name)
                    .cloned()
                    .ok_or_else(|| Diagnostic::new(format!("unknown variable '{name}'"), span))?,
            ),
            ast::ExprKind::Array(items) => {
                if items.is_empty() {
                    return Err(Diagnostic::new(
                        "cannot infer an empty array's element type yet",
                        span,
                    ));
                }
                let mut values = items
                    .iter()
                    .map(|e| self.expression(e, locals))
                    .collect::<Result<Vec<_>, _>>()?;
                let mut element = values[0].ty.clone();
                for value in &values {
                    if matches!(
                        (&element, &value.ty),
                        (Type::Int, Type::Double) | (Type::Double, Type::Int)
                    ) {
                        element = Type::Double;
                    } else if element != value.ty {
                        return Err(Diagnostic::new(
                            "array elements must have a common type",
                            value.span,
                        ));
                    }
                }
                if matches!(element, Type::Unit | Type::Array(..)) {
                    return Err(Diagnostic::new(
                        "only one-dimensional arrays of scalar values are supported",
                        span,
                    ));
                }
                if values.len() > u32::MAX as usize {
                    return Err(Diagnostic::new("array is too large", span));
                }
                values = values
                    .into_iter()
                    .map(|v| coerce(v, &element))
                    .collect::<Result<_, _>>()?;
                (
                    ExprKind::Array(values),
                    Type::Array(Box::new(element), items.len()),
                )
            }
            ast::ExprKind::Index { object, index } => {
                let object = self.expression(object, locals)?;
                let index = self.expression(index, locals)?;
                let Type::Array(element, _) = &object.ty else {
                    return Err(Diagnostic::new("indexing requires an array", object.span));
                };
                if index.ty != Type::Int {
                    return Err(Diagnostic::new(
                        "array index must have type Int",
                        index.span,
                    ));
                }
                let ty = *element.clone();
                (ExprKind::Index(Box::new(object), Box::new(index)), ty)
            }
            ast::ExprKind::Unary { op, operand } => {
                let operand = self.expression(operand, locals)?;
                let valid = match op {
                    UnaryOp::Not => operand.ty == Type::Bool,
                    _ => matches!(operand.ty, Type::Int | Type::Double),
                };
                if !valid {
                    return Err(Diagnostic::new(
                        format!("invalid operand type {} for {op:?}", operand.ty),
                        span,
                    ));
                }
                let ty = operand.ty.clone();
                (ExprKind::Unary(*op, Box::new(operand)), ty)
            }
            ast::ExprKind::Binary { op, left, right } => {
                if matches!(op, BinaryOp::And | BinaryOp::Or) {
                    return Err(Diagnostic::new(
                        "short-circuit logical operators are not implemented by the LLVM backend yet",
                        span,
                    ));
                }
                let mut left = self.expression(left, locals)?;
                let mut right = self.expression(right, locals)?;
                if !matches!(left.ty, Type::Int | Type::Double)
                    || !matches!(right.ty, Type::Int | Type::Double)
                {
                    return Err(Diagnostic::new(
                        "binary arithmetic and comparisons require numeric operands",
                        span,
                    ));
                }
                let operand_type = if left.ty == Type::Double || right.ty == Type::Double {
                    Type::Double
                } else {
                    Type::Int
                };
                left = coerce(left, &operand_type)?;
                right = coerce(right, &operand_type)?;
                let ty = if matches!(
                    op,
                    BinaryOp::Equal
                        | BinaryOp::NotEqual
                        | BinaryOp::Less
                        | BinaryOp::LessEqual
                        | BinaryOp::Greater
                        | BinaryOp::GreaterEqual
                ) {
                    Type::Bool
                } else {
                    operand_type
                };
                (ExprKind::Binary(*op, Box::new(left), Box::new(right)), ty)
            }
            ast::ExprKind::Call { callee, arguments } => {
                let ast::ExprKind::Name(name) = &callee.kind else {
                    return Err(Diagnostic::new(
                        "only calls to named functions are supported",
                        callee.span,
                    ));
                };
                if locals.contains_key(name) {
                    return Err(Diagnostic::new(
                        format!("local variable '{name}' is not callable"),
                        callee.span,
                    ));
                }
                if name == "print" {
                    if arguments.len() != 1 {
                        return Err(Diagnostic::new("print expects exactly one argument", span));
                    }
                    let argument = self.expression(&arguments[0], locals)?;
                    if matches!(argument.ty, Type::Unit | Type::Array(..)) {
                        return Err(Diagnostic::new(
                            "print expects an Int, Double, Bool or String",
                            argument.span,
                        ));
                    }
                    (ExprKind::Print(Box::new(argument)), Type::Unit)
                } else {
                    let index = *self.names.get(name).ok_or_else(|| {
                        Diagnostic::new(format!("unknown function '{name}'"), callee.span)
                    })?;
                    if name == "main" {
                        return Err(Diagnostic::new(
                            "main cannot be called as a regular function",
                            span,
                        ));
                    }
                    let declaration = self.declarations[index];
                    if arguments.len() != declaration.parameters.len() {
                        return Err(Diagnostic::new(
                            format!(
                                "function '{name}' expects {} arguments, got {}",
                                declaration.parameters.len(),
                                arguments.len()
                            ),
                            span,
                        ));
                    }
                    let return_type = self.function(index)?;
                    let parameters = self.functions[index].as_ref().unwrap().parameters.clone();
                    let mut typed_arguments = Vec::new();
                    for (argument, (_, expected)) in arguments.iter().zip(parameters) {
                        typed_arguments
                            .push(coerce(self.expression(argument, locals)?, &expected)?);
                    }
                    (ExprKind::Call(index, typed_arguments), return_type)
                }
            }
            ast::ExprKind::Member { .. } => {
                return Err(Diagnostic::new(
                    "member access is not implemented by the LLVM backend yet",
                    span,
                ));
            }
        };
        Ok(Expr { kind, ty, span })
    }
}

fn annotation(annotation: &ast::TypeRef, value: Option<&Type>) -> Result<Type, Diagnostic> {
    if annotation.nullable {
        return Err(Diagnostic::new(
            "nullable values are not implemented by the LLVM backend yet",
            annotation.span,
        ));
    }
    let scalar = match annotation.name.as_str() {
        "Int" => Type::Int,
        "Double" => Type::Double,
        "Bool" => Type::Bool,
        "String" => Type::String,
        name => {
            return Err(Diagnostic::new(
                format!("unknown type '{name}'"),
                annotation.span,
            ));
        }
    };
    match annotation.array_dimensions {
        0 => Ok(scalar),
        1 => match value {
            Some(Type::Array(_, length)) => Ok(Type::Array(Box::new(scalar), *length)),
            _ => Err(Diagnostic::new(
                "array annotation requires an array value",
                annotation.span,
            )),
        },
        _ => Err(Diagnostic::new(
            "only one-dimensional arrays are supported",
            annotation.span,
        )),
    }
}
fn coerce(mut value: Expr, expected: &Type) -> Result<Expr, Diagnostic> {
    if &value.ty == expected {
        return Ok(value);
    }
    if value.ty == Type::Int && *expected == Type::Double {
        let span = value.span;
        return Ok(Expr {
            kind: ExprKind::Promote(Box::new(value)),
            ty: Type::Double,
            span,
        });
    }
    if let (ExprKind::Array(items), Type::Array(element, length)) = (&mut value.kind, expected) {
        if items.len() == *length {
            *items = std::mem::take(items)
                .into_iter()
                .map(|v| coerce(v, element))
                .collect::<Result<_, _>>()?;
            value.ty = expected.clone();
            return Ok(value);
        }
    }
    Err(Diagnostic::new(
        format!("type mismatch: expected {expected}, found {}", value.ty),
        value.span,
    ))
}

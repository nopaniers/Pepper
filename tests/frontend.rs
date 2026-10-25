use pepper::{
    ast::{BinaryOp, ExprKind, StmtKind, UnaryOp},
    lexer::{TokenKind, lex},
    parse,
};

#[test]
fn repository_examples_parse() {
    for source in [
        include_str!("../examples/hello.pepper"),
        include_str!("../examples/functions.pepper"),
    ] {
        parse(source).unwrap();
    }
}

#[test]
fn function_parameters_and_inferred_result_syntax() {
    let module = parse("add(x: Int, y: Int) = x + y\n").unwrap();
    let StmtKind::Function {
        name,
        parameters,
        body,
    } = &module.statements[0].kind
    else {
        panic!("expected function")
    };
    assert_eq!(name, "add");
    assert_eq!(parameters.len(), 2);
    assert_eq!(parameters[0].ty.name, "Int");
    assert!(matches!(body[0].kind, StmtKind::Expression(_)));
}

#[test]
fn nested_functions_and_dedents() {
    let module =
        parse("outer(x: Int) =\n  inner(y: Int) =\n    return y\n  inner(x)\nmain() = outer(1)\n")
            .unwrap();
    assert_eq!(module.statements.len(), 2);
    let StmtKind::Function { body, .. } = &module.statements[0].kind else {
        panic!()
    };
    assert_eq!(body.len(), 2);
}

#[test]
fn blank_and_comment_lines_do_not_change_indentation() {
    let source = "main() =\n\n  # comment\n  x = 1\n# outside comment\n\n  print(x)";
    let module = parse(source).unwrap();
    let StmtKind::Function { body, .. } = &module.statements[0].kind else {
        panic!()
    };
    assert_eq!(body.len(), 2);
}

#[test]
fn multiline_lists_calls_and_crlf() {
    parse("main() =\r\n  x = [\r\n    1,\r\n    2,\r\n  ]\r\n  print(\r\n    x,\r\n  )\r\n")
        .unwrap();
}

fn expression(source: &str) -> pepper::ast::Expr {
    let module = parse(source).unwrap();
    let StmtKind::Expression(expr) = module.statements.into_iter().next().unwrap().kind else {
        panic!()
    };
    expr
}

#[test]
fn arithmetic_precedence_and_power_associativity() {
    let expr = expression("1 + 2 * 3^4^5");
    let ExprKind::Binary {
        op: BinaryOp::Add,
        right,
        ..
    } = expr.kind
    else {
        panic!()
    };
    let ExprKind::Binary {
        op: BinaryOp::Multiply,
        right,
        ..
    } = right.kind
    else {
        panic!()
    };
    let ExprKind::Binary {
        op: BinaryOp::Power,
        right,
        ..
    } = right.kind
    else {
        panic!()
    };
    assert!(matches!(
        right.kind,
        ExprKind::Binary {
            op: BinaryOp::Power,
            ..
        }
    ));
}

#[test]
fn power_binds_more_tightly_than_negation() {
    let expr = expression("-2^2");
    let ExprKind::Unary {
        op: UnaryOp::Negate,
        operand,
    } = expr.kind
    else {
        panic!()
    };
    assert!(matches!(
        operand.kind,
        ExprKind::Binary {
            op: BinaryOp::Power,
            ..
        }
    ));
}

#[test]
fn logical_negation_wraps_comparison() {
    let expr = expression("not x == y and true");
    let ExprKind::Binary {
        op: BinaryOp::And,
        left,
        ..
    } = expr.kind
    else {
        panic!()
    };
    let ExprKind::Unary {
        op: UnaryOp::Not,
        operand,
    } = left.kind
    else {
        panic!()
    };
    assert!(matches!(
        operand.kind,
        ExprKind::Binary {
            op: BinaryOp::Equal,
            ..
        }
    ));
}

#[test]
fn member_index_and_call_chains() {
    assert!(matches!(
        expression("books[0].title()").kind,
        ExprKind::Call { .. }
    ));
}

#[test]
fn array_and_nullable_annotations() {
    let module = parse("matrix: Double[][]? = [[1.0]]").unwrap();
    let StmtKind::Assign {
        annotation: Some(ty),
        ..
    } = &module.statements[0].kind
    else {
        panic!()
    };
    assert!(ty.nullable);
    assert_eq!(ty.array_dimensions, 2);
}

#[test]
fn strings_preserve_unicode_and_decode_escapes() {
    let expr = expression("\"Hello 🌶️\\n\\\"Pepper\\\"\"");
    assert_eq!(expr.kind, ExprKind::String("Hello 🌶️\n\"Pepper\"".into()));
}

#[test]
fn invalid_source_returns_diagnostics() {
    for source in [
        "main() =\nprint(1)",
        "main(x) = x",
        "main() =\n  x = 1\n y = 2",
        "main() =\n\tx = 1",
        "[1)",
        "(1",
        "\"unclosed",
        "\"\\q\"",
        "1e+",
        "1e9999",
        "9223372036854775808",
        "x =",
        "@",
        "if x",
        "Point(x: Int) = x",
    ] {
        assert!(parse(source).is_err(), "unexpectedly accepted {source:?}");
    }
}

#[test]
fn errors_point_to_original_source() {
    let source = "main() =\n  print(\"🌶\")\n  @\n";
    let error = parse(source).unwrap_err();
    assert_eq!(&source[error.span.start..error.span.end], "@");
    assert!(
        error
            .render("bad.pepper", source)
            .starts_with("bad.pepper:3:3: error:")
    );
}

#[test]
fn lexer_tracks_layout_and_end_of_file_without_final_newline() {
    let tokens = lex("main() =\n  42").unwrap();
    assert_eq!(
        tokens
            .iter()
            .filter(|t| t.kind == TokenKind::Indent)
            .count(),
        1
    );
    assert_eq!(
        tokens
            .iter()
            .filter(|t| t.kind == TokenKind::Dedent)
            .count(),
        1
    );
    assert_eq!(tokens.last().unwrap().kind, TokenKind::Eof);
}

#[test]
fn empty_files_are_valid_modules() {
    assert!(parse("").unwrap().statements.is_empty());
    assert!(parse("# comment\n\n").unwrap().statements.is_empty());
}

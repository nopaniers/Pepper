use std::{ffi::OsStr, fs, path::Path, process::Command};

use pepper::{backend, semantic};

fn program(source: &str) -> semantic::Program {
    semantic::analyze(&pepper::parse(source).unwrap()).unwrap()
}

fn build_source(directory: &Path, source: &str) -> std::process::Output {
    let input = directory.join("source.pepper");
    fs::write(&input, source).unwrap();
    Command::new(env!("CARGO_BIN_EXE_pepper"))
        .arg("build")
        .arg(input)
        .arg("-o")
        .arg(directory.join("hello"))
        .output()
        .unwrap()
}

#[test]
fn hello_world_compiles_links_and_runs() {
    let directory = tempfile::tempdir().unwrap();
    let build = build_source(directory.path(), include_str!("../examples/hello.pepper"));
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let output = Command::new(directory.path().join("hello"))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(output.stdout, b"Hello from Pepper!\n");
    assert!(output.stderr.is_empty());
    assert_eq!(
        fs::read_dir(directory.path()).unwrap().count(),
        2,
        "temporary build files leaked"
    );
}

#[test]
fn output_supports_spaces_unicode_percent_signs_and_empty_strings() {
    let directory = tempfile::Builder::new()
        .prefix("pepper path with spaces ")
        .tempdir()
        .unwrap();
    let build = build_source(
        directory.path(),
        "main() =\n  print(\"🌶 Pepper: 100%\\nsecond line\")\n  print(\"\")\n  return\n",
    );
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let output = Command::new(directory.path().join("hello"))
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "🌶 Pepper: 100%\nsecond line\n\n"
    );
}

#[test]
fn a_bare_return_produces_a_successful_silent_executable() {
    let directory = tempfile::tempdir().unwrap();
    let build = build_source(directory.path(), "main() = return");
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let output = Command::new(directory.path().join("hello"))
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(output.stdout.is_empty());
}

#[test]
fn verified_ir_contains_native_entry_point_and_print_call() {
    let ir = backend::emit_ir(&program("main() = print(\"Hello world!\")"), "hello").unwrap();
    assert!(ir.contains("define i32 @pepper_main()"));
    assert!(ir.contains("call i32 @pepper_runtime_print_v1"));
    assert!(ir.contains("Hello world!"));
    assert!(ir.contains("target triple ="));
    assert!(!ir.contains("@puts"));
    assert!(!ir.contains("define i32 @main()"));
}

#[test]
fn executable_subset_rejects_invalid_and_unimplemented_programs() {
    let cases = [
        ("", "missing main"),
        ("main(x: Int) = print(\"hi\")", "must not take parameters"),
        (
            "main() = print(\"hi\")\nmain() = print(\"again\")",
            "duplicate main",
        ),
        ("other() = print(\"hi\")", "missing main"),
        ("main() = print()", "exactly one"),
        ("main() = print(\"a\", \"b\")", "exactly one"),
        ("main() = typo(\"hi\")", "unknown function"),
        ("main() = x.print(\"hi\")", "only calls"),
        ("main() = return 1", "bare return"),
        ("main() =\n  return\n  print(\"hi\")", "unreachable"),
    ];
    for (source, expected) in cases {
        let error = semantic::analyze(&pepper::parse(source).unwrap()).unwrap_err();
        assert!(error.message.contains(expected), "{source:?}: {error}");
    }
}

#[test]
fn semantic_failure_is_located_and_does_not_write_executable() {
    let directory = tempfile::tempdir().unwrap();
    let output = build_source(directory.path(), "main() =\n  print(unknown)\n");
    assert!(!output.status.success());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(error.contains("source.pepper:2:9: error:"), "{error}");
    assert!(!directory.path().join("hello").exists());
}

#[test]
fn missing_rust_compiler_leaves_existing_output_intact_and_cleans_up() {
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("hello");
    fs::write(&output, b"existing output").unwrap();
    let missing = directory.path().join("missing-rustc");
    let error = backend::build_with_rustc(
        &program("main() = return"),
        "test",
        &output,
        missing.as_os_str(),
    )
    .unwrap_err();
    assert!(error.contains("cannot run Rust compiler"));
    assert_eq!(fs::read(&output).unwrap(), b"existing output");
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
}

#[cfg(unix)]
#[test]
fn failing_rust_compiler_leaves_existing_output_intact_and_cleans_up() {
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("hello");
    fs::write(&output, b"existing output").unwrap();
    let error = backend::build_with_rustc(
        &program("main() = return"),
        "test",
        &output,
        OsStr::new("false"),
    )
    .unwrap_err();
    assert!(error.contains("failed"));
    assert_eq!(fs::read(&output).unwrap(), b"existing output");
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
}

#[test]
fn cli_rejects_overwriting_source_and_malformed_build_options() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("source.pepper");
    let source = "main() = return";
    fs::write(&input, source).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_pepper"))
        .arg("build")
        .arg(&input)
        .arg("-o")
        .arg(&input)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("must not overwrite"));
    assert_eq!(fs::read_to_string(&input).unwrap(), source);
    for args in [
        vec!["build"],
        vec!["build", "examples/hello.pepper"],
        vec!["build", "examples/hello.pepper", "--typo", "hello"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_pepper"))
            .args(args)
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("Usage:"));
    }
}

#[test]
fn rust_runtime_prints_embedded_nul_bytes_without_truncation() {
    let directory = tempfile::tempdir().unwrap();
    let build = build_source(directory.path(), "main() = print(\"a\0b\")");
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let output = Command::new(directory.path().join("hello"))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(output.stdout, b"a\0b\n");
}

#[cfg(unix)]
#[test]
fn rust_io_failure_becomes_a_nonzero_pepper_exit_status() {
    use std::{
        os::{fd::OwnedFd, unix::net::UnixStream},
        process::Stdio,
    };
    let directory = tempfile::tempdir().unwrap();
    let build = build_source(
        directory.path(),
        "show() = print(\"closed output\")\nmain() = show()",
    );
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let (writer, reader) = UnixStream::pair().unwrap();
    drop(reader);
    let output = Command::new(directory.path().join("hello"))
        .stdout(Stdio::from(OwnedFd::from(writer)))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(
        output.stderr.is_empty(),
        "runtime should return errors without panicking"
    );
}

fn run_source(source: &str) -> std::process::Output {
    let directory = tempfile::tempdir().unwrap();
    let build = build_source(directory.path(), source);
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    Command::new(directory.path().join("hello"))
        .output()
        .unwrap()
}

#[test]
fn functions_example_compiles_and_runs() {
    let result = run_source(include_str!("../examples/functions.pepper"));
    assert_eq!(result.status.code(), Some(0));
    assert_eq!(result.stdout, b"3\n");
    assert!(result.stderr.is_empty());
}

#[test]
fn typed_parameters_forward_calls_and_inferred_returns_execute() {
    let result = run_source(
        "main() =\n  print(add(10, 20))\n  print(square(3))\n  print(explicit(-5))\n  print(identity(\"Rust runtime\"))\n  print(positive(4))\nadd(x: Int, y: Int) = x + y\nsquare(x: Double) = x^2\nexplicit(x: Int) = return x*2\nidentity(text: String) = text\npositive(x: Int) = x > 0\n",
    );
    assert_eq!(result.status.code(), Some(0));
    assert_eq!(result.stdout, b"30\n9\n-10\nRust runtime\ntrue\n");
}

#[test]
fn local_reassignment_dynamic_indexes_array_copies_and_numeric_promotion() {
    let result = run_source(
        "main() =\n  a: Int[] = [10, 20, 30]\n  b = a\n  a = [1, 2, 3]\n  i = offset(0)\n  print(b[i])\n  print(a[i])\n  x: Double = 1\n  x = x + 1.5\n  print(x)\n  floats: Double[] = [1, 2]\n  print(floats[0] + 0.5)\noffset(x: Int) = x+1\n",
    );
    assert_eq!(result.status.code(), Some(0));
    assert_eq!(result.stdout, b"20\n2\n2.5\n1.5\n");
}

#[test]
fn arithmetic_precedence_powers_and_comparisons_execute() {
    let result = run_source(
        "main() =\n  print(2 + 3*4)\n  print(2^3^2)\n  print(-2^2)\n  print(7/2)\n  print(7%2)\n  print(7.0/2)\n  print(5.5%2)\n  print(not 3 < 2)\n  print(3 == 3.0)\n  print(3 != 3.0)\n  print(4 >= 4)\n  print(2.0 <= 1)\n",
    );
    assert_eq!(result.status.code(), Some(0));
    assert_eq!(
        result.stdout,
        b"14\n512\n-4\n3\n1\n3.5\n1.5\ntrue\ntrue\nfalse\ntrue\nfalse\n"
    );
}

#[test]
fn unit_functions_execute_and_return() {
    let result =
        run_source("announce() =\n  print(\"from a function\")\n  return\nmain() = announce()\n");
    assert_eq!(result.status.code(), Some(0));
    assert_eq!(result.stdout, b"from a function\n");
}

#[test]
fn invalid_types_names_signatures_and_recursive_inference_are_diagnosed() {
    for (source, message) in [
        ("main() = print(missing)", "unknown variable"),
        ("add(x: Int) = x\nmain() = add(1, 2)", "expects 1 arguments"),
        (
            "add(x: Int) = x\nmain() = add(1.5)",
            "expected Int, found Double",
        ),
        ("main() =\n  x = 1\n  x = \"bad\"", "type mismatch"),
        (
            "main() =\n  x: Double = 1\n  x: Int = 2",
            "cannot change variable",
        ),
        ("main() = print(1+\"x\")", "numeric operands"),
        ("main() = print([1, \"x\"])", "common type"),
        ("main() = print([1, 2])", "print expects"),
        ("main() = print([1][0.5])", "index must have type Int"),
        ("main() = print(1[0])", "requires an array"),
        ("main() =\n  a: String[] = [1]", "expected String"),
        ("main() =\n  a = []", "empty array"),
        ("main() =\n  a = [[1]]", "one-dimensional"),
        (
            "f(x: Int, x: Int) = x\nmain() = f(1,2)",
            "duplicate parameter",
        ),
        ("f(x: Mystery) = x\nmain() = f(1)", "unknown type"),
        ("f(x: Int?) = x\nmain() = f(1)", "nullable"),
        ("f(x: Int[]) = 1\nmain() = return", "array parameters"),
        ("f() = [1]\nmain() = return", "returning arrays"),
        ("f() = f()\nmain() = f()", "recursive"),
        ("f() = g()\ng() = f()\nmain() = f()", "recursive"),
        ("main() =\n  x = print(1)", "Unit value"),
        ("main() = main()", "cannot be called"),
        ("main() =\n  print = 1\n  print(2)", "not callable"),
        ("main() = true and false", "short-circuit"),
    ] {
        let error = semantic::analyze(&pepper::parse(source).unwrap()).unwrap_err();
        assert!(error.message.contains(message), "{source:?}: {error}");
    }
}

#[test]
fn bounds_and_arithmetic_failures_propagate_through_function_calls() {
    for (source, message) in [
        (
            "bad(i: Int) = [1, 2][i]\nmain() = print(bad(-1))",
            "index out of bounds",
        ),
        (
            "bad(i: Int) = [1, 2][i]\nmain() = print(bad(2))",
            "index out of bounds",
        ),
        (
            "bad(x: Int) = 1/x\nmain() = print(bad(0))",
            "division by zero",
        ),
        (
            "bad(x: Int) = 1%x\nmain() = print(bad(0))",
            "division by zero",
        ),
        (
            "bad(x: Int) = (-9223372036854775807-1)/x\nmain() = print(bad(-1))",
            "overflow",
        ),
        (
            "bad(x: Int) = 2^x\nmain() = print(bad(-1))",
            "negative integer exponent",
        ),
    ] {
        let result = run_source(source);
        assert_eq!(result.status.code(), Some(1), "{source}");
        assert!(result.stdout.is_empty());
        assert!(
            String::from_utf8_lossy(&result.stderr).contains(message),
            "{source}: {:?}",
            result.stderr
        );
    }
}

#[test]
fn inferred_types_are_checked_even_in_unused_functions() {
    let module = pepper::parse("unused(x: Int) = x+\"wrong\"\nmain() = return").unwrap();
    assert!(semantic::analyze(&module).is_err());
}

#[test]
fn llvm_contains_real_typed_functions_and_array_indexing() {
    let ir = backend::emit_ir(
        &program(include_str!("../examples/functions.pepper")),
        "functions",
    )
    .unwrap();
    assert!(ir.contains("define internal i32 @pepper_fn_0_add(i64"));
    assert!(ir.contains("define internal i32 @pepper_fn_1_square(double"));
    assert!(ir.contains("call i32 @pepper_fn_0_add"));
    assert!(ir.contains("call double @pepper_runtime_pow_double_v1"));
    assert!(ir.contains("getelementptr [3 x i64]"));
    assert!(ir.contains("call i32 @pepper_runtime_print_int_v1"));
}
